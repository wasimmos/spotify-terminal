use rustfft::{num_complex::Complex, FftPlanner};

#[allow(dead_code)]
pub struct VisualizerAnalyzer {
    fft_size: usize,
    sample_rate: u32,
    planner: FftPlanner<f32>,
    window: Vec<f32>,
    /// Smoothed output amplitudes (0.0 .. 1.0) for each frequency band
    pub bands: Vec<f32>,
    /// Temporal filter history for raw FFT targets
    pub prev_raw: Vec<f32>,
    /// Peak values for peak indicators
    pub peaks: Vec<f32>,
    /// Decay speeds for peaks
    pub peak_speeds: Vec<f32>,
    gravity: f32,
}

#[allow(dead_code)]
impl VisualizerAnalyzer {
    pub fn new(num_bars: usize, sample_rate: u32) -> Self {
        let fft_size = 1024;
        let mut planner = FftPlanner::new();
        let _ = planner.plan_fft_forward(fft_size);

        // Precompute Hann window to minimize spectral leakage
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos())
            })
            .collect();

        Self {
            fft_size,
            sample_rate,
            planner,
            window,
            bands: vec![0.0; num_bars],
            prev_raw: vec![0.0; num_bars],
            peaks: vec![0.0; num_bars],
            peak_speeds: vec![0.0; num_bars],
            gravity: 0.90, // Smooth, organic CAVA falloff
        }
    }

    pub fn resize_bars(&mut self, num_bars: usize) {
        if self.bands.len() != num_bars {
            self.bands.resize(num_bars, 0.0);
            self.prev_raw.resize(num_bars, 0.0);
            self.peaks.resize(num_bars, 0.0);
            self.peak_speeds.resize(num_bars, 0.0);
        }
    }

    /// Smoothly decay bars towards zero when music is paused or silent
    pub fn decay_bands(&mut self) {
        for b in self.bands.iter_mut() {
            *b = (*b * self.gravity).max(0.0);
            if *b < 0.005 {
                *b = 0.0;
            }
        }
        for r in self.prev_raw.iter_mut() {
            *r = (*r * 0.90).max(0.0);
        }
        for (i, p) in self.peaks.iter_mut().enumerate() {
            if i < self.peak_speeds.len() {
                self.peak_speeds[i] += 0.003;
                *p = (*p - self.peak_speeds[i]).max(0.0);
            }
        }
    }

    /// Process PCM audio samples and update frequency bands
    pub fn process_samples(&mut self, samples: &[f32]) {
        let num_bars = self.bands.len();
        if num_bars == 0 || samples.is_empty() {
            self.decay_bands();
            return;
        }

        let count = samples.len().min(self.fft_size);

        // 1. Calculate DC mean to subtract hardware bias (PipeWire/hardware DAC often has -0.26 offset)
        let mut sum = 0.0f32;
        for i in 0..count {
            sum += samples[i];
        }
        let dc_mean = sum / count as f32;

        // 2. Check true AC energy (ignoring DC)
        let mut sum_sq = 0.0f32;
        for i in 0..count {
            let ac = samples[i] - dc_mean;
            sum_sq += ac * ac;
        }
        let ac_rms = (sum_sq / count as f32).sqrt();

        // If true audio signal is silent or paused, decay cleanly
        if ac_rms < 0.0015 {
            self.decay_bands();
            return;
        }

        // 3. Prepare Hann-windowed buffer with DC bias removed
        let mut buffer: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); self.fft_size];
        for i in 0..count {
            let clean = samples[i] - dc_mean;
            buffer[i] = Complex::new(clean * self.window[i], 0.0);
        }

        // 4. Run forward FFT
        let fft = self.planner.plan_fft_forward(self.fft_size);
        fft.process(&mut buffer);

        // 5. Calculate magnitude spectrum (positive frequencies only, 0 to fft_size/2)
        let num_bins = self.fft_size / 2;
        let mut magnitudes: Vec<f32> = Vec::with_capacity(num_bins);
        for i in 0..num_bins {
            let mag = buffer[i].norm() / (self.fft_size as f32 / 2.0);
            magnitudes.push(mag);
        }

        // 6. Map FFT bins to logarithmic perceptual frequency bands (35 Hz - 14 kHz)
        let min_freq = 35.0f32;
        let max_freq = 14000.0f32;
        let nyquist = self.sample_rate as f32 / 2.0;

        let mut raw_bands = vec![0.0f32; num_bars];

        for (bar_idx, band) in raw_bands.iter_mut().enumerate() {
            let f_low = min_freq * (max_freq / min_freq).powf(bar_idx as f32 / num_bars as f32);
            let f_high = min_freq * (max_freq / min_freq).powf((bar_idx + 1) as f32 / num_bars as f32);

            let bin_low = ((f_low / nyquist) * num_bins as f32).floor() as usize;
            let bin_high = (((f_high / nyquist) * num_bins as f32).ceil() as usize).max(bin_low + 1);

            // CRITICAL: Bin 0 is DC offset (0 Hz). Always start from bin >= 1 to prevent bass ceiling pinning!
            let bin_low = bin_low.max(1).min(num_bins - 1);
            let bin_high = bin_high.max(bin_low + 1).min(num_bins);

            let mut bin_sum = 0.0f32;
            let mut bin_count = 0;
            for &m in &magnitudes[bin_low..bin_high] {
                bin_sum += m;
                bin_count += 1;
            }

            let avg = if bin_count > 0 { bin_sum / bin_count as f32 } else { 0.0 };

            // Equal-loudness contour weighting (boost highs smoothly)
            let freq_boost = 1.0 + (bar_idx as f32 / num_bars as f32).powf(0.75) * 3.6;
            // Acoustic loudness compression using sqrt
            let val = (avg * freq_boost * 3.8).sqrt();
            *band = val.clamp(0.0, 1.0);
        }

        // Temporal low-pass filter on raw targets: removes single-frame jumping and high-frequency buzzing
        for i in 0..num_bars {
            if i < self.prev_raw.len() {
                self.prev_raw[i] = self.prev_raw[i] * 0.55 + raw_bands[i] * 0.45;
                raw_bands[i] = self.prev_raw[i];
            }
        }

        // 7. Temporal envelope: gentle attack + silky decay (no jitter / eye strain)
        for i in 0..num_bars {
            let target = raw_bands[i];
            let diff = target - self.bands[i];

            // Micro-deadband: ignore tiny noise jitter (< 2.5%) to prevent resting buzz
            if diff.abs() < 0.025 {
                continue;
            }

            if diff > 0.0 {
                // Gentle attack (0.28): bars swell organically with rhythm without harsh vibration
                self.bands[i] += diff * 0.28;
            } else {
                // Silky decay (0.92): fluid, relaxed downward glide
                self.bands[i] = (self.bands[i] * 0.92).max(0.0);
            }

            if self.bands[i] < 0.01 {
                self.bands[i] = 0.0;
            }

            // Smooth peak markers
            if self.bands[i] >= self.peaks[i] {
                self.peaks[i] = self.bands[i];
                self.peak_speeds[i] = 0.003;
            } else {
                self.peak_speeds[i] += 0.002;
                self.peaks[i] = (self.peaks[i] - self.peak_speeds[i]).max(0.0);
            }
        }

        // 8. Spatial smoothing: 5-tap Gaussian filter for liquid, hypnotic visualizer waves
        let mut smoothed = self.bands.clone();
        if num_bars >= 5 {
            for i in 2..num_bars - 2 {
                smoothed[i] = 0.10 * self.bands[i - 2]
                    + 0.25 * self.bands[i - 1]
                    + 0.30 * self.bands[i]
                    + 0.25 * self.bands[i + 1]
                    + 0.10 * self.bands[i + 2];
            }
            if num_bars > 1 {
                smoothed[1] = 0.25 * self.bands[0] + 0.50 * self.bands[1] + 0.25 * self.bands[2];
                smoothed[num_bars - 2] = 0.25 * self.bands[num_bars - 3]
                    + 0.50 * self.bands[num_bars - 2]
                    + 0.25 * self.bands[num_bars - 1];
            }
        } else if num_bars >= 3 {
            for i in 1..num_bars - 1 {
                smoothed[i] = 0.25 * self.bands[i - 1] + 0.50 * self.bands[i] + 0.25 * self.bands[i + 1];
            }
        }
        self.bands = smoothed;
    }

    /// Generates procedural rhythmic audio visualization frames (demo fallback)
    pub fn update_procedural(&mut self, time: f32) {
        let num_bars = self.bands.len();
        if num_bars == 0 {
            return;
        }

        let mut raw_bands = vec![0.0f32; num_bars];
        for i in 0..num_bars {
            let ratio = i as f32 / num_bars as f32;
            let kick = ((time * 3.5).sin().powi(4) * 0.9).max(0.0) * (1.0 - ratio).powi(2);
            let hats = ((time * 7.0 + ratio * 8.0).cos().abs() * 0.6) * ratio.powf(0.8);
            let melody = ((time * 2.2 + ratio * 4.0).sin().abs() * 0.5) * (1.0 - (ratio - 0.5).abs() * 1.5).max(0.0);

            raw_bands[i] = (kick + hats + melody).clamp(0.0, 1.0);
        }

        for i in 0..num_bars {
            if raw_bands[i] > self.bands[i] {
                self.bands[i] = raw_bands[i];
            } else {
                self.bands[i] = (self.bands[i] * self.gravity).max(0.0);
            }
        }
    }
}
