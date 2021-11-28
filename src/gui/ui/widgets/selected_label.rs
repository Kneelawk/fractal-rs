use egui::{NumExt, Response, Sense, TextStyle, Ui, Widget, WidgetInfo, WidgetType};

#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
#[derive(Debug)]
pub struct SelectableLabel2 {
    selected: bool,
    text: String,
    text_style: Option<TextStyle>,
    sense: Sense,
}

impl SelectableLabel2 {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(selected: bool, text: impl ToString) -> Self {
        Self {
            selected,
            text: text.to_string(),
            text_style: None,
            sense: Sense::click(),
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
}

impl Widget for SelectableLabel2 {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            selected,
            text,
            text_style,
            sense,
        } = self;

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
        let (rect, response) = ui.allocate_at_least(desired_size, sense);
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, selected, galley.text())
        });

        let text_pos = ui
            .layout()
            .align_size_within_rect(galley.size(), rect.shrink2(button_padding))
            .min;

        let visuals = ui.style().interact_selectable(&response, selected);

        if selected || response.hovered() || response.has_focus() {
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
