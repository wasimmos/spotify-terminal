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

    pub fn set_bands(&mut self, new_bands: &[f32]) {
        if self.bands.len() != new_bands.len() {
            self.resize_bars(new_bands.len());
        }
        self.bands.copy_from_slice(new_bands);
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

        // Dynamic AGC boost for full screen dynamic range
        let agc_gain = (0.30 / ac_rms.max(0.03)).clamp(1.2, 9.0);

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
            let val = (avg * freq_boost * agc_gain * 2.5).sqrt();
            *band = val.clamp(0.0, 1.0);
        }

        // CAVA Monstercat spatial filter (spreads peaks into neighboring bars organically)
        let monstercat = 1.4f32;
        for z in 0..num_bars {
            for m_y in (0..z).rev() {
                let de = (z - m_y) as f32;
                raw_bands[m_y] = raw_bands[m_y].max(raw_bands[z] / (monstercat * 1.5).powf(de));
            }
            for m_y in (z + 1)..num_bars {
                let de = (m_y - z) as f32;
                raw_bands[m_y] = raw_bands[m_y].max(raw_bands[z] / (monstercat * 1.5).powf(de));
            }
        }

        // Responsive attack with physics-based gravity falloff
        for i in 0..num_bars {
            let target = raw_bands[i];
            if target > self.bands[i] {
                // Snappy attack: jump up with beats
                self.bands[i] += (target - self.bands[i]) * 0.72;
                self.peaks[i] = self.bands[i];
                self.peak_speeds[i] = 0.0;
            } else {
                // CAVA gravity falloff
                self.peak_speeds[i] += 0.032;
                let fall = self.peak_speeds[i];
                let falloff = self.peaks[i] * (1.0 - fall * fall * 1.6);
                self.bands[i] = falloff.max(0.0);
            }

            if self.bands[i] < 0.01 {
                self.bands[i] = 0.0;
            }
        }
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
