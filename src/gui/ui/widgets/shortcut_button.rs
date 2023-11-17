//! This module contains the [`ShortcutButton`].
//!
//! Most of this code has been copied from [`egui::widgets::button`].

#![allow(dead_code)]

use egui::{
    epaint, pos2, vec2, Color32, NumExt, Response, Sense, Shape, Stroke, TextStyle, Ui, Vec2,
    Widget, WidgetInfo, WidgetType,
};

/// Clickable button with text and shortcut text.
///
/// Almost all of this was copied from [`egui::widgets::button::Button`].
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct ShortcutButton {
    text: String,
    shortcut_text: String,
    text_color: Option<Color32>,
    text_style: Option<TextStyle>,
    shortcut_color: Option<Color32>,
    shortcut_style: Option<TextStyle>,
    /// None means default for interact
    fill: Option<Color32>,
    stroke: Option<Stroke>,
    sense: Sense,
    small: bool,
    frame: Option<bool>,
    wrap: Option<bool>,
    min_size: Vec2,
}

impl ShortcutButton {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(text: impl ToString, shortcut_text: impl ToString) -> Self {
        Self {
            text: text.to_string(),
            shortcut_text: shortcut_text.to_string(),
            text_color: None,
            text_style: None,
            shortcut_color: None,
            shortcut_style: None,
            fill: None,
            stroke: None,
            sense: Sense::click(),
            small: false,
            frame: None,
            wrap: None,
            min_size: Vec2::ZERO,
        }
    }

    pub fn text_color(mut self, text_color: Color32) -> Self {
        self.text_color = Some(text_color);
        self
    }

    pub fn text_color_opt(mut self, text_color: Option<Color32>) -> Self {
        self.text_color = text_color;
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn shortcut_color(mut self, shortcut_color: Color32) -> Self {
        self.shortcut_color = Some(shortcut_color);
        self
    }

    pub fn shortcut_style(mut self, shortcut_style: TextStyle) -> Self {
        self.shortcut_style = Some(shortcut_style);
        self
    }

    /// Override background fill color. Note that this will override any
    /// on-hover effects. Calling this will also turn on the frame.
    pub fn fill(mut self, fill: impl Into<Color32>) -> Self {
        self.fill = Some(fill.into());
        self.frame = Some(true);
        self
    }

    /// Override button stroke. Note that this will override any on-hover
    /// effects. Calling this will also turn on the frame.
    pub fn stroke(mut self, stroke: impl Into<Stroke>) -> Self {
        self.stroke = Some(stroke.into());
        self.frame = Some(true);
        self
    }

    /// Make this a small button, suitable for embedding into text.
    pub fn small(mut self) -> Self {
        self.text_style = Some(TextStyle::Body);
        self.small = true;
        self
    }

    /// Turn off the frame
    pub fn frame(mut self, frame: bool) -> Self {
        self.frame = Some(frame);
        self
    }

    /// By default, buttons senses clicks.
    /// Change this to a drag-button with `Sense::drag()`.
    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = sense;
        self
    }

    /// If `true`, the text will wrap at the `max_width`.
    /// By default [`Self::wrap`] will be true in vertical layouts
    /// and horizontal layouts with wrapping,
    /// and false on non-wrapping horizontal layouts.
    ///
    /// Note that any `\n` in the button text will always produce a new line.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = Some(wrap);
        self
    }

    pub(crate) fn min_size(mut self, min_size: Vec2) -> Self {
        self.min_size = min_size;
        self
    }
}

