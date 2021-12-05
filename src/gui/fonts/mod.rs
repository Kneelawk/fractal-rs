use egui::{FontDefinitions, FontFamily};
use std::borrow::Cow;

pub fn font_definitions() -> FontDefinitions {
    let mut fonts: FontDefinitions = Default::default();
    fonts.font_data.insert(
        "SourceSansPro-Regular".to_string(),
        Cow::Borrowed(include_bytes!("SourceSansPro-Regular.ttf")),
    );
    fonts
        .fonts_for_family
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .push("SourceSansPro-Regular".to_string());

    fonts
}
