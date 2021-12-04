macro_rules! shortcut {
    ($($keys:tt)-+) => {
        shortcut!(@internal ($($keys)-+) => (false, false, false, false))
    };
    (@internal (Ctrl - $($keys:tt)-+) => ($ctrl:literal, $logo:literal, $shift:literal, $alt:literal)) => {
        shortcut!(@internal ($($keys)-+) => (true, $logo, $shift, $alt))
    };
    (@internal (Logo - $($keys:tt)-+) => ($ctrl:literal, $logo:literal, $shift:literal, $alt:literal)) => {
        shortcut!(@internal ($($keys)-+) => ($ctrl, true, $shift, $alt))
    };
    (@internal (Shift - $($keys:tt)-+) => ($ctrl:literal, $logo:literal, $shift:literal, $alt:literal)) => {
        shortcut!(@internal ($($keys)-+) => ($ctrl, $logo, true, $alt))
    };
    (@internal (Alt - $($keys:tt)-+) => ($ctrl:literal, $logo:literal, $shift:literal, $alt:literal)) => {
        shortcut!(@internal ($($keys)-+) => ($ctrl, $logo, $shift, true))
    };
    (@internal (MacAlt - $($keys:tt)-+) => ($ctrl:literal, $logo:literal, $shift:literal, $alt:literal)) => {{
        if cfg!(target_os = "macos") {
            shortcut!(@internal ($($keys)-+) => ($ctrl, true, $shift, true))
        } else {
            shortcut!(@internal ($($keys)-+) => ($ctrl, $logo, $shift, true))
        }
    }};
    (@internal (Cmd - $($keys:tt)-+) => ($ctrl:literal, $logo:literal, $shift:literal, $alt:literal)) => {{
        if cfg!(target_os = "macos") {
            shortcut!(@internal ($($keys)-+) => ($ctrl, true, $shift, $alt))
        } else {
            shortcut!(@internal ($($keys)-+) => (true, $logo, $shift, $alt))
        }
    }};
    (@internal ($key:tt) => ($ctrl:literal, $logo:literal, $shift:literal, $alt:literal)) => {
        crate::gui::keyboard::Shortcut {
            modifiers: crate::gui::keyboard::Modifiers {
                shift: $shift,
                ctrl: $ctrl,
                alt: $alt,
                logo: $logo,
            },
            key: winit::event::VirtualKeyCode::$key,
        }
    };
}
pub(crate) use shortcut;
