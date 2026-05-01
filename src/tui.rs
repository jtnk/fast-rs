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
    pub unloaded_latency_samples: Vec<f64>, // ms, bounded to SPARKLINE_LEN
    pub loaded_latency_samples: Vec<f64>,   // ms, bounded to SPARKLINE_LEN
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
            unloaded_latency_samples: Vec::new(),
            loaded_latency_samples: Vec::new(),
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
            }
            Progress::PhaseEnd(phase) => {
                match phase {
                    Phase::UnloadedLatency if !self.unloaded_latency_samples.is_empty() => {
                        self.unloaded_latency_ms = Some(min_f64(&self.unloaded_latency_samples));
                    }
                    Phase::LoadedLatency if !self.loaded_latency_samples.is_empty() => {
                        self.loaded_latency_ms = Some(min_f64(&self.loaded_latency_samples));
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
                let buf = match self.current_phase {
                    Some(Phase::UnloadedLatency) => Some(&mut self.unloaded_latency_samples),
                    Some(Phase::LoadedLatency) => Some(&mut self.loaded_latency_samples),
                    _ => None,
                };
                if let Some(buf) = buf {
                    buf.push(ms);
                    if buf.len() > SPARKLINE_LEN {
                        let drop = buf.len() - SPARKLINE_LEN;
                        buf.drain(0..drop);
                    }
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

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Sparkline};
use ratatui::Frame;

impl App {
    pub fn title(&self) -> String {
        match (self.current_phase, &self.final_report) {
            (_, Some(_)) => "fastrs — done — press q to quit".into(),
            (Some(Phase::UnloadedLatency), _) => "fastrs — Phase: Unloaded latency".into(),
            (Some(Phase::Download), _) => "fastrs — Phase: Download".into(),
            (Some(Phase::LoadedLatency), _) => "fastrs — Phase: Loaded latency".into(),
            (Some(Phase::Upload), _) => "fastrs — Phase: Upload".into(),
            (None, None) => "fastrs — connecting...".into(),
        }
    }

    pub fn render(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),    // chart (gets all leftover space)
                Constraint::Length(4), // latency row (two sparklines side-by-side)
                Constraint::Length(6), // stats + footer
            ])
            .split(f.area());

        self.render_chart(f, chunks[0]);
        self.render_latency(f, chunks[1]);
        self.render_stats(f, chunks[2]);
    }

    fn render_chart(&self, f: &mut Frame, area: Rect) {
        let dl: Vec<(f64, f64)> = self.download_samples.clone();
        let ul: Vec<(f64, f64)> = self.upload_samples.clone();

        let max_t = dl
            .iter()
            .chain(ul.iter())
            .map(|(t, _)| *t)
            .fold(0.0_f64, f64::max)
            .max(5.0);
        let max_mbps = dl
            .iter()
            .chain(ul.iter())
            .map(|(_, m)| *m)
            .fold(0.0_f64, f64::max)
            .max(1.0)
            * 1.1;

        let datasets = vec![
            Dataset::default()
                .name("download")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(&dl),
            Dataset::default()
                .name("upload")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Magenta))
                .data(&ul),
        ];

        let chart = Chart::new(datasets)
            .block(Block::default().borders(Borders::ALL).title(self.title()))
            .x_axis(
                Axis::default()
                    .title("seconds")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, max_t])
                    .labels(vec![
                        Span::raw("0"),
                        Span::raw(format!("{}", (max_t / 2.0) as u32)),
                        Span::raw(format!("{}", max_t as u32)),
                    ]),
            )
            .y_axis(
                Axis::default()
                    .title("Mbps")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, max_mbps])
                    .labels(vec![
                        Span::raw("0"),
                        Span::raw(format!("{:.0}", max_mbps / 2.0)),
                        Span::raw(format!("{:.0}", max_mbps)),
                    ]),
            );
        f.render_widget(chart, area);
    }

    fn render_latency(&self, f: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let unloaded_data: Vec<u64> = self
            .unloaded_latency_samples
            .iter()
            .map(|x| *x as u64)
            .collect();
        let loaded_data: Vec<u64> = self
            .loaded_latency_samples
            .iter()
            .map(|x| *x as u64)
            .collect();

        let unloaded_label = format!(
            "Unloaded latency  {}",
            self.unloaded_latency_ms
                .map(|x| format!("{x:.0} ms"))
                .unwrap_or_else(|| "—".into()),
        );
        let loaded_label = format!(
            "Loaded latency  {}",
            self.loaded_latency_ms
                .map(|x| format!("{x:.0} ms"))
                .unwrap_or_else(|| "—".into()),
        );

        let unloaded = Sparkline::default()
            .block(Block::default().borders(Borders::ALL).title(unloaded_label))
            .data(&unloaded_data)
            .style(Style::default().fg(Color::Yellow));
        let loaded = Sparkline::default()
            .block(Block::default().borders(Borders::ALL).title(loaded_label))
            .data(&loaded_data)
            .style(Style::default().fg(Color::Red));

        f.render_widget(unloaded, cols[0]);
        f.render_widget(loaded, cols[1]);
    }

    fn render_stats(&self, f: &mut Frame, area: Rect) {
        let server = self
            .final_report
            .as_ref()
            .and_then(|r| r.server_locations.first().cloned())
            .unwrap_or_else(|| "—".into());
        let client = self
            .final_report
            .as_ref()
            .map(|r| format!("{} / {}", r.client_ip, r.client_isp))
            .unwrap_or_else(|| "—".into());

        let text = vec![
            Line::from(vec![
                Span::styled("↓ ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{:>7.1} Mbps  ", self.current_dl_mbps)),
                Span::styled("↑ ", Style::default().fg(Color::Magenta)),
                Span::raw(format!("{:>7.1} Mbps  ", self.current_ul_mbps)),
                Span::raw(format!("Server: {server}")),
            ]),
            Line::from(vec![
                Span::raw(format!("Peak ↓ {:>7.1}  ", self.peak_dl_mbps)),
                Span::raw(format!("Peak ↑ {:>7.1}  ", self.peak_ul_mbps)),
                Span::raw(format!("Client: {client}")),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                if self.final_report.is_some() {
                    "press q to quit"
                } else {
                    "press q to abort"
                },
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];

        let para =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Stats"));
        f.render_widget(para, area);
    }
}

