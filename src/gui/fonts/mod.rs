use egui::{FontData, FontDefinitions, FontFamily};
use std::borrow::Cow;

pub fn font_definitions() -> FontDefinitions {
    let mut fonts: FontDefinitions = Default::default();
    fonts.font_data.insert(
        "SourceSansPro-Regular".to_string(),
        FontData {
            font: Cow::Borrowed(include_bytes!("SourceSansPro-Regular.ttf")),
            index: 0,
            tweak: Default::default(),
        },
    );
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .push("SourceSansPro-Regular".to_string());

    fonts
}
