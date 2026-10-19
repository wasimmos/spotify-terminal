use rtrb::{Consumer, Producer, RingBuffer};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;

pub struct AudioEngine {
    consumer: Consumer<f32>,
    is_active: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl AudioEngine {
    pub fn new() -> Self {
        // Buffer for ~0.37 seconds of mono audio at 44.1kHz
        let (producer, consumer) = RingBuffer::new(16384);
        let is_active = Arc::new(AtomicBool::new(true));
        let child_arc = Arc::new(Mutex::new(None));

        let active_clone = is_active.clone();
        let child_clone = child_arc.clone();

        thread::spawn(move || {
            capture_loop(producer, active_clone, child_clone);
        });

        Self {
            consumer,
            is_active,
            child: child_arc,
        }
    }

    /// Pull all newly captured mono audio samples into the target buffer.
    /// Returns the number of samples added.
    pub fn read_samples(&mut self, output: &mut Vec<f32>) -> usize {
        let mut count = 0;
        while let Ok(sample) = self.consumer.pop() {
            output.push(sample);
            count += 1;
        }
        count
    }

    #[allow(dead_code)]
    pub fn is_capturing(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }
}

impl Drop for AudioEngine {
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

fn capture_loop(
    mut producer: Producer<f32>,
    is_active: Arc<AtomicBool>,
    child_slot: Arc<Mutex<Option<Child>>>,
) {
    // 1. Try parec (PulseAudio/PipeWire monitor) - reliably captures active speaker loopback
    let parec_cmd = Command::new("parec")
        .args([
            "--format=float32le",
            "--rate=44100",
            "--channels=2",
            "--raw",
            "-d",
            "@DEFAULT_MONITOR@",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut maybe_child = match parec_cmd {
        Ok(c) => Some(c),
        Err(_) => {
            // 2. Fallback: pw-record
            Command::new("pw-record")
                .args([
                    "--format",
                    "f32",
                    "--rate",
                    "44100",
                    "--channels",
                    "2",
                    "--raw",
                    "-",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()
        }
    };

    let mut child = match maybe_child.take() {
        Some(c) => c,
        None => {
            is_active.store(false, Ordering::Relaxed);
            return;
        }
    };

    let mut stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            is_active.store(false, Ordering::Relaxed);
            return;
        }
    };

    // Store child handle for clean cleanup on exit
    if let Ok(mut lock) = child_slot.lock() {
        *lock = Some(child);
    }

    // 4096 bytes = 1024 f32 = 512 stereo frames
    let mut byte_buf = [0u8; 4096];

    while is_active.load(Ordering::Relaxed) {
        match stdout.read(&mut byte_buf) {
            Ok(0) => break, // EOF
            Ok(bytes_read) => {
                let num_floats = bytes_read / 4;
                let floats: &[f32] = unsafe {
                    std::slice::from_raw_parts(byte_buf.as_ptr() as *const f32, num_floats)
                };

                // Convert stereo interleaved (L, R) to mono
                let mut i = 0;
                while i + 1 < floats.len() {
                    let mono = (floats[i] + floats[i + 1]) * 0.5;
                    // If ring buffer is full, drop sample to keep latency at zero
                    let _ = producer.push(mono);
                    i += 2;
                }
            }
            Err(_) => break,
        }
    }

    is_active.store(false, Ordering::Relaxed);
}