impl Widget for ShortcutButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let ShortcutButton {
            text,
            shortcut_text,
            text_color,
            text_style,
            shortcut_color,
            shortcut_style,
            fill,
            stroke,
            sense,
            small,
            frame,
            wrap,
            min_size,
        } = self;

        let frame = frame.unwrap_or_else(|| ui.visuals().button_frame);

        let text_style = text_style
            .or(ui.style().override_text_style.clone())
            .unwrap_or(TextStyle::Button);
        let shortcut_style = shortcut_style.unwrap_or(text_style.clone());

        let shortcut_padding = if shortcut_text.is_empty() { 0.0 } else { 5.0 };

        let mut button_padding = ui.spacing().button_padding;
        if small {
            button_padding.y = 0.0;
        }
        let total_extra = button_padding + vec2(shortcut_padding, 0.0) + button_padding;

        let wrap = wrap.unwrap_or_else(|| ui.wrap_text());
        let shortcut_wrap_width = if wrap {
            ui.available_width() - total_extra.x
        } else {
            f32::INFINITY
        };
        let shortcut_galley = ui.fonts(|font| {
            font.layout_delayed_color(
                shortcut_text,
                shortcut_style.resolve(ui.style()),
                shortcut_wrap_width,
            )
        });
        let shortcut_size = shortcut_galley.size();

        let wrap_width = if wrap {
            ui.available_width() - total_extra.x - shortcut_size.x
        } else {
            f32::INFINITY
        };
        let galley = ui.fonts(|font| {
            font.layout_delayed_color(text, text_style.resolve(ui.style()), wrap_width)
        });
        let galley_size = galley.size();

        let mut desired_size = vec2(
            galley_size.x + shortcut_size.x,
            galley_size.y.max(shortcut_size.y),
        ) + total_extra;
        if !small {
            desired_size.y = desired_size.y.at_least(ui.spacing().interact_size.y);
        }
        desired_size = desired_size.at_least(min_size);

        let (rect, response) = ui.allocate_at_least(desired_size, sense);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, galley.text()));

        if ui.clip_rect().intersects(rect) {
            let visuals = ui.style().interact(&response);
            let mut text_rect = rect.shrink2(button_padding);
            let shortcut_pos = pos2(
                text_rect.right() - shortcut_size.x,
                text_rect.top() + (text_rect.height() - shortcut_size.y) / 2.0,
            );

            text_rect.max.x -= shortcut_size.x;
            let text_pos = ui
                .layout()
                .align_size_within_rect(galley_size, text_rect)
                .min;

            if frame {
                let fill = fill.unwrap_or(visuals.bg_fill);
                let stroke = stroke.unwrap_or(visuals.bg_stroke);
                ui.painter().rect(
                    rect.expand(visuals.expansion),
                    visuals.rounding,
                    fill,
                    stroke,
                );
            }

            let text_color = text_color
                .or(ui.visuals().override_text_color)
                .unwrap_or_else(|| visuals.text_color());
            let shortcut_color = shortcut_color.unwrap_or(text_color);
            ui.painter().galley_with_color(text_pos, galley, text_color);
            ui.painter()
                .galley_with_color(shortcut_pos, shortcut_galley, shortcut_color);
        }

        response
    }
}

/// Boolean on/off control with text label and shortcut text.
///
/// Almost all of this was copied from [`egui::widgets::button::Checkbox`].
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
#[derive(Debug)]
pub struct ShortcutCheckbox<'a> {
    checked: &'a mut bool,
    text: String,
    shortcut_text: String,
    text_color: Option<Color32>,
    text_style: Option<TextStyle>,
    shortcut_color: Option<Color32>,
    shortcut_style: Option<TextStyle>,
}

impl<'a> ShortcutCheckbox<'a> {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(checked: &'a mut bool, text: impl ToString, shortcut_text: impl ToString) -> Self {
        ShortcutCheckbox {
            checked,
            text: text.to_string(),
            shortcut_text: shortcut_text.to_string(),
            text_color: None,
            text_style: None,
            shortcut_color: None,
            shortcut_style: None,
        }
    }

    pub fn text_color(mut self, text_color: Color32) -> Self {
        self.text_color = Some(text_color);
        self
    }

    pub fn text_style(mut self, text_style: TextStyle) -> Self {
        self.text_style = Some(text_style);
        self
    }

