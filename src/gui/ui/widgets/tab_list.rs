use crate::gui::ui::widgets::selected_label::SelectableLabel2;
use egui::{pos2, vec2, Align, Layout, Rect, ScrollArea, Sense, TextStyle, Ui};

pub fn tab_list<T: TabX, F1: FnMut(&mut T) -> String>(
    ui: &mut Ui,
    instances: &mut Vec<T>,
    current_instance: &mut usize,
    dragging_instance: &mut Option<usize>,
    mut name_func: F1,
) {
    ui.with_layout(Layout::right_to_left().with_cross_align(Align::Min), |ui| {
        let mut tab_y = 0.0;
        let mut tab_height = 0.0;
        ui.add_enabled_ui(!instances.is_empty(), |ui| {
            let res = ui.button("X");
            tab_y = res.rect.min.y;
            tab_height = res.rect.height();
            if res.clicked() {
                if *current_instance < instances.len() {
                    instances.remove(*current_instance);
                    if *current_instance > 0 {
                        *current_instance -= 1;
                    }
                } else {
                    *current_instance = 0;
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
                    for (index, instance) in instances.iter_mut().enumerate() {
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

                        let tab_x = total_tab_x;
                        total_tab_x += tab_size.x + item_spacing;

                        if *dragging_instance != Some(index) {
                            // if the currently rendered instance is not the currently
                            // dragged instance, reset the instance's position
                            instance.set_tab_x(tab_x);

                            // get a ui to contain the tab, specifically at the tab's
                            // current position
                            let mut ui = ui.child_ui(
                                Rect::from_min_size(
                                    pos2(instance.tab_x() + offset, tab_y),
                                    tab_size,
                                ),
                                Layout::left_to_right(),
                            );

                            // render the tab
                            let res = ui.add(
                                SelectableLabel2::new(*current_instance == index, &name)
                                    .sense(Sense::click_and_drag()),
                            );

                            // handle tab clicking and dragging
                            if res.clicked() {
                                *current_instance = index;
                            } else if res.dragged() {
                                starting_dragging = true;
                                *dragging_instance = Some(index);
                                instance.set_tab_x(instance.tab_x() + res.drag_delta().x);
                            }

                            // if the drag is released, then we're not dragging anything
                            // anymore
                            if res.drag_released() {
                                // we have this here just in case something wonky happens
                                // between frames
                                *dragging_instance = None;
                            }
                        }
                    }

                    // render the dragged tab last so it appears on top
                    if dragging_instance.is_some() && !starting_dragging {
                        let index = dragging_instance.unwrap();
                        let instance = &mut instances[index];
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
                            Rect::from_min_size(pos2(instance.tab_x() + offset, tab_y), tab_size),
                            Layout::left_to_right(),
                        );

                        // render the tab
                        let res = ui.add(
                            SelectableLabel2::new(*current_instance == index, &name)
                                .sense(Sense::click_and_drag()),
                        );

                        // handle tab clicking and dragging
                        if res.clicked() {
                            *current_instance = index;
                        } else if res.dragged() {
                            *dragging_instance = Some(index);
                            instance.set_tab_x(instance.tab_x() + res.drag_delta().x);
                        }

                        // if the drag is released, then we're not dragging anything
                        // anymore
                        if res.drag_released() {
                            *dragging_instance = None;
                        }
                    }

                    // make sure the scroll area is large enough
                    ui.allocate_space(vec2(total_tab_x, tab_height));

                    if let Some(drag_index) = &mut *dragging_instance {
                        // check if we need to move drag index up
                        while *drag_index < instances.len() - 1
                            && instances[*drag_index].tab_x() > instances[*drag_index + 1].tab_x()
                        {
                            instances.swap(*drag_index, *drag_index + 1);

                            if *current_instance == *drag_index {
                                *current_instance += 1;
                            } else if *current_instance == *drag_index + 1 {
                                *current_instance -= 1;
                            }

                            *drag_index += 1;
                        }

                        // check if we need to move drag index down
                        while *drag_index > 0
                            && instances[*drag_index].tab_x() < instances[*drag_index - 1].tab_x()
                        {
                            instances.swap(*drag_index, *drag_index - 1);

                            if *current_instance == *drag_index {
                                *current_instance -= 1;
                            } else if *current_instance == *drag_index - 1 {
                                *current_instance += 1;
                            }

                            *drag_index -= 1;
                        }
                    }
                });
        });
    });
}

/// Something that can be used as a tab.
pub trait TabX {
    /// Gets the X position of this tab.
    fn tab_x(&self) -> f32;

    /// Sets the X position of this tab.
    fn set_tab_x(&mut self, tab_x: f32);
}

/// Tab wrapper for data.
pub struct SimpleTab<T> {
    pub tab_x: f32,
    pub data: T,
}

impl<T> SimpleTab<T> {
    pub fn new(data: T) -> SimpleTab<T> {
        SimpleTab { tab_x: 0.0, data }
    }
}

impl<T> TabX for SimpleTab<T> {
    fn tab_x(&self) -> f32 {
        self.tab_x
    }

    fn set_tab_x(&mut self, tab_x: f32) {
        self.tab_x = tab_x;
    }
}
