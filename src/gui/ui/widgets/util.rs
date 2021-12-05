/// Makes a shortcut button with the given text and shortcut, drawing from the
/// figen context.
macro_rules! shortcut_button {
    ($text:expr, $ctx:expr, $shortcut:tt) => {
        crate::gui::ui::widgets::shortcut_button::ShortcutButton::new(
            $text,
            $ctx.shortcuts
                .keys_for(crate::gui::keyboard::ShortcutName::$shortcut),
        )
    };
}
pub(crate) use shortcut_button;

macro_rules! shortcut_checkbox {
    ($cond:expr, $text:expr, $ctx:expr, $shortcut:tt) => {
        crate::gui::ui::widgets::shortcut_button::ShortcutCheckbox::new(
            $cond,
            $text,
            $ctx.shortcuts
                .keys_for(crate::gui::keyboard::ShortcutName::$shortcut),
        )
    };
}
pub(crate) use shortcut_checkbox;
