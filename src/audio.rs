use anyhow::Result;
use cpal::traits::DeviceTrait;
use crossbeam_channel::Sender;
use num_traits::ToPrimitive;

/// 音声入力ストリームを構築する
pub fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sender: Sender<Vec<f32>>,
) -> Result<cpal::Stream>
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
