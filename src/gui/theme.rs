use super::palette;

pub struct Theme {
    pub desktop_bg: u8,
    pub window_bg: u8,
    pub title_bg: u8,
    pub title_text: u8,
    pub button_bg: u8,
    pub button_hover: u8,
    pub button_pressed: u8,
    pub button_text: u8,
    pub text_primary: u8,
    pub text_muted: u8,
    pub text_bright: u8,
    pub border: u8,
    pub close_btn: u8,
    pub taskbar_bg: u8,
    pub taskbar_text: u8,
    pub title_height: u16,
    pub taskbar_height: u16,
    pub button_padding: u16,
    pub border_width: u16,
}

pub fn default_dark_theme() -> Theme {
    Theme {
        desktop_bg: palette::BG_DARK,
        window_bg: palette::BG_BASE,
        title_bg: palette::BG_SURFACE,
        title_text: palette::TEXT_BRIGHT,
        button_bg: palette::ACCENT_BLUE,
        button_hover: palette::ACCENT_HOVER,
        button_pressed: palette::BG_HIGHLIGHT,
        button_text: palette::TEXT_BRIGHT,
        text_primary: palette::TEXT_PRIMARY,
        text_muted: palette::TEXT_MUTED,
        text_bright: palette::TEXT_BRIGHT,
        border: palette::BORDER,
        close_btn: palette::ERROR,
        taskbar_bg: palette::BG_SURFACE,
        taskbar_text: palette::TEXT_PRIMARY,
        title_height: 12,
        taskbar_height: 14,
        button_padding: 4,
        border_width: 1,
    }
}
