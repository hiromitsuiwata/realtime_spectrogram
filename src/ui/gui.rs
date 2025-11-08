use std::sync::{Arc, Mutex};
use eframe::egui;

/// GUIモードでのターミナル描画を行う
pub fn run_ui(sample_rate: f32, spectrogram: Arc<Mutex<Vec<Vec<f32>>>>) -> anyhow::Result<()> {
    println!("GUIモードで起動します。");
    let options = eframe::NativeOptions::default();


    // Our application state:
    let mut name = "Arthur".to_owned();
    let mut age = 42;

    let _ = eframe::run_simple_native("My egui App", options, move |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut age, 0..=120).text("age"));
            if ui.button("Increment").clicked() {
                age += 1;
            }
            ui.label(format!("Hello '{name}', age {age}"));
        });
    });
    Ok(())
}