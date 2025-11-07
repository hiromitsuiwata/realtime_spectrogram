// === ライブラリのインポート ===
// 並行処理・同期に使う仕組み（Arc, Mutex, threadなど）
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// 音声入力用ライブラリ（cpal）
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

// スレッド間通信に使うチャンネル
use crossbeam_channel::unbounded;

// 端末制御ライブラリ（CUI操作）
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

// 数値型変換（to_f32 など）
use num_traits::ToPrimitive;

// 文字描画・スタイル制御
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{
    backend::CrosstermBackend,
    style::{Color, Style},
    widgets::{Block, Borders},
    Terminal,
};

// 高速フーリエ変換(FFT)ライブラリ
use rustfft::{num_complex::Complex, FftPlanner};

// === 定数設定 ===
const FFT_SIZE: usize = 512; // FFTで使うサンプル数（1回の解析に使う点数）
const SPEC_WIDTH: usize = 200; // スペクトログラムの横幅（時間方向のフレーム数）

fn main() -> anyhow::Result<()> {
    // === 音声入力デバイスの初期化 ===
    let host = cpal::default_host(); // OS依存のホスト（WindowsならWASAPIなど）
    let device = host.default_input_device().expect("no input device"); // デフォルトマイクを取得
    println!("使用デバイス: {}", device.name()?);
    let config = device.default_input_config()?; // サンプリング設定を取得
    let sample_rate = config.sample_rate().0 as f32; // サンプリングレート(Hz)

    // === 音声データをスレッド間でやり取りするためのチャンネルを作成 ===
    let (tx, rx) = unbounded::<Vec<f32>>(); // tx: 送信側, rx: 受信側

    // === ストリーム構築 ===
    // マイクから音声を取得し、サンプルをf32形式に変換して送信
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), tx.clone())?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), tx.clone())?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), tx.clone())?,
        _ => panic!("unsupported format"),
    };
    stream.play()?; // マイク入力開始

    // === スペクトログラムを保持する共有データ構造 ===
    // 2次元配列 [時間][周波数] 形式
    // Arc+Mutexで複数スレッドから安全に読み書きできるようにする
    let spectrogram = Arc::new(Mutex::new(vec![vec![0.0; FFT_SIZE / 2]; SPEC_WIDTH]));
    let spec_ref = Arc::clone(&spectrogram);

    // === FFT処理スレッド ===
    // 音声データを受信してリアルタイムにFFTを行い、スペクトログラムを更新
    thread::spawn(move || {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE); // FFT実行器を準備
        let mut buffer = Vec::<f32>::new(); // 音声バッファ

        // 音声チャンクを受け取り続けるループ
        for chunk in rx {
            buffer.extend(chunk); // 新しいデータを追加
            while buffer.len() >= FFT_SIZE {
                // FFT_SIZE分たまったら1フレーム分処理
                let frame: Vec<f32> = buffer.drain(..FFT_SIZE).collect();

                // FFT入力データを複素数に変換（実部=音声値, 虚部=0）
                let mut input: Vec<Complex<f32>> =
                    frame.into_iter().map(|x| Complex { re: x, im: 0.0 }).collect();
                fft.process(&mut input); // FFT実行

                // 振幅スペクトルに変換（対数スケールで強度計算）
                let mags: Vec<f32> = input[..FFT_SIZE / 2]
                    .iter()
                    .map(|c| (c.norm() / (FFT_SIZE as f32)).log10().max(-2.0) + 2.0)
                    .collect();

                // スペクトログラム更新（最新データを左端に挿入）
                let mut spec = spec_ref.lock().unwrap();
                spec.pop(); // 一番右の古い列を削除
                spec.insert(0, mags); // 新しい列を左端に追加（左へ流れる）
            }
        }
    });

    // === 端末描画準備 ===
    enable_raw_mode()?; // 標準入力を「即時入力モード」に
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?; // 別画面バッファに切り替え
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?; // 初期化

    // === メインループ（描画と入力処理） ===
    loop {
        // キー入力チェック
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break; // qキーで終了
                }
            }
        }

        // スペクトログラムデータを取得
        let spec = spectrogram.lock().unwrap().clone();

        // 描画
        terminal.draw(|f| {
            let size = f.area();
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Spectrogram (press 'q' to quit, log scale)");
            f.render_widget(&block, size);
            let inner = block.inner(size);

            let width = inner.width.min(SPEC_WIDTH as u16) as usize;
            let height = inner.height as usize;

            // === 対数スケール設定 ===
            let f_min: f32 = 20.0; // 最低周波数（人間の可聴域下限）
            let f_max: f32 = sample_rate as f32 / 2.0; // ナイキスト周波数
            let log_min = f_min.log10();
            let log_max = f_max.log10();

            // === 強度に応じた色付け関数 ===
            fn intensity_color(val: f32) -> Color {
                if val < 0.3 {
                    Color::Blue
                } else if val < 0.4 {
                    Color::Cyan
                } else if val < 0.6 {
                    Color::Green
                } else if val < 0.8 {
                    Color::Yellow
                } else {
                    Color::Red
                }
            }

            // === 行（周波数）ごとに描画 ===
            let mut lines: Vec<Line> = Vec::new();
            for row in 0..height {
                // 対数スケールで周波数を算出
                let frac = 1.0 - row as f32 / height as f32;
                let freq = 10f32.powf(log_min + frac * (log_max - log_min));

                // 周波数ラベル（Hz単位）
                let label = if row % (height / 8).max(1) == 0 {
                    format!("{:>6.0}Hz | ", freq)
                } else {
                    "         | ".to_string()
                };

                let mut spans: Vec<Span> = vec![Span::raw(label)];

                // 各列（時間軸方向）を描画
                for (_x, column) in spec.iter().rev().take(width).enumerate() {
                    // rowに対応するFFTインデックスを求める
                    let target_freq = freq;
                    let fft_index =
                        ((target_freq / f_max) * (FFT_SIZE as f32 / 2.0)).round() as usize;
                    if fft_index < column.len() {
                        let val = column[fft_index];
                        // 強度に応じて文字を選ぶ
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

            // 1つのParagraph（テキストブロック）として描画
            let paragraph = Paragraph::new(lines);
            f.render_widget(paragraph, inner);
        })?;
    }

    // === 終了処理 ===
    disable_raw_mode()?; // 標準入力モードを戻す
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?; // 元の画面へ戻す
    terminal.show_cursor()?; // カーソルを再表示
    Ok(())
}

// === 音声入力ストリーム構築関数 ===
// CPALを使ってマイクからデータを受け取り、f32配列として送信する
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sender: crossbeam_channel::Sender<Vec<f32>>,
) -> Result<cpal::Stream, anyhow::Error>
where
    // ジェネリクスTは、i16, u16, f32などのサンプル型を想定
    T: cpal::Sample + cpal::SizedSample + Send + 'static + ToPrimitive,
{
    // エラー時に呼ばれるコールバック
    let err_fn = |err| eprintln!("Stream error: {}", err);
    let channels = config.channels as usize; // チャンネル数（モノラル=1, ステレオ=2など）

    // 入力ストリーム作成
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            // サンプルをf32に変換
            let buffer: Vec<f32> = data.iter().map(|s| s.to_f32().unwrap_or(0.0)).collect();

            // ステレオの場合は左チャンネルだけを使う
            let mono: Vec<f32> = buffer.chunks(channels).map(|c| c[0]).collect();

            // 変換後の音声データを送信
            sender.send(mono).ok();
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}
