use crate::gui::ui::widgets::selected_label::SelectableLabel2;
use egui::{pos2, vec2, Align, Id, Layout, Rect, ScrollArea, Sense, TextStyle, Ui};

pub fn tab_list<T: TabX, F1: FnMut(&mut T) -> String, F2: FnMut(&mut Ui, &mut T) -> Id>(
    ui: &mut Ui,
    tabs: &mut Vec<T>,
    current_tab: &mut usize,
    dragging_tab: &mut Option<usize>,
    mut name_func: F1,
    mut id_func: F2,
) -> TabListResponse {
    let mut close_tab = false;

    ui.with_layout(Layout::right_to_left().with_cross_align(Align::Min), |ui| {
        let mut tab_y = 0.0;
        let mut tab_height = 0.0;
        ui.add_enabled_ui(!tabs.is_empty(), |ui| {
            let res = ui.button("X");
            tab_y = res.rect.min.y;
            tab_height = res.rect.height();
            if res.clicked() {
                if *current_tab < tabs.len() {
                    close_tab = true;
                } else {
                    *current_tab = 0;
                }
            }
        });
        ui.with_layout(Layout::left_to_right(), |ui| {
            ScrollArea::horizontal()
                .always_show_scroll(true)
                .show(ui, |ui| {
                    let mut total_tab_x = 0.0;
                    let offset = ui.max_rect().left();
                    let item_spacing = ui.spacing().item_spacing.x;
                    let mut starting_dragging = false;

                    // render all the tabs
                    for (index, instance) in tabs.iter_mut().enumerate() {
                        let name = name_func(instance);

                        // get the tab size
                        let tab_padding = ui.ctx().style().spacing.button_padding;
                        let tab_extra = tab_padding + tab_padding;
                        let tab_galley = ui.ctx().fonts().layout_delayed_color(
                            name.to_string(),
                            TextStyle::Button,
                            f32::INFINITY,
                        );
                        let tab_size = tab_galley.size() + tab_extra;
                        let half_width = tab_size.x / 2.0;

                        // Add half width to tab_x so that our x position is for the center of the
                        // tab. This is so that the tab swapping checks compare the position of the
                        // center of the tab instead of its start.
                        let tab_x = total_tab_x + half_width;
                        total_tab_x += tab_size.x + item_spacing;

                        if *dragging_tab != Some(index) {
                            // if the currently rendered tab has a position, animate it toward the
                            // position it should have, otherwise just set it to the correct
                            // position
                            if let Some(cur_tab_x) = instance.tab_x() {
                                let diff = tab_x - cur_tab_x;
                                instance.set_tab_x(cur_tab_x + diff * 0.5);
                            } else {
                                instance.set_tab_x(tab_x);
                            }

                            // get a ui to contain the tab, specifically at the tab's
                            // current position
                            let mut ui = ui.child_ui(
                                Rect::from_min_size(
                                    pos2(instance.tab_x().unwrap() - half_width + offset, tab_y),
                                    tab_size,
                                ),
                                Layout::left_to_right(),
                            );

                            // get a unique label id
                            let label_id = id_func(&mut ui, instance);

                            // render the tab
                            let res = ui.add(
                                SelectableLabel2::new(*current_tab == index, &name)
                                    .id(label_id)
                                    .sense(Sense::click_and_drag()),
                            );

                            // handle tab clicking and dragging
                            if res.clicked() {
                                *current_tab = index;
                            } else if res.dragged() {
                                starting_dragging = true;
                                *dragging_tab = Some(index);
                                instance.set_tab_x(instance.tab_x().unwrap() + res.drag_delta().x);
                            }

                            // if the drag is released, then we're not dragging anything
                            // anymore
                            if res.drag_released() {
                                // we have this here just in case something wonky happens
                                // between frames
                                *dragging_tab = None;
                            }
                        }
                    }

                    // render the dragged tab last so it appears on top
                    if dragging_tab.is_some() && !starting_dragging {
                        let index = dragging_tab.unwrap();
                        let instance = &mut tabs[index];
                        let name = name_func(instance);

                        // get the tab size
                        let tab_padding = ui.ctx().style().spacing.button_padding;
                        let tab_extra = tab_padding + tab_padding;
                        let tab_galley = ui.ctx().fonts().layout_delayed_color(
                            name.to_string(),
                            TextStyle::Button,
                            f32::INFINITY,
                        );
                        let tab_size = tab_galley.size() + tab_extra;

                        // get a ui to contain the tab, specifically at the tab's
                        // current position
                        let mut ui = ui.child_ui(
                            Rect::from_min_size(
                                pos2(instance.tab_x_or(0.0) - tab_size.x / 2.0 + offset, tab_y),
                                tab_size,
                            ),
                            Layout::left_to_right(),
                        );

                        // get a unique label id
                        let label_id = id_func(&mut ui, instance);

                        // render the tab
                        let res = ui.add(
                            SelectableLabel2::new(*current_tab == index, &name)
                                .id(label_id)
                                .always_draw_background(true)
                                .sense(Sense::click_and_drag()),
                        );

                        // handle tab clicking and dragging
                        if res.clicked() {
                            *current_tab = index;
                        } else if res.dragged() {
                            *dragging_tab = Some(index);
                            instance.set_tab_x(instance.tab_x_or(0.0) + res.drag_delta().x);
                        }

                        // if the drag is released, then we're not dragging anything
                        // anymore
                        if res.drag_released() {
                            *dragging_tab = None;
                        }
                    }

                    // make sure the scroll area is large enough
                    ui.allocate_space(vec2(total_tab_x, tab_height));

                    if let Some(drag_index) = &mut *dragging_tab {
                        // check if we need to move drag index up
                        while *drag_index < tabs.len() - 1
                            && tabs[*drag_index].tab_x_or(0.0) > tabs[*drag_index + 1].tab_x_or(0.0)
                        {
                            tabs.swap(*drag_index, *drag_index + 1);

                            if *current_tab == *drag_index {
                                *current_tab += 1;
                            } else if *current_tab == *drag_index + 1 {
                                *current_tab -= 1;
                            }

                            *drag_index += 1;
                        }

                        // check if we need to move drag index down
                        while *drag_index > 0
                            && tabs[*drag_index].tab_x_or(0.0) < tabs[*drag_index - 1].tab_x_or(0.0)
                        {
                            tabs.swap(*drag_index, *drag_index - 1);

                            if *current_tab == *drag_index {
                                *current_tab -= 1;
                            } else if *current_tab == *drag_index - 1 {
                                *current_tab += 1;
                            }

                            *drag_index -= 1;
                        }
                    }
                });
        });
    });

    TabListResponse { close_tab }
}

/// Response of a tab list widget.
pub struct TabListResponse {
    /// Whether the close tab button was clicked.
    pub close_tab: bool,
}

/// Something that can be used as a tab.
pub trait TabX {
    /// Gets the X position of the center of this tab if this tab has a
    /// position.
    fn tab_x(&self) -> Option<f32>;

    /// Gets the X position of the center of this tab or the default if this tab
    /// does not have a position.
    fn tab_x_or(&self, default: f32) -> f32 {
        self.tab_x().unwrap_or(default)
    }

    /// Sets the X position of the center of this tab.
    fn set_tab_x(&mut self, tab_x: f32);
}

/// Tab wrapper for data.
pub struct SimpleTab<T> {
    pub tab_x: Option<f32>,
    pub data: T,
}

impl<T> SimpleTab<T> {
    pub fn new(data: T) -> SimpleTab<T> {
        SimpleTab { tab_x: None, data }
    }
}

impl<T> TabX for SimpleTab<T> {
    fn tab_x(&self) -> Option<f32> {
        self.tab_x
    }

    fn set_tab_x(&mut self, tab_x: f32) {
        self.tab_x = Some(tab_x);
    }
}
