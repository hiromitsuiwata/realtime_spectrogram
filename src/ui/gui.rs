use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, TextureHandle};

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

        for x in 0..width {
            for y in 0..height {
                let val = spec[x][y].clamp(0.0, 2.0);
                let intensity = ((val / 2.0) * 255.0) as u8;
                image[(x, height - 1 - y)] = egui::Color32::from_rgb(intensity, intensity / 2, 0);
            }
        }

        let texture = ctx.load_texture(
            "spectrogram",
            image,
            egui::TextureOptions::NEAREST,
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.image((texture.id(), ui.available_size()));
        });

        ctx.request_repaint(); // 常に更新
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
