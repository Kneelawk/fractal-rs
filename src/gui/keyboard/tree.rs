//! This module contains the mechanism for representing a shortcut tree.

use crate::gui::keyboard::{ShortcutMap, ShortcutName};
use egui::Ui;
use std::{collections::HashMap, ops::Deref};
use strum::IntoEnumIterator;

lazy_static! {
    static ref TREE: ShortcutTreeNode = ShortcutTreeNode::generate();
}

/// Used to describe how a shortcut should be displayed.
pub enum ShortcutTreeNode {
    Parent {
        child_names: Vec<String>,
        children: HashMap<String, ShortcutTreeNode>,
    },
    Leaf {
        shortcut: ShortcutName,
    },
}

impl ShortcutTreeNode {
    fn generate() -> ShortcutTreeNode {
        let mut root = ShortcutTreeNode::Parent {
            child_names: vec![],
            children: Default::default(),
        };

        for shortcut in ShortcutName::iter() {
            let shortcut: ShortcutName = shortcut;
            let name = shortcut.to_string();

            let segments: Vec<_> = name.split('/').collect();
            let mut parent = &mut root;
            for (index, &segment) in segments.iter().enumerate() {
                if let ShortcutTreeNode::Parent {
                    child_names,
                    children,
                } = parent
                {
                    if !children.contains_key(segment) {
                        child_names.push(segment.to_string());
                        children.insert(
                            segment.to_string(),
                            if index == segments.len() - 1 {
                                ShortcutTreeNode::Leaf { shortcut }
                            } else {
                                ShortcutTreeNode::Parent {
                                    child_names: vec![],
                                    children: Default::default(),
                                }
                            },
                        );
                    }

                    parent = children.get_mut(segment).unwrap();
                }
            }
        }

        root
    }

    pub fn ui(ui: &mut Ui, shortcuts: &ShortcutMap, change_request: &mut Option<ShortcutName>) {
        if let ShortcutTreeNode::Parent {
            child_names,
            children,
        } = TREE.deref()
        {
            for child_name in child_names.iter() {
                let child = children.get(child_name).unwrap();
                if let ShortcutTreeNode::Parent {
                    child_names,
                    children,
                } = child
                {
                    Self::ui_impl(
                        ui,
                        child_names,
                        children,
                        child_name,
                        shortcuts,
                        change_request,
                    );
                } else {
                    ui.label(child_name);
                }
            }
        }
    }

    fn ui_impl(
        ui: &mut Ui,
        child_names: &[String],
        children: &HashMap<String, ShortcutTreeNode>,
        name: impl ToString,
        shortcuts: &ShortcutMap,
        change_request: &mut Option<ShortcutName>,
    ) {
        egui::CollapsingHeader::new(name.to_string())
            .default_open(false)
            .show(ui, |ui| {
                egui::Grid::new(
                    ui.make_persistent_id(format!("shortcut_tree.{}", name.to_string())),
                )
                .show(ui, |ui| {
                    for child_name in child_names.iter() {
                        let child = children.get(child_name).unwrap();
                        match child {
                            ShortcutTreeNode::Parent {
                                child_names,
                                children,
                            } => {
                                Self::ui_impl(
                                    ui,
                                    child_names,
                                    children,
                                    child_name,
                                    shortcuts,
                                    change_request,
                                );
                            },
                            ShortcutTreeNode::Leaf { shortcut } => {
                                ui.label(child_name);
                                if ui.button(shortcuts.keys_for(shortcut)).clicked() {
                                    *change_request = Some(*shortcut);
                                }
                            },
                        }
                        ui.end_row();
                    }
                });
            });
    }
}
