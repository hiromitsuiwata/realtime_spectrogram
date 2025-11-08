use crate::constants::FFT_SIZE;
use eframe::egui::{self, Color32, TextureHandle};
use std::sync::{Arc, Mutex};

/// GUIモードでのターミナル描画を行う
pub fn run_ui(sample_rate: f32, spectrogram: Arc<Mutex<Vec<Vec<f32>>>>) -> anyhow::Result<()> {
    println!("GUIモードで起動します。");
    let options = eframe::NativeOptions::default();

    let _ = eframe::run_native(
        "Spectrogram Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(SpectrogramApp::new(sample_rate, spectrogram)))),
    );
    Ok(())
}

struct SpectrogramApp {
    sample_rate: f32,
    spectrogram: Arc<Mutex<Vec<Vec<f32>>>>,
    texture: Option<TextureHandle>,
}

impl eframe::App for SpectrogramApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let spec = self.spectrogram.lock().unwrap().clone();
        let width = spec.len();
        let height = spec[0].len();

        // 黒で初期化
        let pixels = vec![Color32::BLACK; width * height];
        let mut image = egui::ColorImage::new([width, height], pixels);

        let f_min: f32 = 20.0;
        let f_max = self.sample_rate / 2.0;
        let log_min = f_min.log10();
        let log_max = f_max.log10();

        for x in 0..width {
            let rev_x = width - 1 - x; // 左右反転
            for y in 0..height {
                let frac = 1.0 - (y as f32 / height as f32);
                let freq = 10f32.powf(log_min + frac * (log_max - log_min));
                let fft_index = ((freq / f_max) * (FFT_SIZE as f32 / 2.0)).round() as usize;

                let val = if fft_index < spec[rev_x].len() {
                    spec[rev_x][fft_index].clamp(0.0, 2.0)
                } else {
                    0.0
                };

                let intensity = ((val / 2.0) * 255.0) as u8;
                image[(x, height - 1 - y)] = egui::Color32::from_rgb(intensity, intensity / 2, 0);
            }
        }

        // 🟢 最初だけロードして、以降は更新
        if let Some(texture) = self.texture.as_mut() {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(ctx.load_texture("spectrogram", image, egui::TextureOptions::NEAREST));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.texture {
                ui.image((texture.id(), ui.available_size()));
            }
        });

        ctx.request_repaint();
    }
}

impl SpectrogramApp {
    fn new(sample_rate: f32, spectrogram: Arc<Mutex<Vec<Vec<f32>>>>) -> Self {
        Self {
            sample_rate: sample_rate,
            spectrogram: spectrogram,
            texture: None,
        }
    }
}
