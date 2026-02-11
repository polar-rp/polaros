pub mod desktop;
pub mod window;
pub mod button;
pub mod label;
pub mod panel;
pub mod terminal;

use super::event::Event;
use super::framebuffer::Framebuffer;
use super::theme::Theme;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub fn new(x: i16, y: i16, w: u16, h: u16) -> Self {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, px: i16, py: i16) -> bool {
        px >= self.x && px < self.x + self.w as i16
            && py >= self.y && py < self.y + self.h as i16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResponse {
    Consumed,
    Ignored,
    CloseRequest,
    OpenTerminal,
}

pub trait Widget {
    fn id(&self) -> u32;
    fn bounds(&self) -> Rect;
    fn set_position(&mut self, x: i16, y: i16);
    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme);
    fn handle_event(&mut self, event: &Event) -> EventResponse;
    fn focusable(&self) -> bool { false }
    fn is_dirty(&self) -> bool { true }
    fn clear_dirty(&mut self) {}
    fn as_terminal(&mut self) -> Option<&mut terminal::TerminalWidget> { None }
}
