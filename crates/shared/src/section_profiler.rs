#[cfg(feature = "profile")]
use std::fmt::Write;
#[cfg(feature = "profile")]
use std::time::{Duration, Instant};

#[cfg(feature = "profile")]
use indexmap::IndexMap;
#[cfg(feature = "profile")]
use log::*;

/// Information accumulated from multiple samples across a period of time.
#[derive(Default)]
#[cfg(feature = "profile")]
struct SampleSet {
    /// The total time of all samples.
    sum: Duration,

    /// The number of samples taken.
    length: u32,

    /// The number of samples active when recording this one.
    depth: usize,
}

#[cfg(feature = "profile")]
impl SampleSet {
    /// Adds `sample` to the set.
    fn add(&mut self, sample: Duration) {
        self.sum += sample;
        self.length += 1;
    }

    /// The average time per sample.
    fn average(&self) -> Duration {
        self.sum / self.length
    }
}

/// An actively running sample for a given [SectionProfiler] section.
pub struct Sample {
    /// The name of the section being sampled.
    #[cfg(feature = "profile")]
    section: &'static str,

    /// The time the sample began being recorded.
    #[cfg(feature = "profile")]
    start: Instant,
}

/// A simple struct that tracks the average time taken by various operations
/// each frame.
pub struct SectionProfiler {
    /// The time at which the last report was written, or else the time at which
    /// the profiler was created.
    #[cfg(feature = "profile")]
    last_report: Instant,

    /// A map from section names to the rolling averages for each section.
    #[cfg(feature = "profile")]
    averages: IndexMap<&'static str, SampleSet>,

    /// The number of samples currently being recorded.
    #[cfg(feature = "profile")]
    depth: usize,
}

impl SectionProfiler {
    /// Creates a new profiler.
    #[cfg(feature = "profile")]
    pub fn new() -> Self {
        SectionProfiler {
            last_report: Instant::now(),
            averages: Default::default(),
            depth: 0,
        }
    }

    #[cfg(not(feature = "profile"))]
    #[inline]
    pub fn new() -> Self {
        SectionProfiler {}
    }

    /// Begins a sample for a given `section`.
    #[cfg(feature = "profile")]
    pub fn start_sample(&mut self, section: &'static str) -> Sample {
        // Add an entry if one doesn't exist so that the profiler sections
        // appear in source order.
        self.averages.entry(section).or_default().depth = self.depth;
        self.depth += 1;
        Sample {
            section,
            start: Instant::now(),
        }
    }

    #[cfg(not(feature = "profile"))]
    #[inline]
    pub fn start_sample(&mut self, _section: &'static str) -> Sample {
        Sample {}
    }

    /// Ends and records a sample for a given section.
    #[cfg(feature = "profile")]
    pub fn record_sample(&mut self, sample: Sample) {
        self.depth -= 1;
        self.averages
            .entry(sample.section)
            .or_default()
            .add(Instant::now().duration_since(sample.start));
    }

    #[cfg(not(feature = "profile"))]
    #[inline]
    pub fn record_sample(&mut self, _sample: Sample) {}

    /// Prints a report on the average of all sections since the last report.
    #[cfg(feature = "profile")]
    pub fn report(&mut self) {
        let elapsed = Instant::now().duration_since(self.last_report);
        let frames = self.averages.values().next().unwrap().length;
        let fps = f64::from(frames) / elapsed.as_secs_f64();
        let mut message = format!(
            "In the last {:?}, {} frames were rendered ({:02} FPS):\n",
            elapsed, frames, fps
        );

        let total_time_per_frame = elapsed / frames;
        for (section, samples) in self.averages.iter() {
            let time_per_frame = samples.average();
            let _ = writeln!(
                message,
                "{}{section} took {:?}/frame avg ({:.2}% of frame)",
                "  ".repeat(samples.depth + 1),
                time_per_frame,
                100.0 * (time_per_frame.as_micros() as f64)
                    / (total_time_per_frame.as_micros() as f64),
            );
        }

        info!("{}", message.trim_end());

        self.last_report = Instant::now();
        self.averages.clear();
    }

    #[cfg(not(feature = "profile"))]
    #[inline]
    pub fn report(&mut self) {}
}

impl Default for SectionProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// A helper macro for profiling a section of code using a [SectionProfiler].
///
/// **Warning:** This intentionally executes `$profiler` multiple times so that
/// it can call `&mut self` methods without holding a mutable borrow during
/// `$b`.
macro_rules! prof {
    ($profiler:expr, $name:expr, $b:block) => {{
        let sample = ($profiler).start_sample($name);
        $b;
        ($profiler).record_sample(sample);
    }};
}

pub(crate) use prof;
