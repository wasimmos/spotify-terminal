use rustfft::{num_complex::Complex, FftPlanner};

#[allow(dead_code)]
pub struct VisualizerAnalyzer {
    fft_size: usize,
    sample_rate: u32,
    planner: FftPlanner<f32>,
    window: Vec<f32>,
    /// Smoothed output amplitudes (0.0 .. 1.0) for each frequency band
    pub bands: Vec<f32>,
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
            peaks: vec![0.0; num_bars],
            peak_speeds: vec![0.0; num_bars],
            gravity: 0.88, // Gravity falloff factor for organic decay
        }
    }

    pub fn resize_bars(&mut self, num_bars: usize) {
        if self.bands.len() != num_bars {
            self.bands.resize(num_bars, 0.0);
            self.peaks.resize(num_bars, 0.0);
            self.peak_speeds.resize(num_bars, 0.0);
        }
    }

    /// Process PCM audio samples and update frequency bands
    pub fn process_samples(&mut self, samples: &[f32]) {
        let num_bars = self.bands.len();
        if num_bars == 0 {
            return;
        }

        let mut buffer: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); self.fft_size];

        // Apply window and copy samples
        let count = samples.len().min(self.fft_size);
        for i in 0..count {
            buffer[i] = Complex::new(samples[i] * self.window[i], 0.0);
        }

        // Run FFT
        let fft = self.planner.plan_fft_forward(self.fft_size);
        fft.process(&mut buffer);

        // Calculate magnitude spectrum (positive frequencies only, 0 to fft_size/2)
        let num_bins = self.fft_size / 2;
        let mut magnitudes: Vec<f32> = Vec::with_capacity(num_bins);
        for i in 0..num_bins {
            let mag = (buffer[i].norm() / (self.fft_size as f32 / 2.0)) * 2.5;
            magnitudes.push(mag);
        }

        // Map FFT bins to logarithmic perceptual frequency bands (20 Hz - 18 kHz)
        let min_freq = 25.0f32;
        let max_freq = 16000.0f32;
        let nyquist = self.sample_rate as f32 / 2.0;

        let mut raw_bands = vec![0.0f32; num_bars];

        for (bar_idx, band) in raw_bands.iter_mut().enumerate() {
            // Logarithmic spacing
            let f_low = min_freq * (max_freq / min_freq).powf(bar_idx as f32 / num_bars as f32);
            let f_high = min_freq * (max_freq / min_freq).powf((bar_idx + 1) as f32 / num_bars as f32);

            let bin_low = ((f_low / nyquist) * num_bins as f32).floor() as usize;
            let bin_high = (((f_high / nyquist) * num_bins as f32).ceil() as usize).max(bin_low + 1);

            let bin_low = bin_low.min(num_bins - 1);
            let bin_high = bin_high.min(num_bins);

            let mut sum = 0.0;
            let mut count = 0;
            for &m in &magnitudes[bin_low..bin_high] {
                sum += m;
                count += 1;
            }

            let avg = if count > 0 { sum / count as f32 } else { 0.0 };

            // Apply perceptual frequency weighting (equal-loudness / treble boost)
            let freq_boost = 1.0 + (bar_idx as f32 / num_bars as f32) * 2.2;
            *band = (avg * freq_boost).clamp(0.0, 1.0);
        }

        // Smooth bands with gravity decay
        for i in 0..num_bars {
            let current = raw_bands[i];
            if current > self.bands[i] {
                // Instant rise on audio beats
                self.bands[i] = current;
            } else {
                // Organic falloff
                self.bands[i] = (self.bands[i] * self.gravity).max(0.0);
            }

            // Peaks
            if self.bands[i] >= self.peaks[i] {
                self.peaks[i] = self.bands[i];
                self.peak_speeds[i] = 0.005;
            } else {
                self.peak_speeds[i] += 0.003;
                self.peaks[i] = (self.peaks[i] - self.peak_speeds[i]).max(0.0);
            }
        }

        // Light neighbor smoothing for that liquid CAVA aesthetic
        let mut smoothed = self.bands.clone();
        for i in 1..num_bars - 1 {
            smoothed[i] = 0.25 * self.bands[i - 1] + 0.5 * self.bands[i] + 0.25 * self.bands[i + 1];
        }
        self.bands = smoothed;
    }

    /// Generates procedural rhythmic audio visualization frames (for demo/standby mode)
    pub fn update_procedural(&mut self, time: f32) {
        let num_bars = self.bands.len();
        if num_bars == 0 {
            return;
        }

        let mut raw_bands = vec![0.0f32; num_bars];
        for i in 0..num_bars {
            let ratio = i as f32 / num_bars as f32;
            // Rhythmic kick beat on bass (left)
            let kick = ((time * 3.5).sin().powi(4) * 0.9).max(0.0) * (1.0 - ratio).powi(2);
            // Snare / high hat flutter on treble (right)
            let hats = ((time * 7.0 + ratio * 8.0).cos().abs() * 0.6) * ratio.powf(0.8);
            // Mid melody wave
            let melody = ((time * 2.2 + ratio * 4.0).sin().abs() * 0.5) * (1.0 - (ratio - 0.5).abs() * 1.5).max(0.0);

            raw_bands[i] = (kick + hats + melody).clamp(0.0, 1.0);
        }

        for i in 0..num_bars {
            if raw_bands[i] > self.bands[i] {
                self.bands[i] = raw_bands[i];
            } else {
                self.bands[i] = (self.bands[i] * self.gravity).max(0.0);
            }

            if self.bands[i] >= self.peaks[i] {
                self.peaks[i] = self.bands[i];
                self.peak_speeds[i] = 0.005;
            } else {
                self.peak_speeds[i] += 0.003;
                self.peaks[i] = (self.peaks[i] - self.peak_speeds[i]).max(0.0);
            }
        }
    }
}
