//! gui/mod.rs - This is where the GUI-based core application logic happens.

use crate::{
    gpu::GPUContext,
    gui::{
        flow::{Flow, FlowModel, FlowModelInit, FlowSignal},
        fonts::font_definitions,
        keyboard::{tracker::KeyboardTracker, ShortcutMap},
        storage::CfgUiSettings,
        ui::{FractalRSUI, UICreationContext, UIRenderContext, UIUpdateContext},
    },
    storage::{CfgGeneral, CfgSingleton},
};
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use wgpu::{Color, CommandBuffer, CommandEncoderDescriptor, TextureView};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    window::{Fullscreen, Window},
};

mod flow;
mod fonts;
mod keyboard;
mod storage;
mod ui;
mod util;

/// Launches the application as a GUI application.
pub fn start_gui_application() -> ! {
    CfgUiSettings::load().expect("Error loading ui settings config");

    let cfg = CfgUiSettings::read_clone();

    Flow::new()
        .width(cfg.initial_window_width)
        .height(cfg.initial_window_height)
        .fullscreen(cfg.start_fullscreen)
        .title("Fractal-RS 2")
        .start::<FractalRSGuiMain>()
        .expect("Error starting Flow!")
}

struct FractalRSGuiMain {
    // initialization state
    present: GPUContext,
    window: Arc<Window>,
    window_size: PhysicalSize<u32>,
    scale_factor: f64,
    platform: Platform,
    render_pass: RenderPass,
    keyboard_tracker: KeyboardTracker,
    shortcut_map: ShortcutMap,
    start_time: Instant,

    // running state
    commands: Vec<CommandBuffer>,
    ui: FractalRSUI,
}

impl FlowModel for FractalRSGuiMain {
    fn init(init: FlowModelInit) -> Self {
        let handle = init.handle;
        let present = init.present;
        let window = init.window;
        let window_size = init.window_size;
        let scale_factor = window.scale_factor();
        let frame_format = init.frame_format;

        info!("Setting up UI...");

        // Setup Egui
        let platform = Platform::new(PlatformDescriptor {
            physical_width: window_size.width,
            physical_height: window_size.height,
            scale_factor,
            font_definitions: font_definitions(),
            style: Default::default(),
        });

        let mut render_pass = RenderPass::new(&present.device, frame_format, 1);

        info!("Initializing UI State...");
        let ui = FractalRSUI::new(UICreationContext {
            instance: init.instance,
            handle: handle.clone(),
            present: present.clone(),
            render_pass: &mut render_pass,
        });

        let (shortcut_map, conflicts) = ShortcutMap::new();
        if !conflicts.conflicts.is_empty() {
            warn!("Shortcut conflicts: {}", conflicts);
        }

        FractalRSGuiMain {
            present,
            window,
            window_size,
            scale_factor,
            platform,
            render_pass,
            keyboard_tracker: KeyboardTracker::new(),
            shortcut_map,
            start_time: Instant::now(),
            commands: vec![],
            ui,
        }
    }

    fn event(&mut self, event: &WindowEvent<'_>) -> Option<FlowSignal> {
        if let WindowEvent::Resized(new_size) = event {
            self.window_size = *new_size;
        }

        if let WindowEvent::ScaleFactorChanged {
            new_inner_size,
            scale_factor,
        } = event
        {
            self.window_size = **new_inner_size;
            self.scale_factor = *scale_factor;
        }

        if let WindowEvent::KeyboardInput { input, .. } = event {
            self.keyboard_tracker.keyboard_input(input);
        }

        if let WindowEvent::ModifiersChanged(state) = event {
            self.keyboard_tracker.modifiers_changed(state);
        }

        None
    }

    fn all_events(&mut self, event: &Event<FlowSignal>) {
        self.platform.handle_event(event);
    }

    fn update(&mut self, _update_delta: Duration) -> Option<FlowSignal> {
        self.ui.update(&mut UIUpdateContext {
            render_pass: &mut self.render_pass,
        });

        if self.ui.close_requested {
            Some(FlowSignal::Exit)
        } else if self.ui.request_fullscreen != self.ui.previous_fullscreen {
            self.ui.previous_fullscreen = self.ui.request_fullscreen;
            if self.ui.request_fullscreen {
                Some(FlowSignal::Fullscreen(Some(Fullscreen::Borderless(None))))
            } else {
                Some(FlowSignal::Fullscreen(None))
            }
        } else {
            None
        }
    }

    fn render(&mut self, frame_view: &TextureView, _render_delta: Duration) {
        // Setup platform for frame
        self.platform
            .update_time(self.start_time.elapsed().as_secs_f64());

        // Draw UI
        self.platform.begin_frame();

        self.ui.draw(&UIRenderContext {
            ctx: &self.platform.context(),
            shortcuts: self
                .shortcut_map
                .lookup(self.keyboard_tracker.make_shortcuts()),
            window_size: self.window_size,
        });

        let (_output, paint_commands) = self.platform.end_frame(Some(&self.window));

        // Clear keyboard keypress events.
        self.keyboard_tracker.reset_keyboard_input();

        // Encode UI draw commands
        let paint_jobs = self.platform.context().tessellate(paint_commands);

        let screen_descriptor = ScreenDescriptor {
            physical_width: self.window_size.width,
            physical_height: self.window_size.height,
            scale_factor: self.scale_factor as f32,
        };
        self.render_pass.update_texture(
            &self.present.device,
            &self.present.queue,
            &self.platform.context().texture(),
        );
        self.render_pass
            .update_user_textures(&self.present.device, &self.present.queue);
        self.render_pass.update_buffers(
            &self.present.device,
            &self.present.queue,
            &paint_jobs,
            &screen_descriptor,
        );

        let mut encoder = self
            .present
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("UI Render Encoder"),
            });

        self.render_pass
            .execute(
                &mut encoder,
                frame_view,
                &paint_jobs,
                &screen_descriptor,
                Some(Color {
                    r: 0.1,
                    g: 0.1,
                    b: 0.1,
                    a: 1.0,
                }),
            )
            .unwrap();

        // Add UI render command to list of commands for this frame
        self.commands.push(encoder.finish());

        // Submit all commands encoded since the last frame
        self.present.queue.submit(self.commands.drain(..));
    }

    fn shutdown(self) {
        // Let's store our settings before we shut down.
        self.ui.store_settings();

        // Save the settings back to a file.
        let cfg_general_res = CfgGeneral::store();
        let cfg_ui_settings_res = CfgUiSettings::store();

        // Once we've attempted to save everything, we can start throwing errors.
        cfg_general_res.expect("Error saving general config");
        cfg_ui_settings_res.expect("Error saving ui config");
    }
}