    pub fn shortcut_color(mut self, shortcut_color: Color32) -> Self {
        self.shortcut_color = Some(shortcut_color);
        self
    }

    pub fn shortcut_style(mut self, shortcut_style: TextStyle) -> Self {
        self.shortcut_style = Some(shortcut_style);
        self
    }
}

impl<'a> Widget for ShortcutCheckbox<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let ShortcutCheckbox {
            checked,
            text,
            shortcut_text,
            text_color,
            text_style,
            shortcut_color,
            shortcut_style,
        } = self;

        let text_style = text_style
            .or(ui.style().override_text_style.clone())
            .unwrap_or(TextStyle::Button);
        let shortcut_style = shortcut_style.unwrap_or(text_style.clone());

        let spacing = ui.spacing();
        let icon_width = spacing.icon_width;
        let icon_spacing = spacing.icon_spacing;
        let button_padding = spacing.button_padding;
        let shortcut_padding = if shortcut_text.is_empty() { 0.0 } else { 5.0 };
        let total_extra = button_padding
            + vec2(icon_width + icon_spacing + shortcut_padding, 0.0)
            + button_padding;

        let wrap = ui.wrap_text();
        let shortcut_wrap_width = if wrap {
            ui.available_width() - total_extra.x
        } else {
            f32::INFINITY
        };
        let shortcut_galley = ui.fonts(|font| {
            font.layout_delayed_color(
                shortcut_text,
                shortcut_style.resolve(ui.style()),
                shortcut_wrap_width,
            )
        });
        let shortcut_size = shortcut_galley.size();

        let wrap_width = if wrap {
            ui.available_width() - total_extra.x - shortcut_size.x
        } else {
            f32::INFINITY
        };
        let galley = ui.fonts(|font| {
            font.layout_delayed_color(text, text_style.resolve(ui.style()), wrap_width)
        });
        let galley_size = galley.size();

        let mut desired_size = vec2(
            galley_size.x + shortcut_size.x,
            galley_size.y.max(shortcut_size.y),
        ) + total_extra;
        desired_size = desired_size.at_least(spacing.interact_size);
        desired_size.y = desired_size.y.max(icon_width);
        let (rect, mut response) = ui.allocate_at_least(desired_size, Sense::click());

        if response.clicked() {
            *checked = !*checked;
            response.mark_changed();
        }
        response
            .widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, *checked, galley.text()));

        // let visuals = ui.style().interact_selectable(&response, *checked); // too
        // colorful
        let visuals = ui.style().interact(&response);
        let shortcut_pos = pos2(
            rect.max.x - button_padding.x - shortcut_size.x,
            rect.center().y - 0.5 * shortcut_size.y,
        );
        let text_pos = pos2(
            rect.min.x + button_padding.x + icon_width + icon_spacing,
            rect.center().y - 0.5 * galley_size.y,
        );
        let (small_icon_rect, big_icon_rect) = ui.spacing().icon_rectangles(rect);
        ui.painter().add(epaint::RectShape::new(
            big_icon_rect.expand(visuals.expansion),
            visuals.rounding,
            visuals.bg_fill,
            visuals.bg_stroke,
        ));

        if *checked {
            // Check mark:
            ui.painter().add(Shape::line(
                vec![
                    pos2(small_icon_rect.left(), small_icon_rect.center().y),
                    pos2(small_icon_rect.center().x, small_icon_rect.bottom()),
                    pos2(small_icon_rect.right(), small_icon_rect.top()),
                ],
                visuals.fg_stroke,
            ));
        }

        let text_color = text_color
            .or(ui.visuals().override_text_color)
            .unwrap_or_else(|| visuals.text_color());
        let shortcut_color = shortcut_color.unwrap_or(text_color);
        ui.painter().galley_with_color(text_pos, galley, text_color);
        ui.painter()
            .galley_with_color(shortcut_pos, shortcut_galley, shortcut_color);
        response
    }
}
