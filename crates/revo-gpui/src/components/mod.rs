pub mod home;
pub mod modal;
pub mod modal_shell;
pub mod quick_modal;

use gpui::{div, prelude::*, Div};

use crate::theme::RevoTheme;

pub(crate) fn panel(theme: RevoTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .rounded_lg()
        .text_color(theme.text)
}