use crate::api::Targets;
use crate::measure::{self, Options};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

const TICK: Duration = Duration::from_millis(100);

/// Run the measurement under a TUI. Returns the same Report as `measure::run`.
pub async fn run(client: &reqwest::Client, targets: &Targets, opts: &Options) -> Result<Report> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_inner(&mut terminal, client, targets, opts).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_inner<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    client: &reqwest::Client,
    targets: &Targets,
    opts: &Options,
) -> Result<Report>
where
    B::Error: Send + Sync + 'static,
{
    let (tx, mut rx) = mpsc::channel::<Progress>(64);
    let mut app = App::new();

    let measurement = {
        let client = client.clone();
        let targets = targets.clone();
        let opts = Options {
            no_upload: opts.no_upload,
        };
        let tx = tx.clone();
        tokio::spawn(
            async move { measure::run_with_progress(&client, &targets, &opts, Some(tx)).await },
        )
    };
    drop(tx);

    let mut tick = tokio::time::interval(TICK);
    let mut measurement = Some(measurement);
    let mut report: Option<Report> = None;

    loop {
        terminal.draw(|f| app.render(f))?;

        tokio::select! {
            _ = tick.tick() => {
                // Drain any pending events without blocking.
                while crossterm::event::poll(Duration::from_millis(0))? {
                    if let Event::Key(k) = crossterm::event::read()? {
                        if k.kind == KeyEventKind::Press
                            && (k.code == KeyCode::Char('q') || k.code == KeyCode::Esc)
                        {
                            app.quit_requested = true;
                        }
                    }
                }
            }
            sample = rx.recv() => {
                match sample {
                    Some(p) => app.apply(p),
                    None => {
                        // Channel closed; measurement task should be done.
                        if let Some(handle) = measurement.take() {
                            let r = handle.await??;
                            app.measurement_done(r.clone());
                            report = Some(r);
                        }
                    }
                }
            }
        }

        if app.quit_requested {
            // If user quit before measurement finished, we still need to await it,
            // but we shouldn't draw anymore. Just return whatever we got, or error
            // out via the running task.
            break;
        }
    }

    if let Some(handle) = measurement {
        // User quit early. Wait for the measurement to wind down.
        report = Some(handle.await??);
    }

    Ok(report.expect("measurement must have produced a Report"))
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
    fn loaded_and_unloaded_buffers_are_independent() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::UnloadedLatency));
        app.apply(Progress::Latency { ms: 10.0 });
        app.apply(Progress::PhaseEnd(Phase::UnloadedLatency));
        app.apply(Progress::PhaseStart(Phase::LoadedLatency));
        // Unloaded samples are preserved across the loaded phase so the UI
        // can render both sparklines simultaneously.
        assert_eq!(app.unloaded_latency_samples, vec![10.0]);
        assert!(app.loaded_latency_samples.is_empty());
        app.apply(Progress::Latency { ms: 35.0 });
        app.apply(Progress::Latency { ms: 38.0 });
        app.apply(Progress::PhaseEnd(Phase::LoadedLatency));
        assert_eq!(app.unloaded_latency_ms, Some(10.0));
        assert_eq!(app.loaded_latency_ms, Some(35.0));
        assert_eq!(app.unloaded_latency_samples, vec![10.0]);
        assert_eq!(app.loaded_latency_samples, vec![35.0, 38.0]);
    }

    #[test]
    fn latency_outside_latency_phase_is_ignored() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::Download));
        app.apply(Progress::Latency { ms: 99.0 });
        assert!(app.unloaded_latency_samples.is_empty());
        assert!(app.loaded_latency_samples.is_empty());
    }

    #[test]
    fn latency_buffer_is_bounded() {
        let mut app = App::new();
        app.apply(Progress::PhaseStart(Phase::UnloadedLatency));
        for i in 0..(SPARKLINE_LEN + 10) {
            app.apply(Progress::Latency { ms: i as f64 });
        }
        assert_eq!(app.unloaded_latency_samples.len(), SPARKLINE_LEN);
        assert_eq!(
            *app.unloaded_latency_samples.last().unwrap(),
            (SPARKLINE_LEN + 10 - 1) as f64
        );
    }
}
