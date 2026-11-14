use std::time::{Duration, Instant};
pub struct Meter {
    start: Instant,
    offset: u64,
}
impl Meter {
    pub fn new(offset: u64) -> Self {
        Self {
            start: Instant::now(),
            offset,
        }
    }
    pub fn measure(&self, bytes: u64, total: u64) -> (u64, Option<u64>) {
        Self::calculate(
            bytes.saturating_sub(self.offset),
            total.saturating_sub(bytes),
            self.start.elapsed(),
        )
    }
    fn calculate(bytes: u64, remaining: u64, elapsed: Duration) -> (u64, Option<u64>) {
        if elapsed < Duration::from_millis(250) {
            return (0, None);
        }
        let speed = (bytes as f64 / elapsed.as_secs_f64()).round() as u64;
        (
            speed,
            if remaining == 0 {
                Some(0)
            } else if speed > 0 {
                Some(remaining.div_ceil(speed))
            } else {
                None
            },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn estimates_only_transferred_bytes_after_resume() {
        assert_eq!(
            Meter::calculate(1024, 2048, Duration::from_secs(2)),
            (512, Some(4))
        );
        assert_eq!(Meter::calculate(0, 2048, Duration::from_secs(2)), (0, None));
        assert_eq!(
            Meter::calculate(1024, 0, Duration::from_millis(10)),
            (0, None)
        );
    }
}
