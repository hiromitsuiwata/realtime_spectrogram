mod audio;
mod constants;
mod fft_worker;
mod ui;
mod util;

use audio::build_input_stream;
use constants::{FFT_SIZE, SPEC_WIDTH};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::unbounded;
use fft_worker::start_fft_thread;
use std::sync::{Arc, Mutex};

fn main() -> anyhow::Result<()> {
    // コマンドライン引数でUIモードを選択
    let args: Vec<String> = std::env::args().collect();
    let use_gui = args.iter().any(|a| a == "--gui");

    // === 音声デバイス初期化 ===
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    println!("使用デバイス: {}", device.name()?);
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0 as f32;

    // === チャンネル作成 ===
    let (tx, rx) = unbounded::<Vec<f32>>();

    // === ストリーム構築 ===
    println!("サンプルフォーマット: {:?}", config.sample_format());
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_input_stream::<f32>(&device, &config.into(), tx.clone())?,
        cpal::SampleFormat::I16 => build_input_stream::<i16>(&device, &config.into(), tx.clone())?,
        cpal::SampleFormat::U16 => build_input_stream::<u16>(&device, &config.into(), tx.clone())?,
        _ => panic!("unsupported format"),
    };
    stream.play()?;

    // === スペクトログラム共有領域 ===
    let spectrogram = Arc::new(Mutex::new(vec![vec![0.0; FFT_SIZE / 2]; SPEC_WIDTH]));

    // === FFTスレッド起動 ===
    start_fft_thread(rx, Arc::clone(&spectrogram));

    // === UI起動 ===
    if use_gui {
        println!("GUIモードで起動します。");
        ui::gui::run_ui(sample_rate, spectrogram)
    } else {
        println!("CLIモードで起動します。");
        ui::cli::run_ui(sample_rate, spectrogram)
    }
}
