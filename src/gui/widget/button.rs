use alloc::string::String;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::{Event, MouseButton};
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::primitives::fill_rect;
use crate::gui::font::{draw_text, text_width};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonState {
    Normal,
    Hovered,
    Pressed,
}

pub struct Button {
    id: u32,
    x: i16,
    y: i16,
    w: u16,
    h: u16,
    label: String,
    state: ButtonState,
    pub on_click: Option<fn()>,
}

impl Button {
    pub fn new(id: u32, x: i16, y: i16, w: u16, h: u16, label: &str) -> Self {
        Button {
            id,
            x, y, w, h,
            label: String::from(label),
            state: ButtonState::Normal,
            on_click: None,
        }
    }
}

impl Widget for Button {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        let bg = match self.state {
            ButtonState::Normal => theme.button_bg,
            ButtonState::Hovered => theme.button_hover,
            ButtonState::Pressed => theme.button_pressed,
        };
        fill_rect(fb, self.x, self.y, self.w, self.h, bg);

        // Center label
        let tw = text_width(&self.label);
        let tx = self.x + (self.w as i16 - tw as i16) / 2;
        let ty = self.y + (self.h as i16 - 8) / 2;
        draw_text(fb, tx, ty, &self.label, theme.button_text);
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseMove { x, y } => {
                if self.bounds().contains(*x, *y) {
                    if self.state != ButtonState::Pressed {
                        self.state = ButtonState::Hovered;
                    }
                } else {
                    self.state = ButtonState::Normal;
                }
                EventResponse::Ignored
            }
            Event::MouseDown { x, y, button: MouseButton::Left } => {
                if self.bounds().contains(*x, *y) {
                    self.state = ButtonState::Pressed;
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            Event::MouseUp { x, y, button: MouseButton::Left } => {
                if self.state == ButtonState::Pressed && self.bounds().contains(*x, *y) {
                    self.state = ButtonState::Hovered;
                    if let Some(callback) = self.on_click {
                        callback();
                    }
                    EventResponse::Consumed
                } else {
                    self.state = ButtonState::Normal;
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn focusable(&self) -> bool { true }
}
