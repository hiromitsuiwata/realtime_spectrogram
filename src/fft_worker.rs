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
