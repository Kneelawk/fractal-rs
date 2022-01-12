use egui::Vec2;

/// Trait for converting things into `Vec2`s. I've done this enough times that
/// I'd like an extension method for it.
pub trait IntoVec2 {
    fn into_vec2(self) -> Vec2;
}

impl IntoVec2 for (isize, isize) {
    fn into_vec2(self) -> Vec2 {
        Vec2 {
            x: self.0 as f32,
            y: self.1 as f32,
        }
    }
}
