use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::unbounded;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use num_traits::ToPrimitive;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders},
    Terminal,
};
use rustfft::{num_complex::Complex, FftPlanner};

const SAMPLE_RATE: usize = 44100;
const FFT_SIZE: usize = 512;
const SPEC_WIDTH: usize = 200; // 横方向の時間フレーム数

fn main() -> anyhow::Result<()> {
    // === マイク初期化 ===
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    println!("使用デバイス: {}", device.name()?);
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0 as usize;

    let (tx, rx) = unbounded::<Vec<f32>>();

    // ストリーム構築
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), tx.clone())?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), tx.clone())?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), tx.clone())?,
        _ => panic!("unsupported format"),
    };
    stream.play()?;

    // === スペクトログラムデータ共有 ===
    let spectrogram = Arc::new(Mutex::new(vec![vec![0.0; FFT_SIZE / 2]; SPEC_WIDTH]));
    let spec_ref = Arc::clone(&spectrogram);

    // === FFTスレッド ===
    thread::spawn(move || {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let mut buffer = Vec::<f32>::new();
        for chunk in rx {
            buffer.extend(chunk);
            while buffer.len() >= FFT_SIZE {
                let frame: Vec<f32> = buffer.drain(..FFT_SIZE).collect();
                let mut input: Vec<Complex<f32>> = frame
                    .into_iter()
                    .map(|x| Complex { re: x, im: 0.0 })
                    .collect();
                fft.process(&mut input);
                let mags: Vec<f32> = input[..FFT_SIZE / 2]
                    .iter()
                    .map(|c| (c.norm() / (FFT_SIZE as f32)).log10().max(-2.0) + 2.0)
                    .collect();
                let mut spec = spec_ref.lock().unwrap();

                spec.pop();
                spec.insert(0, mags);
            }
        }
    });

    // === TUI描画 ===
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        // 終了判定
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }

        let spec = spectrogram.lock().unwrap().clone();
        terminal.draw(|f| {
            let size = f.size();
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Spectrogram (press 'q' to quit)");
            f.render_widget(&block, size);
            let inner = block.inner(size);

            let width = inner.width.min(SPEC_WIDTH as u16) as usize;
            let height = inner.height as usize;

            let mut buf = vec![vec![' '; width]; height];
            for (x, column) in spec.iter().rev().take(width).enumerate() {
                for (y, &val) in column.iter().enumerate() {
                    let intensity = ((val * 10.0) as u8).min(9);
                    if y < height {
                        buf[height - 1 - y][x] =
                            " .:-=+*#%@".chars().nth(intensity as usize).unwrap_or(' ');
                    }
                }
            }

            let mut text = String::new();
            for row in buf {
                text.push_str(&row.iter().collect::<String>());
                text.push('\n');
            }

            use ratatui::text::Text;
            use ratatui::widgets::Paragraph;
            let paragraph =
                Paragraph::new(Text::raw(text)).style(Style::default().fg(Color::Green));
            f.render_widget(paragraph, inner);
        })?;
    }

    // 終了処理
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sender: crossbeam_channel::Sender<Vec<f32>>,
) -> Result<cpal::Stream, anyhow::Error>
where
    T: cpal::Sample + cpal::SizedSample + Send + 'static + ToPrimitive,
{
    let err_fn = |err| eprintln!("Stream error: {}", err);
    let channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            let buffer: Vec<f32> = data.iter().map(|s| s.to_f32().unwrap_or(0.0)).collect();
            let mono: Vec<f32> = buffer.chunks(channels).map(|c| c[0]).collect();
            sender.send(mono).ok();
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}
