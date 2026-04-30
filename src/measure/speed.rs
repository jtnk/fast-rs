use std::time::Duration;

/// Convert (bytes, elapsed) to megabits per second.
pub fn bytes_to_mbps(bytes: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return 0.0;
    }
    (bytes as f64 * 8.0) / 1_000_000.0 / secs
}

/// Return true when the most recent `window` samples vary by less than `tolerance` (relative).
pub fn is_stable(samples: &[f64], window: usize, tolerance: f64) -> bool {
    if samples.len() < window {
        return false;
    }
    let recent = &samples[samples.len() - window..];
    let avg = recent.iter().sum::<f64>() / recent.len() as f64;
    if avg <= 0.0 {
        return false;
    }
    let max_dev = recent
        .iter()
        .map(|x| (x - avg).abs())
        .fold(0.0_f64, f64::max);
    (max_dev / avg) < tolerance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_megabyte_per_second_is_eight_mbps() {
        assert!((bytes_to_mbps(1_000_000, Duration::from_secs(1)) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn zero_elapsed_is_zero_mbps() {
        assert_eq!(bytes_to_mbps(1_000_000, Duration::from_secs(0)), 0.0);
    }

    #[test]
    fn stable_sequence_is_stable() {
        let samples = vec![100.0, 99.5, 100.2, 99.8, 100.1];
        assert!(is_stable(&samples, 5, 0.05));
    }

    #[test]
    fn rising_sequence_is_not_stable() {
        let samples = vec![10.0, 20.0, 50.0, 80.0, 100.0];
        assert!(!is_stable(&samples, 5, 0.05));
    }

    #[test]
    fn fewer_than_window_is_not_stable() {
        assert!(!is_stable(&[100.0, 100.0], 5, 0.05));
    }
}
