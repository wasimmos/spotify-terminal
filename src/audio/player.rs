use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

#[allow(dead_code)]
pub struct AudioEngine {
    pub is_playing: Arc<AtomicBool>,
    pub position_ms: Arc<AtomicU64>,
    pub duration_ms: Arc<AtomicU64>,
    pub volume: f32,
    sample_consumer: Option<Consumer<f32>>,
}

#[allow(dead_code)]
impl AudioEngine {
    pub fn new() -> (Self, Producer<f32>) {
        // 16384 samples buffer (~0.37s at 44.1kHz), plenty for FFT chunk extraction
        let (producer, consumer) = RingBuffer::new(16384);

        let engine = Self {
            is_playing: Arc::new(AtomicBool::new(true)),
            position_ms: Arc::new(AtomicU64::new(0)),
            duration_ms: Arc::new(AtomicU64::new(245_000)), // 4m 05s default
            volume: 0.8,
            sample_consumer: Some(consumer),
        };

        (engine, producer)
    }

    pub fn take_consumer(&mut self) -> Option<Consumer<f32>> {
        self.sample_consumer.take()
    }

    pub fn toggle_play(&self) -> bool {
        let current = self.is_playing.load(Ordering::Relaxed);
        self.is_playing.store(!current, Ordering::Relaxed);
        !current
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn elapsed(&self) -> Duration {
        Duration::from_millis(self.position_ms.load(Ordering::Relaxed))
    }

    pub fn total_duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms.load(Ordering::Relaxed))
    }

    pub fn set_position_ms(&self, ms: u64) {
        self.position_ms.store(ms, Ordering::Relaxed);
    }
}
