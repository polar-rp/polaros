pub mod framebuffer;
pub mod primitives;
pub mod font;
pub mod palette;
pub mod event;
pub mod theme;
pub mod cursor;
pub mod widget;
pub mod compositor;
pub mod wallpaper;

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use alloc::boxed::Box;

use framebuffer::Framebuffer;
use theme::{default_dark_theme, Theme};
use cursor::Cursor;
use compositor::Compositor;
use event::{EVENT_QUEUE, Event};
use widget::desktop::Desktop;
use widget::window::Window;
use widget::terminal::TerminalWidget;

pub static GUI_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);
static NEXT_WIDGET_ID: AtomicU32 = AtomicU32::new(10);

pub fn next_id() -> u32 {
    NEXT_WIDGET_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn run() -> ! {
    crate::serial_println!("[GUI] Initializing...");

    // Load VGA palette
    palette::load_palette();
    crate::serial_println!("[GUI] Palette loaded");

    // Set GUI mode flag - this redirects keyboard events
    GUI_MODE_ACTIVE.store(true, Ordering::SeqCst);

    // Initialize mouse
    crate::drivers::mouse::init();
    crate::serial_println!("[GUI] Mouse initialized");

    let mut fb = Framebuffer::new();
    let theme = default_dark_theme();
    let mut compositor = Compositor::new();
    let mut cursor = Cursor::new(160, 100);

    // Add desktop with wallpaper (always layer 0)
    compositor.add_widget(Box::new(Desktop::new(0, &theme)));
    crate::serial_println!("[GUI] Desktop with wallpaper ready");

    // Add initial terminal window
    spawn_terminal_window(&mut compositor, &theme);

    crate::serial_println!("[GUI] Compositor ready, entering event loop");

    let mut frame_counter: u32 = 0;

    loop {
        // Process all pending events
        let mut need_new_terminal = false;
        loop {
            let event = {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.pop()
                } else {
                    None
                }
            };
            match event {
                Some(Event::MouseMove { x, y }) => {
                    cursor.x = x;
                    cursor.y = y;
                    compositor.handle_event(&Event::MouseMove { x, y });
                }
                Some(Event::MouseDown { x, y, button }) => {
                    if compositor.handle_event(&Event::MouseDown { x, y, button }) {
                        need_new_terminal = true;
                    }
                }
                Some(Event::MouseUp { x, y, button }) => {
                    compositor.handle_event(&Event::MouseUp { x, y, button });
                }
                Some(Event::KeyPress(key)) => {
                    compositor.handle_event(&Event::KeyPress(key));
                }
                Some(other) => {
                    compositor.handle_event(&other);
                }
                None => break,
            }
        }

        if need_new_terminal {
            spawn_terminal_window(&mut compositor, &theme);
        }

        // Render ~30fps: every 3 ticks at 100Hz timer
        frame_counter += 1;
        if frame_counter >= 3 {
            frame_counter = 0;

            // Send tick for cursor blink etc
            compositor.handle_event(&Event::Tick);

            // Render
            compositor.render(&mut fb, &theme, &cursor);
            fb.present();
        }

        x86_64::instructions::hlt();
    }
}

fn spawn_terminal_window(compositor: &mut Compositor, theme: &Theme) {
    let win_id = next_id();
    let term_id = next_id();

    // Offset each new window slightly
    let offset = ((win_id as i16 - 10) * 15) % 60;
    let x = 20 + offset;
    let y = 10 + offset;

    let mut win = Window::new(win_id, x, y, 280, 160, "Terminal");
    let cr = win.content_rect(theme);
    let terminal = TerminalWidget::new(term_id, cr.x, cr.y, cr.w, cr.h);
    win.set_content(Box::new(terminal));
    compositor.add_widget(Box::new(win));
}
