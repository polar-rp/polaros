use alloc::vec::Vec;
use alloc::boxed::Box;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::Event;
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::primitives::{fill_rect, draw_rect};

pub struct Panel {
    id: u32,
    x: i16,
    y: i16,
    w: u16,
    h: u16,
    bg_color: Option<u8>,
    has_border: bool,
    children: Vec<Box<dyn Widget>>,
}

impl Panel {
    pub fn new(id: u32, x: i16, y: i16, w: u16, h: u16) -> Self {
        Panel {
            id,
            x, y, w, h,
            bg_color: None,
            has_border: false,
            children: Vec::new(),
        }
    }

    pub fn with_bg(mut self, color: u8) -> Self {
        self.bg_color = Some(color);
        self
    }

    pub fn with_border(mut self) -> Self {
        self.has_border = true;
        self
    }

    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }
}

impl Widget for Panel {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        if let Some(bg) = self.bg_color {
            fill_rect(fb, self.x, self.y, self.w, self.h, bg);
        }
        if self.has_border {
            draw_rect(fb, self.x, self.y, self.w, self.h, theme.border);
        }
        for child in &mut self.children {
            child.render(fb, theme);
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        for child in self.children.iter_mut().rev() {
            let resp = child.handle_event(event);
            if resp != EventResponse::Ignored {
                return resp;
            }
        }
        EventResponse::Ignored
    }
}
