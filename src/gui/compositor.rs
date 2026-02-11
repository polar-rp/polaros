use alloc::vec::Vec;
use alloc::boxed::Box;
use super::widget::{Widget, EventResponse};
use super::event::{Event, MouseButton};
use super::framebuffer::Framebuffer;
use super::theme::Theme;
use super::cursor::Cursor;

struct DragState {
    widget_id: u32,
    offset_x: i16,
    offset_y: i16,
}

pub struct Compositor {
    layers: Vec<Box<dyn Widget>>,
    focused_id: Option<u32>,
    drag_state: Option<DragState>,
}

impl Compositor {
    pub fn new() -> Self {
        Compositor {
            layers: Vec::new(),
            focused_id: None,
            drag_state: None,
        }
    }

    pub fn add_widget(&mut self, widget: Box<dyn Widget>) {
        self.layers.push(widget);
    }

    pub fn remove_widget(&mut self, id: u32) {
        self.layers.retain(|w| w.id() != id);
        if self.focused_id == Some(id) {
            self.focused_id = None;
        }
        if let Some(ref drag) = self.drag_state {
            if drag.widget_id == id {
                self.drag_state = None;
            }
        }
    }

    fn raise_to_top(&mut self, id: u32) {
        if let Some(pos) = self.layers.iter().position(|w| w.id() == id) {
            if pos < self.layers.len() - 1 {
                let widget = self.layers.remove(pos);
                self.layers.push(widget);
            }
        }
    }

    fn hit_test(&self, x: i16, y: i16) -> Option<u32> {
        for layer in self.layers.iter().rev() {
            if layer.bounds().contains(x, y) {
                return Some(layer.id());
            }
        }
        None
    }

    /// Returns true if a new terminal should be spawned
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseDown { x, y, button: MouseButton::Left } => {
                if let Some(hit_id) = self.hit_test(*x, *y) {
                    // Raise and focus
                    if hit_id != 0 {
                        self.raise_to_top(hit_id);
                        self.focused_id = Some(hit_id);
                    }

                    // Check if on title bar for dragging (skip desktop id=0)
                    if hit_id != 0 {
                        let theme = crate::gui::theme::default_dark_theme();
                        if let Some(widget) = self.layers.iter().find(|w| w.id() == hit_id) {
                            let bounds = widget.bounds();
                            let title_bar = super::widget::Rect::new(
                                bounds.x, bounds.y, bounds.w, theme.title_height,
                            );
                            let close_rect = super::widget::Rect::new(
                                bounds.x + bounds.w as i16 - theme.title_height as i16 + 1,
                                bounds.y + 1,
                                theme.title_height - 2,
                                theme.title_height - 2,
                            );
                            if title_bar.contains(*x, *y) && !close_rect.contains(*x, *y) {
                                self.drag_state = Some(DragState {
                                    widget_id: hit_id,
                                    offset_x: *x - bounds.x,
                                    offset_y: *y - bounds.y,
                                });
                            }
                        }
                    }

                    // Forward event to widget
                    let mut close_id = None;
                    let mut open_terminal = false;
                    if let Some(widget) = self.layers.iter_mut().find(|w| w.id() == hit_id) {
                        let resp = widget.handle_event(event);
                        match resp {
                            EventResponse::CloseRequest => { close_id = Some(hit_id); }
                            EventResponse::OpenTerminal => { open_terminal = true; }
                            _ => {}
                        }
                    }
                    if let Some(id) = close_id {
                        self.remove_widget(id);
                    }
                    return open_terminal;
                }
                false
            }
            Event::MouseDown { .. } => { false }
            Event::MouseMove { x, y } => {
                if let Some(ref drag) = self.drag_state {
                    let new_x = *x - drag.offset_x;
                    let new_y = *y - drag.offset_y;
                    let drag_id = drag.widget_id;
                    if let Some(widget) = self.layers.iter_mut().find(|w| w.id() == drag_id) {
                        widget.set_position(new_x, new_y);
                    }
                }
                for layer in self.layers.iter_mut() {
                    layer.handle_event(event);
                }
                false
            }
            Event::MouseUp { .. } => {
                self.drag_state = None;
                for layer in self.layers.iter_mut() {
                    layer.handle_event(event);
                }
                false
            }
            Event::KeyPress(_) => {
                if let Some(fid) = self.focused_id {
                    if let Some(widget) = self.layers.iter_mut().find(|w| w.id() == fid) {
                        widget.handle_event(event);
                    }
                }
                false
            }
            Event::Tick => { false }
        }
    }

    pub fn render(&mut self, fb: &mut Framebuffer, theme: &Theme, cursor: &Cursor) {
        for layer in &mut self.layers {
            layer.render(fb, theme);
        }
        cursor.render(fb);
    }
}
