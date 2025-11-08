use crate::constants::FFT_SIZE;
use crossbeam_channel::Receiver;
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::{Arc, Mutex};

/// FFTスレッドを起動し、リアルタイムでスペクトログラムを更新
pub fn start_fft_thread(rx: Receiver<Vec<f32>>, spec_ref: Arc<Mutex<Vec<Vec<f32>>>>) {
    std::thread::spawn(move || {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use std::{thread, time::Duration};

    /// FFTスレッドが正しくスペクトログラムを更新するかを確認
    #[test]
    fn test_start_fft_thread_updates_spec_ref() {
        let spec_ref = Arc::new(Mutex::new(vec![vec![0.0; FFT_SIZE / 2]; 5]));
        let spec_clone = spec_ref.clone();
        let (tx, rx) = unbounded::<Vec<f32>>();

        // FFTスレッドを開始
        start_fft_thread(rx, spec_ref);

        // FFT_SIZE 分のデータを送信
        tx.send(vec![1.0; FFT_SIZE]).unwrap();

        // スレッドが処理を終えるまで待つ（送信側をすぐにdropしない）
        thread::sleep(Duration::from_millis(500));

        // ここで送信側をdrop → スレッド終了
        drop(tx);

        // 更新されたか確認
        let spec = spec_clone.lock().unwrap();
        let updated = spec.iter().any(|col| col.iter().any(|&x| x > 0.0));
        assert!(updated, "スペクトログラムが更新されていません");
    }

    /// 複数チャンクを処理できるか確認
    #[test]
    fn test_fft_thread_handles_multiple_chunks() {
        let spec_ref = Arc::new(Mutex::new(vec![vec![0.0; FFT_SIZE / 2]; 5]));
        let spec_clone = spec_ref.clone();
        let (tx, rx) = unbounded::<Vec<f32>>();

        start_fft_thread(rx, spec_ref);

        // 2回分送信
        tx.send(vec![0.5; FFT_SIZE]).unwrap();
        tx.send(vec![0.2; FFT_SIZE]).unwrap();

        // しばらく待つ（スレッドが処理完了するまで）
        thread::sleep(Duration::from_millis(800));
        drop(tx); // スレッド終了を促す

        let spec = spec_clone.lock().unwrap();
        let updated = spec.iter().any(|col| col.iter().any(|&x| x > 0.0));
        assert!(updated, "複数チャンクの処理が行われていません");
    }
}
