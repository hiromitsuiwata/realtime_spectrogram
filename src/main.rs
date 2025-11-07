use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::{
    io,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

fn main() -> anyhow::Result<()> {
    // === 1. 入力デバイスを取得 ===
    let host = cpal::default_host();
    let device = host
        .input_devices()?
        .find(|d| d.name().unwrap().contains("Realtek"))
        .expect("Could not find microphone device");
    println!("使用デバイス: {}", device.name()?);

    // === 2. 設定を取得 ===
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0 as usize;
    println!("サンプリングレート: {} Hz", sample_rate);

    // === 3. チャンネルを用意 ===
    let (tx, rx) = mpsc::channel::<Vec<f32>>();

    // === 4. ストリームを生成 ===
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), tx)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), tx)?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), tx)?,
        _ => panic!("Unsupported sample format"),
    };

    stream.play()?;
    println!("録音開始... [Q]キーで終了");

    // === 5. TUIセットアップ ===
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // === 6. メインループ ===
    loop {
        // キーイベントで終了
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }

        // 最新データを受信
        let mut latest_data = vec![];
        while let Ok(data) = rx.try_recv() {
            latest_data = data;
        }

        // 描画
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.size());

            let wave_str: String = latest_data
                .iter()
                .step_by(100) // 表示を間引く
                .map(|v| {
                    let height = ((*v * 10.0).round() as i32).clamp(-10, 10);
                    if height > 0 {
                        "▇"
                    } else if height < 0 {
                        "▂"
                    } else {
                        " "
                    }
                })
                .collect();

            let block = Block::default()
                .title("マイク入力波形（Qで終了）")
                .borders(Borders::ALL);
            let paragraph = Paragraph::new(wave_str)
                .block(block)
                .style(Style::default().fg(Color::Cyan));
            f.render_widget(paragraph, chunks[0]);
        })?;
    }

    // === 7. 終了処理 ===
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

// === ストリーム生成関数 ===
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: Sender<Vec<f32>>,
) -> Result<cpal::Stream, anyhow::Error>
where
    T: cpal::Sample + cpal::SizedSample + num_traits::ToPrimitive + Send + 'static,
{
    let channels = config.channels as usize;
    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // num_traits の ToPrimitive を使って f32 に変換
            let buffer: Vec<f32> = data
                .iter()
                .map(|x| x.to_f32().unwrap_or(0.0))
                .collect();
            let _ = tx.send(buffer);
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}
