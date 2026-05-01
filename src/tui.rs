use crate::measure::{Phase, Progress, Report};
use std::time::Instant;

const SPARKLINE_LEN: usize = 40;

/// Pure UI state. Updated by Progress events; drives the render.
#[derive(Debug)]
pub struct App {
    pub started: Instant,
    pub current_phase: Option<Phase>,
    pub phase_started: Option<Instant>,
    pub download_samples: Vec<(f64, f64)>, // (elapsed_secs, mbps)
    pub upload_samples: Vec<(f64, f64)>,
    pub latency_samples: Vec<f64>, // ms, bounded to SPARKLINE_LEN
    pub current_dl_mbps: f64,
    pub current_ul_mbps: f64,
    pub peak_dl_mbps: f64,
    pub peak_ul_mbps: f64,
    pub unloaded_latency_ms: Option<f64>,
    pub loaded_latency_ms: Option<f64>,
    pub final_report: Option<Report>,
    pub quit_requested: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            current_phase: None,
            phase_started: None,
            download_samples: Vec::new(),
            upload_samples: Vec::new(),
            latency_samples: Vec::new(),
            current_dl_mbps: 0.0,
            current_ul_mbps: 0.0,
            peak_dl_mbps: 0.0,
            peak_ul_mbps: 0.0,
            unloaded_latency_ms: None,
            loaded_latency_ms: None,
            final_report: None,
            quit_requested: false,
        }
    }

    /// Apply a progress event. Pure; no I/O.
    pub fn apply(&mut self, p: Progress) {
        let elapsed = self.started.elapsed().as_secs_f64();
        match p {
            Progress::PhaseStart(phase) => {
                self.current_phase = Some(phase);
                self.phase_started = Some(Instant::now());
                if matches!(phase, Phase::UnloadedLatency | Phase::LoadedLatency) {
                    self.latency_samples.clear();
                }
            }
            Progress::PhaseEnd(phase) => {
                match phase {
                    Phase::UnloadedLatency if !self.latency_samples.is_empty() => {
                        self.unloaded_latency_ms = Some(min_f64(&self.latency_samples));
                    }
                    Phase::LoadedLatency if !self.latency_samples.is_empty() => {
                        self.loaded_latency_ms = Some(min_f64(&self.latency_samples));
                    }
                    _ => {}
                }
                if self.current_phase == Some(phase) {
                    self.current_phase = None;
                }
            }
            Progress::Throughput { mbps } => match self.current_phase {
                Some(Phase::Download) => {
                    self.current_dl_mbps = mbps;
                    if mbps > self.peak_dl_mbps {
                        self.peak_dl_mbps = mbps;
                    }
                    self.download_samples.push((elapsed, mbps));
                }
                Some(Phase::Upload) => {
                    self.current_ul_mbps = mbps;
                    if mbps > self.peak_ul_mbps {
                        self.peak_ul_mbps = mbps;
                    }
                    self.upload_samples.push((elapsed, mbps));
                }
                _ => {}
            },
            Progress::Latency { ms } => {
                self.latency_samples.push(ms);
                if self.latency_samples.len() > SPARKLINE_LEN {
                    let drop = self.latency_samples.len() - SPARKLINE_LEN;
                    self.latency_samples.drain(0..drop);
                }
            }
        }
    }

    pub fn measurement_done(&mut self, report: Report) {
        self.unloaded_latency_ms = Some(report.unloaded_latency_ms);
        self.loaded_latency_ms = Some(report.loaded_latency_ms);
        self.final_report = Some(report);
        self.current_phase = None;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn min_f64(xs: &[f64]) -> f64 {
    xs.iter().cloned().fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throughput_during_download_records_sample_and_peak() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::Download));
        app.apply(Progress::Throughput { mbps: 100.0 });
        app.apply(Progress::Throughput { mbps: 250.0 });
        app.apply(Progress::Throughput { mbps: 200.0 });
        assert_eq!(app.download_samples.len(), 3);
        assert_eq!(app.current_dl_mbps, 200.0);
        assert_eq!(app.peak_dl_mbps, 250.0);
        assert!(app.upload_samples.is_empty());
    }

    #[test]
    fn throughput_during_upload_records_sample() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::Upload));
        app.apply(Progress::Throughput { mbps: 50.0 });
        assert_eq!(app.upload_samples.len(), 1);
        assert_eq!(app.current_ul_mbps, 50.0);
    }

    #[test]
    fn throughput_outside_throughput_phase_is_ignored() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::UnloadedLatency));
        app.apply(Progress::Throughput { mbps: 100.0 });
        assert!(app.download_samples.is_empty());
        assert!(app.upload_samples.is_empty());
    }

    #[test]
    fn latency_phase_end_records_min() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::UnloadedLatency));
        app.apply(Progress::Latency { ms: 30.0 });
        app.apply(Progress::Latency { ms: 12.0 });
        app.apply(Progress::Latency { ms: 18.0 });
        app.apply(Progress::PhaseEnd(Phase::UnloadedLatency));
        assert_eq!(app.unloaded_latency_ms, Some(12.0));
    }

    #[test]
    fn loaded_latency_clears_unloaded_buffer_then_records_its_own_min() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::UnloadedLatency));
        app.apply(Progress::Latency { ms: 10.0 });
        app.apply(Progress::PhaseEnd(Phase::UnloadedLatency));
        app.apply(Progress::PhaseStart(Phase::LoadedLatency));
        assert!(app.latency_samples.is_empty());
        app.apply(Progress::Latency { ms: 35.0 });
        app.apply(Progress::Latency { ms: 38.0 });
        app.apply(Progress::PhaseEnd(Phase::LoadedLatency));
        assert_eq!(app.unloaded_latency_ms, Some(10.0));
        assert_eq!(app.loaded_latency_ms, Some(35.0));
    }

    #[test]
    fn latency_buffer_is_bounded() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::Download));
        for i in 0..(SPARKLINE_LEN + 10) {
            app.apply(Progress::Latency { ms: i as f64 });
        }
        assert_eq!(app.latency_samples.len(), SPARKLINE_LEN);
        // Most-recent value should be the last we pushed.
        assert_eq!(
            *app.latency_samples.last().unwrap(),
            (SPARKLINE_LEN + 10 - 1) as f64
        );
    }
}
