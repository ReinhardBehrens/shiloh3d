//! Engine clocks and fixed-timestep helpers (pure `std::time`).

use core::time::Duration;
use std::time::Instant as StdInstant;

pub type Instant = StdInstant;

/// Wall-clock and simulation timing for one frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameTime {
    pub delta: Duration,
    pub delta_seconds: f32,
    pub elapsed: Duration,
    pub frame_index: u64,
}

/// Accumulates frame deltas and drives fixed updates (physics-friendly).
#[derive(Debug, Clone)]
pub struct FixedTimestep {
    step: Duration,
    accumulator: Duration,
    max_steps: u32,
}

impl FixedTimestep {
    pub fn new(hz: f64) -> Self {
        assert!(hz > 0.0);
        Self {
            step: Duration::from_secs_f64(1.0 / hz),
            accumulator: Duration::ZERO,
            max_steps: 8,
        }
    }

    pub fn with_max_steps(mut self, max_steps: u32) -> Self {
        self.max_steps = max_steps.max(1);
        self
    }

    #[inline]
    pub fn step(&self) -> Duration {
        self.step
    }

    /// Consumes `delta`, returns how many fixed steps to run this frame.
    pub fn advance(&mut self, delta: Duration) -> u32 {
        self.accumulator = self.accumulator.saturating_add(delta);
        let mut steps = 0u32;
        while self.accumulator >= self.step && steps < self.max_steps {
            self.accumulator -= self.step;
            steps += 1;
        }
        // Spiral-of-death guard: drop leftover if we hit the cap.
        if steps == self.max_steps {
            self.accumulator = Duration::ZERO;
        }
        steps
    }

    #[inline]
    pub fn alpha(&self) -> f32 {
        let step = self.step.as_secs_f32();
        if step <= f32::EPSILON {
            0.0
        } else {
            (self.accumulator.as_secs_f32() / step).clamp(0.0, 1.0)
        }
    }
}

/// Primary engine clock.
#[derive(Debug)]
pub struct Time {
    start: Instant,
    last: Instant,
    frame: FrameTime,
    fixed: FixedTimestep,
}

impl Time {
    pub fn new(fixed_hz: f64) -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            frame: FrameTime {
                delta: Duration::ZERO,
                delta_seconds: 0.0,
                elapsed: Duration::ZERO,
                frame_index: 0,
            },
            fixed: FixedTimestep::new(fixed_hz),
        }
    }

    /// Tick once per frame; returns fixed-step count for this frame.
    pub fn tick(&mut self) -> u32 {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last);
        self.last = now;
        self.frame.delta = delta;
        self.frame.delta_seconds = delta.as_secs_f32();
        self.frame.elapsed = now.saturating_duration_since(self.start);
        self.frame.frame_index = self.frame.frame_index.wrapping_add(1);
        self.fixed.advance(delta)
    }

    #[inline]
    pub fn frame(&self) -> FrameTime {
        self.frame
    }

    #[inline]
    pub fn fixed(&self) -> &FixedTimestep {
        &self.fixed
    }

    #[inline]
    pub fn fixed_mut(&mut self) -> &mut FixedTimestep {
        &mut self.fixed
    }
}
