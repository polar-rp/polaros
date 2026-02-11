use alloc::string::String;
use alloc::boxed::Box;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::{Event, MouseButton};
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::primitives::{fill_rect, draw_rect};
use crate::gui::font::draw_text;

pub struct Window {
    id: u32,
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
    title: String,
    pub content: Option<Box<dyn Widget>>,
}

impl Window {
    pub fn new(id: u32, x: i16, y: i16, w: u16, h: u16, title: &str) -> Self {
        Window {
            id,
            x,
            y,
            w,
            h,
            title: String::from(title),
            content: None,
        }
    }

    pub fn set_content(&mut self, widget: Box<dyn Widget>) {
        self.content = Some(widget);
    }

    pub fn title_bar_rect(&self, theme: &Theme) -> Rect {
        Rect::new(self.x, self.y, self.w, theme.title_height)
    }

    pub fn close_button_rect(&self, theme: &Theme) -> Rect {
        let size = theme.title_height - 2;
        Rect::new(
            self.x + self.w as i16 - size as i16 - 1,
            self.y + 1,
            size,
            size,
        )
    }

    pub fn content_rect(&self, theme: &Theme) -> Rect {
        Rect::new(
            self.x + 1,
            self.y + theme.title_height as i16,
            self.w - 2,
            self.h - theme.title_height - 1,
        )
    }
}

impl Widget for Window {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        // Window border
        draw_rect(fb, self.x, self.y, self.w, self.h, theme.border);

        // Title bar
        fill_rect(fb, self.x + 1, self.y + 1, self.w - 2, theme.title_height - 1, theme.title_bg);

        // Title text
        draw_text(fb, self.x + 4, self.y + 2, &self.title, theme.title_text);

        // Close button
        let cb = self.close_button_rect(theme);
        fill_rect(fb, cb.x, cb.y, cb.w, cb.h, theme.close_btn);
        // Draw X
        draw_text(fb, cb.x + 1, cb.y + 1, "x", theme.text_bright);

        // Content area background
        let cr = self.content_rect(theme);
        fill_rect(fb, cr.x, cr.y, cr.w, cr.h, theme.window_bg);

        // Render content widget (reposition to content area)
        if let Some(ref mut content) = self.content {
            content.set_position(cr.x, cr.y);
            // Update terminal size to match content area
            if let Some(term) = content.as_terminal() {
                term.w = cr.w;
                term.h = cr.h;
            }
            content.render(fb, theme);
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseDown { x, y, button: MouseButton::Left } => {
                let theme = crate::gui::theme::default_dark_theme();
                let cb = self.close_button_rect(&theme);
                if cb.contains(*x, *y) {
                    return EventResponse::CloseRequest;
                }
                if self.bounds().contains(*x, *y) {
                    // Forward to content
                    if let Some(ref mut content) = self.content {
                        let resp = content.handle_event(event);
                        if resp != EventResponse::Ignored {
                            return resp;
                        }
                    }
                    return EventResponse::Consumed;
                }
                EventResponse::Ignored
            }
            Event::KeyPress(_) => {
                if let Some(ref mut content) = self.content {
                    return content.handle_event(event);
                }
                EventResponse::Ignored
            }
            _ => {
                if let Some(ref mut content) = self.content {
                    let resp = content.handle_event(event);
                    if resp != EventResponse::Ignored {
                        return resp;
                    }
                }
                EventResponse::Ignored
            }
        }
    }

    fn focusable(&self) -> bool { true }

    fn as_terminal(&mut self) -> Option<&mut super::terminal::TerminalWidget> {
        if let Some(ref mut content) = self.content {
            content.as_terminal()
        } else {
            None
        }
    }
}
