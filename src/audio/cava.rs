use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

pub const CAVA_BARS: usize = 64;

pub struct CavaStream {
    latest_bars: Arc<Mutex<Vec<f32>>>,
    is_active: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl CavaStream {
    pub fn new() -> Option<Self> {
        // 1. Verify `cava` binary is available in PATH
        let check = Command::new("cava").arg("-v").output();
        if check.is_err() {
            return None;
        }

        // 2. Prepare cava config file in ~/.config/spotif/cava.conf
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/insane".to_string());
        let config_dir = format!("{}/.config/spotif", home);
        let _ = std::fs::create_dir_all(&config_dir);
        let config_path = format!("{}/cava.conf", config_dir);

        let config_content = format!(
            r#"[general]
bars = {CAVA_BARS}
framerate = 60

[input]
method = pipewire
source = auto

[output]
method = raw
raw_target = /dev/stdout
data_format = binary
bit_format = 8bit

[smoothing]
monstercat = 1
waves = 0
noise_reduction = 77
"#
        );

        if std::fs::write(&config_path, config_content).is_err() {
            return None;
        }

        let latest_bars = Arc::new(Mutex::new(vec![0.0f32; CAVA_BARS]));
        let is_active = Arc::new(AtomicBool::new(true));
        let child_arc = Arc::new(Mutex::new(None));

        let bars_clone = latest_bars.clone();
        let active_clone = is_active.clone();
        let child_clone = child_arc.clone();
        let conf_clone = config_path;

        thread::spawn(move || {
            cava_worker_loop(bars_clone, active_clone, child_clone, conf_clone);
        });

        Some(Self {
            latest_bars,
            is_active,
            child: child_arc,
        })
    }

    /// Retrieve interpolated bar amplitudes for `target_bars` (0.0 to 1.0)
    pub fn get_interpolated_bars(&self, target_bars: usize) -> Option<Vec<f32>> {
        let guard = self.latest_bars.lock().ok()?;
        if guard.is_empty() || target_bars == 0 {
            return None;
        }
        let src_len = guard.len();
        let mut result = vec![0.0f32; target_bars];
        for i in 0..target_bars {
            let pos = i as f32 * ((src_len - 1) as f32 / (target_bars - 1).max(1) as f32);
            let idx0 = pos.floor() as usize;
            let idx1 = (idx0 + 1).min(src_len - 1);
            let frac = pos - idx0 as f32;
            result[i] = guard[idx0] * (1.0 - frac) + guard[idx1] * frac;
        }
        Some(result)
    }
}

impl Drop for CavaStream {
    fn drop(&mut self) {
        self.is_active.store(false, Ordering::Relaxed);
        if let Ok(mut lock) = self.child.lock() {
            if let Some(mut child) = lock.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn cava_worker_loop(
    bars: Arc<Mutex<Vec<f32>>>,
    is_active: Arc<AtomicBool>,
    child_slot: Arc<Mutex<Option<Child>>>,
    config_path: String,
) {
    let mut use_pulse_fallback = false;

    while is_active.load(Ordering::Relaxed) {
        if use_pulse_fallback {
            let pulse_config = format!(
                r#"[general]
bars = {CAVA_BARS}
framerate = 60

[input]
method = pulse
source = auto

[output]
method = raw
raw_target = /dev/stdout
data_format = binary
bit_format = 8bit

[smoothing]
monstercat = 1
waves = 0
noise_reduction = 77
"#
            );
            let _ = std::fs::write(&config_path, pulse_config);
        }

        let child_res = Command::new("cava")
            .args(["-p", &config_path])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let mut child = match child_res {
            Ok(c) => c,
            Err(_) => {
                thread::sleep(Duration::from_millis(500));
                continue;
            }
        };

        let mut stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = child.kill();
                thread::sleep(Duration::from_millis(500));
                continue;
            }
        };

        if let Ok(mut lock) = child_slot.lock() {
            *lock = Some(child);
        }

        let mut buf = [0u8; CAVA_BARS];
        let mut read_count = 0;

        while is_active.load(Ordering::Relaxed) {
            match stdout.read_exact(&mut buf) {
                Ok(()) => {
                    read_count += 1;
                    let float_bands: Vec<f32> = buf.iter().map(|&b| (b as f32) / 255.0).collect();
                    if let Ok(mut lock) = bars.lock() {
                        *lock = float_bands;
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }

        // If pipewire failed immediately (read 0 frames), switch to pulse
        if read_count == 0 && !use_pulse_fallback {
            use_pulse_fallback = true;
        }

        // Clean up child before potential respawn
        if let Ok(mut lock) = child_slot.lock() {
            if let Some(mut c) = lock.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }

        if is_active.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
        }
    }
}
