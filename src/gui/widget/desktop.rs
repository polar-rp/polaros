use alloc::vec::Vec;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::{Event, MouseButton};
use crate::gui::framebuffer::{Framebuffer, SCREEN_WIDTH, SCREEN_HEIGHT};
use crate::gui::theme::Theme;
use crate::gui::primitives::fill_rect;
use crate::gui::font::{draw_text_bg, draw_text};
use crate::gui::wallpaper;
use crate::kernel::timer;

const ICON_X: i16 = 8;
const ICON_Y: i16 = 8;
const ICON_W: u16 = 32;
const ICON_H: u16 = 28;

pub struct Desktop {
    id: u32,
    wallpaper: Vec<u8>,
    wallpaper_w: u16,
    wallpaper_h: u16,
}

impl Desktop {
    pub fn new(id: u32, theme: &Theme) -> Self {
        let wp_h = SCREEN_HEIGHT - theme.taskbar_height;
        let bg_color = 17u8;   // dark gray
        let logo_color = 20u8; // slightly lighter gray
        let wp = wallpaper::render_wallpaper(SCREEN_WIDTH, wp_h, bg_color, logo_color);
        Desktop {
            id,
            wallpaper: wp,
            wallpaper_w: SCREEN_WIDTH,
            wallpaper_h: wp_h,
        }
    }

    fn icon_rect(&self) -> Rect {
        Rect::new(ICON_X, ICON_Y, ICON_W, ICON_H)
    }
}

impl Widget for Desktop {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    fn set_position(&mut self, _x: i16, _y: i16) {}

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        // Blit pre-rendered wallpaper
        let w = self.wallpaper_w as usize;
        let h = self.wallpaper_h as usize;
        for y in 0..h {
            for x in 0..w {
                let color = self.wallpaper[y * w + x];
                fb.set_pixel(x as i16, y as i16, color);
            }
        }

        // Terminal icon
        let ir = self.icon_rect();
        fill_rect(fb, ir.x, ir.y, ir.w, ir.h, theme.title_bg);
        // Icon border
        crate::gui::primitives::draw_rect(fb, ir.x, ir.y, ir.w, ir.h, theme.border);
        // ">_" text
        draw_text(fb, ir.x + 4, ir.y + 4, ">_", theme.text_bright);
        // Label under icon
        draw_text(fb, ir.x - 4, ir.y + ir.h as i16 + 2, "Term", theme.text_primary);

        // Taskbar
        let tb_y = SCREEN_HEIGHT as i16 - theme.taskbar_height as i16;
        fill_rect(fb, 0, tb_y, SCREEN_WIDTH, theme.taskbar_height, theme.taskbar_bg);

        // "PolarOs" on the left
        draw_text_bg(fb, 4, tb_y + 3, "PolarOs", theme.text_bright, theme.taskbar_bg);

        // Clock on the right
        let ticks = timer::ticks();
        let secs = ticks / timer::TIMER_HZ as u64;
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;

        let mut buf = [0u8; 16];
        let time_str = format_time(&mut buf, h, m, s);
        let tw = time_str.len() as i16 * 8;
        draw_text_bg(fb, SCREEN_WIDTH as i16 - tw - 4, tb_y + 3, time_str, theme.taskbar_text, theme.taskbar_bg);
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseDown { x, y, button: MouseButton::Left } => {
                if self.icon_rect().contains(*x, *y) {
                    return EventResponse::OpenTerminal;
                }
                EventResponse::Ignored
            }
            _ => EventResponse::Ignored,
        }
    }
}

fn format_time<'a>(buf: &'a mut [u8; 16], h: u64, m: u64, s: u64) -> &'a str {
    use core::fmt::Write;
    struct BufWriter<'b> {
        buf: &'b mut [u8],
        pos: usize,
    }
    impl<'b> core::fmt::Write for BufWriter<'b> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for &byte in s.as_bytes() {
                if self.pos < self.buf.len() {
                    self.buf[self.pos] = byte;
                    self.pos += 1;
                }
            }
            Ok(())
        }
    }
    let mut w = BufWriter { buf, pos: 0 };
    let _ = write!(w, "{}:{:02}:{:02}", h, m, s);
    let len = w.pos;
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}
