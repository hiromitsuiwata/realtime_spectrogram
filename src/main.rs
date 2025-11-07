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

const SAMPLE_RATE: usize = 44100; // サンプリングレート（未使用だが基準値として定義）
const FFT_SIZE: usize = 512; // FFTのサイズ（1フレームのサンプル数）
const SPEC_WIDTH: usize = 200; // スペクトログラムの横幅（時間方向のフレーム数）

fn main() -> anyhow::Result<()> {
    // === マイク入力デバイスの初期化 ===
    let host = cpal::default_host(); // ホスト（OS依存のオーディオドライバ管理）
    let device = host.default_input_device().expect("no input device"); // デフォルトの入力デバイス取得
    println!("使用デバイス: {}", device.name()?);
    let config = device.default_input_config()?; // 入力設定（サンプルレート・フォーマットなど）
    let sample_rate = config.sample_rate().0 as f32;

    // === 音声データをスレッド間で渡すチャンネルを作成 ===
    let (tx, rx) = unbounded::<Vec<f32>>();

    // === ストリームの構築（サンプル形式に応じて） ===
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), tx.clone())?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), tx.clone())?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), tx.clone())?,
        _ => panic!("unsupported format"),
    };
    stream.play()?; // マイク入力を開始

    // === スペクトログラムデータを共有するための構造 ===
    // 2次元配列 [時間][周波数] を保持する
    // Arc + Mutexで複数スレッドから安全にアクセスできるようにする
    let spectrogram = Arc::new(Mutex::new(vec![vec![0.0; FFT_SIZE / 2]; SPEC_WIDTH]));
    let spec_ref = Arc::clone(&spectrogram);

    // === FFTスレッド ===
    // 音声データを受信してリアルタイムにFFTを計算し、結果をスペクトログラムに保存する
    thread::spawn(move || {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE); // FFT計算器を準備
        let mut buffer = Vec::<f32>::new();

        for chunk in rx {
            buffer.extend(chunk); // 新しい音声データを追加
            while buffer.len() >= FFT_SIZE {
                // FFT_SIZE分のデータが溜まったら1フレーム処理
                let frame: Vec<f32> = buffer.drain(..FFT_SIZE).collect();

                // 複素数に変換してFFT実行
                let mut input: Vec<Complex<f32>> = frame
                    .into_iter()
                    .map(|x| Complex { re: x, im: 0.0 })
                    .collect();
                fft.process(&mut input);

                // FFT結果を振幅スペクトルに変換（対数スケールで強度を求める）
                let mags: Vec<f32> = input[..FFT_SIZE / 2]
                    .iter()
                    .map(|c| (c.norm() / (FFT_SIZE as f32)).log10().max(-2.0) + 2.0)
                    .collect();

                // スペクトログラム更新
                let mut spec = spec_ref.lock().unwrap();
                spec.pop(); // 一番右の列（古いデータ）を削除
                spec.insert(0, mags); // 左端に新しい列を追加（左→右に流れる表示にするなら逆に）
            }
        }
    });

    // === TUI描画 ===
    enable_raw_mode()?; // 入力を即時処理するモードに切り替え
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?; // 新しいスクリーンバッファへ
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // === メインループ（描画と入力処理） ===
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

            // === 周波数ラベルを生成 ===
            // 上が高周波、下が低周波
            let max_freq = sample_rate / 2.0; // ナイキスト周波数
            let mut text = String::new();
            for (i, row) in buf.iter().enumerate() {
                let freq = max_freq * (1.0 - i as f32 / height as f32);
                let label = if i % (height / 8).max(1) == 0 {
                    format!("{:>5.0}Hz | ", freq)
                } else {
                    "        | ".to_string()
                };
                text.push_str(&format!("{}{}\n", label, row.iter().collect::<String>()));
            }

            use ratatui::text::Text;
            use ratatui::widgets::Paragraph;
            let paragraph =
                Paragraph::new(Text::raw(text)).style(Style::default().fg(Color::Green));
            f.render_widget(paragraph, inner);
        })?;
    }

    // === 終了処理 ===
    disable_raw_mode()?; // 端末を元に戻す
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?; // 元の画面に戻す
    terminal.show_cursor()?; // カーソル再表示
    Ok(())
}

// === 音声ストリーム構築関数 ===
// CPALを使ってマイク入力を受け取り、サンプルをf32へ変換して送信する
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
            // サンプルをf32に変換（0.0〜1.0）
            let buffer: Vec<f32> = data.iter().map(|s| s.to_f32().unwrap_or(0.0)).collect();
            // ステレオなど複数チャンネルの場合、左チャンネルのみ使用
            let mono: Vec<f32> = buffer.chunks(channels).map(|c| c[0]).collect();
            sender.send(mono).ok(); // FFTスレッドへ送信
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}
