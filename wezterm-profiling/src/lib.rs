use std::time::{Duration, Instant};

pub trait ProfilingZoneBackend {
    fn begin(name: &'static str) -> Self;
    fn elapsed(&self) -> Duration {
        Duration::ZERO
    }
}

pub struct MetricsZone {
    start: Instant,
    histogram: metrics::Histogram,
}

impl ProfilingZoneBackend for MetricsZone {
    fn begin(name: &'static str) -> Self {
        Self {
            start: Instant::now(),
            histogram: metrics::histogram!(name),
        }
    }

    fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

impl Drop for MetricsZone {
    fn drop(&mut self) {
        self.histogram.record(self.start.elapsed());
    }
}

impl<A: ProfilingZoneBackend, B: ProfilingZoneBackend> ProfilingZoneBackend for (A, B) {
    fn begin(name: &'static str) -> Self {
        (A::begin(name), B::begin(name))
    }

    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
}

pub type DefaultZone = MetricsZone;

#[macro_export]
macro_rules! profile_zone {
    ($name:expr) => {
        let _zone = $crate::DefaultZone::begin($name);
    };
    ($name:expr, $var:ident) => {
        let $var = $crate::DefaultZone::begin($name);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_zone_records_elapsed() {
        let zone = MetricsZone::begin("test.zone");
        std::thread::sleep(Duration::from_millis(1));
        let e = zone.elapsed();
        assert!(e >= Duration::from_millis(1));
    }

    #[test]
    fn tuple_elapsed_delegates_to_first() {
        let zone = (MetricsZone::begin("a"), MetricsZone::begin("b"));
        std::thread::sleep(Duration::from_millis(1));
        let e = zone.elapsed();
        assert!(e >= Duration::from_millis(1));
    }
}
