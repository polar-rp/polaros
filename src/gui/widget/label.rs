use alloc::string::String;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::Event;
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::font::{draw_text, text_width, CHAR_HEIGHT};

pub struct Label {
    id: u32,
    x: i16,
    y: i16,
    text: String,
    color: Option<u8>,
}

impl Label {
    pub fn new(id: u32, x: i16, y: i16, text: &str) -> Self {
        Label {
            id,
            x, y,
            text: String::from(text),
            color: None,
        }
    }

    pub fn with_color(mut self, color: u8) -> Self {
        self.color = Some(color);
        self
    }
}

impl Widget for Label {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, text_width(&self.text), CHAR_HEIGHT)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        let color = self.color.unwrap_or(theme.text_primary);
        draw_text(fb, self.x, self.y, &self.text, color);
    }

    fn handle_event(&mut self, _event: &Event) -> EventResponse {
        EventResponse::Ignored
    }
}
