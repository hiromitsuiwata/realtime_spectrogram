use crate::{
    constants::{FFT_SIZE, SPEC_WIDTH},
    util::intensity_color,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// ターミナルを起動してリアルタイム描画を行う
pub fn run_ui(sample_rate: f32, spectrogram: Arc<Mutex<Vec<Vec<f32>>>>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }

        let spec = spectrogram.lock().unwrap().clone();

        terminal.draw(|f| {
            let size = f.area();
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Spectrogram (press 'q' to quit)");
            f.render_widget(&block, size);
            let inner = block.inner(size);

            let width = inner.width.min(SPEC_WIDTH as u16) as usize;
            let height = inner.height as usize;

            let f_min: f32 = 20.0;
            let f_max: f32 = sample_rate / 2.0;
            let log_min = f_min.log10();
            let log_max = f_max.log10();

            let mut lines: Vec<Line> = Vec::new();
            for row in 0..height {
                let frac = 1.0 - row as f32 / height as f32;
                let freq = 10f32.powf(log_min + frac * (log_max - log_min));

                let label = if row % (height / 8).max(1) == 0 {
                    format!("{:>6.0}Hz | ", freq)
                } else {
                    "         | ".to_string()
                };

                let mut spans: Vec<Span> = vec![Span::raw(label)];
                for column in spec.iter().rev().take(width) {
                    let fft_index = ((freq / f_max) * (FFT_SIZE as f32 / 2.0)).round() as usize;
                    if fft_index < column.len() {
                        let val = column[fft_index];
                        let intensity = ((val * 10.0) as u8).min(9);
                        let ch = " .:-=+*#%@".chars().nth(intensity as usize).unwrap_or(' ');
                        spans.push(Span::styled(
                            ch.to_string(),
                            Style::default().fg(intensity_color(val)),
                        ));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                }
                lines.push(Line::from(spans));
            }

            f.render_widget(Paragraph::new(lines), inner);
        })?;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
