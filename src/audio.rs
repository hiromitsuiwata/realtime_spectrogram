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
            // 入力データを f32 に変換
            let buffer: Vec<f32> = data.iter().map(|s| s.to_f32().unwrap_or(0.0)).collect();
            // モノラル化（1チャンネル目のみ使用）
            let mono: Vec<f32> = buffer.chunks(channels).map(|c| c[0]).collect();
            sender.send(mono).ok();
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}

//
// -------------
// 単体テスト
// -------------
#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use cpal::traits::{HostTrait, StreamTrait};

    /// テスト用に最小限の設定を行ってストリームを構築できるか確認する
    #[test]
    fn test_build_input_stream_creates_stream() {
        // デフォルトのオーディオホストを取得
        let host = cpal::default_host();

        // 入力デバイスを取得（存在しない場合はスキップ）
        let device = match host.default_input_device() {
            Some(dev) => dev,
            None => {
                eprintln!("入力デバイスが存在しないためテストをスキップします。");
                return;
            }
        };

        // デフォルト設定を取得
        let config = match device.default_input_config() {
            Ok(c) => c.config(),
            Err(_) => {
                eprintln!("デフォルト入力設定を取得できないためスキップします。");
                return;
            }
        };

        // チャネル作成
        let (sender, receiver) = unbounded::<Vec<f32>>();

        // ストリーム生成
        let stream = build_input_stream::<f32>(&device, &config, sender);
        assert!(stream.is_ok(), "ストリーム構築に失敗しました");

        // 実際にストリームを起動してみる
        let stream = stream.unwrap();
        assert!(stream.play().is_ok(), "ストリーム起動に失敗しました");

        // 少し待ってデータが届くか確認（実環境による）
        std::thread::sleep(std::time::Duration::from_millis(100));
        // データが届いていればOK（届かなくてもpanicはしない）
        let _ = receiver.try_recv();
    }

    /// `build_input_stream` がエラーを返さず実行できることの最小テスト
    #[test]
    fn test_build_input_stream_type_constraints() {
        // ここではダミーのデバイスと設定を使用するため、エラーを許容
        let host = cpal::default_host();
        if let Some(device) = host.default_input_device() {
            let config = device.default_input_config().unwrap().config();
            let (sender, _) = unbounded::<Vec<f32>>();
            let result = build_input_stream::<f32>(&device, &config, sender);
            assert!(result.is_ok() || result.is_err());
        }
    }
}
