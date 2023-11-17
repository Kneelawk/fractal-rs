#![allow(dead_code)]

use egui::{
    Id, NumExt, Response, Sense, TextStyle, Ui, Widget, WidgetInfo, WidgetText, WidgetType,
};

/// Slightly altered version of egui's `SelectableLabel`, allowing for different
/// `Sense`s.
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct SelectableLabel2 {
    selected: bool,
    text: WidgetText,
    sense: Sense,
    always_draw_background: bool,
    override_id: Option<Id>,
}

impl SelectableLabel2 {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(selected: bool, text: impl Into<WidgetText>) -> Self {
        Self {
            selected,
            text: text.into(),
            sense: Sense::click(),
            always_draw_background: false,
            override_id: None,
        }
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
            sense,
            always_draw_background,
            override_id,
        } = self;

        let id = override_id.unwrap_or_else(|| ui.make_persistent_id(text.text()));

        let button_padding = ui.spacing().button_padding;
        let total_extra = button_padding + button_padding;

        let wrap_width = ui.available_width() - total_extra.x;
        let text = text.into_galley(ui, None, wrap_width, TextStyle::Button);

        let mut desired_size = total_extra + text.size();
        desired_size.y = desired_size.y.at_least(ui.spacing().interact_size.y);
        let (_extra_id, rect) = ui.allocate_space(desired_size);
        let response = ui.interact(rect, id, sense);
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, selected, text.text())
        });

        if ui.is_rect_visible(response.rect) {
            let text_pos = ui
                .layout()
                .align_size_within_rect(text.size(), rect.shrink2(button_padding))
                .min;

            let visuals = ui.style().interact_selectable(&response, selected);

            if always_draw_background
                || selected
                || response.hovered()
                || response.highlighted()
                || response.has_focus()
            {
                let rect = rect.expand(visuals.expansion);

                ui.painter().rect(
                    rect,
                    visuals.rounding,
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                );
            }

            text.paint_with_visuals(ui.painter(), text_pos, &visuals);
        }

        response
    }
}
