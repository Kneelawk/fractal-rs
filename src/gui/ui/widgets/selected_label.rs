#![allow(dead_code)]

use egui::{Id, NumExt, Response, Sense, TextStyle, Ui, Widget, WidgetInfo, WidgetType};

/// Slightly altered version of egui's `SelectableLabel`, allowing for different
/// `Sense`s.
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
#[derive(Debug)]
pub struct SelectableLabel2 {
    selected: bool,
    text: String,
    text_style: Option<TextStyle>,
    sense: Sense,
    always_draw_background: bool,
    override_id: Option<Id>,
}

impl SelectableLabel2 {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(selected: bool, text: impl ToString) -> Self {
        Self {
            selected,
            text: text.to_string(),
            text_style: None,
            sense: Sense::click(),
            always_draw_background: false,
            override_id: None,
        }
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = sense;
        self
    }

    pub fn always_draw_background(mut self, always_draw_background: bool) -> Self {
        self.always_draw_background = always_draw_background;
        self
    }

    pub fn id(mut self, id: Id) -> Self {
        self.override_id = Some(id);
        self
    }
}

impl Widget for SelectableLabel2 {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            selected,
            text,
            text_style,
            sense,
            always_draw_background,
            override_id,
        } = self;

        let id = override_id.unwrap_or_else(|| ui.make_persistent_id(&text));

        let text_style = text_style
            .or(ui.style().override_text_style)
            .unwrap_or(TextStyle::Button);

        let button_padding = ui.spacing().button_padding;
        let total_extra = button_padding + button_padding;

        let wrap_width = if ui.wrap_text() {
            ui.available_width() - total_extra.x
        } else {
            f32::INFINITY
        };

        let galley = ui
            .fonts()
            .layout_delayed_color(text, text_style, wrap_width);

        let mut desired_size = total_extra + galley.size();
        desired_size.y = desired_size.y.at_least(ui.spacing().interact_size.y);
        let (_, rect) = ui.allocate_space(desired_size);
        let response = ui.interact(rect, id, sense);
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, selected, galley.text())
        });

        let text_pos = ui
            .layout()
            .align_size_within_rect(galley.size(), rect.shrink2(button_padding))
            .min;

        let visuals = ui.style().interact_selectable(&response, selected);

        if always_draw_background || selected || response.hovered() || response.has_focus() {
            let rect = rect.expand(visuals.expansion);

            let corner_radius = 2.0;
            ui.painter()
                .rect(rect, corner_radius, visuals.bg_fill, visuals.bg_stroke);
        }

        let text_color = ui
            .style()
            .visuals
            .override_text_color
            .unwrap_or_else(|| visuals.text_color());
        ui.painter().galley_with_color(text_pos, galley, text_color);
        response
    }
}
