use ratatui::style::Color;

/// 振幅に応じた色を返す
pub fn intensity_color(val: f32) -> Color {
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
