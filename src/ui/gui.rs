use std::sync::{Arc, Mutex};
use eframe::egui::{self, Color32, ColorImage, TextureHandle};
use egui::load::SizedTexture;

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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("Sample rate: {}", self.sample_rate));

            if let Some(texture) = &self.texture {
                let texture2 = SizedTexture::new(texture, ui.available_size());
                ui.image(texture2);
            } else {
                ui.label("Spectrogram texture not created yet");
            }
        });
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
