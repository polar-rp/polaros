use spin::Mutex;
use x86_64::instructions::port::Port;
use crate::gui::event::{Event, MouseButton, EVENT_QUEUE};
use crate::gui::framebuffer::{SCREEN_WIDTH, SCREEN_HEIGHT};

static MOUSE_STATE: Mutex<MouseState> = Mutex::new(MouseState::new());

struct MouseState {
    cycle: u8,
    bytes: [u8; 3],
    x: i16,
    y: i16,
    prev_buttons: u8,
}

impl MouseState {
    const fn new() -> Self {
        MouseState {
            cycle: 0,
            bytes: [0; 3],
            x: 160,
            y: 100,
            prev_buttons: 0,
        }
    }
}

fn wait_input() {
    for _ in 0..100_000 {
        let mut status_port = Port::<u8>::new(0x64);
        let status: u8 = unsafe { status_port.read() };
        if status & 0x02 == 0 {
            return;
        }
    }
}

fn wait_output() {
    for _ in 0..100_000 {
        let mut status_port = Port::<u8>::new(0x64);
        let status: u8 = unsafe { status_port.read() };
        if status & 0x01 != 0 {
            return;
        }
    }
}

fn command(cmd: u8) {
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(0x64);
        command_port.write(0xD4);
    }
    wait_input();
    unsafe {
        let mut data_port = Port::<u8>::new(0x60);
        data_port.write(cmd);
    }
    wait_output();
    unsafe {
        let mut data_port = Port::<u8>::new(0x60);
        let _ack = data_port.read();
    }
}

pub fn init() {
    // Enable auxiliary device (mouse)
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(0x64);
        command_port.write(0xA8);
    }

    // Enable IRQ12 - read config byte
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(0x64);
        command_port.write(0x20);
    }
    wait_output();
    let mut config: u8;
    unsafe {
        let mut data_port = Port::<u8>::new(0x60);
        config = data_port.read();
    }
    config |= 0x02;
    config &= !0x20;
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(0x64);
        command_port.write(0x60);
    }
    wait_input();
    unsafe {
        let mut data_port = Port::<u8>::new(0x60);
        data_port.write(config);
    }

    command(0xF6);
    command(0xF4);
}

pub fn handle_byte(byte: u8) {
    let mut state = MOUSE_STATE.lock();

    match state.cycle {
        0 => {
            if byte & 0x08 != 0 {
                state.bytes[0] = byte;
                state.cycle = 1;
            }
        }
        1 => {
            state.bytes[1] = byte;
            state.cycle = 2;
        }
        2 => {
            state.bytes[2] = byte;
            state.cycle = 0;

            let flags = state.bytes[0];
            let mut dx = state.bytes[1] as i16;
            let mut dy = state.bytes[2] as i16;

            if flags & 0x10 != 0 { dx -= 256; }
            if flags & 0x20 != 0 { dy -= 256; }
            if flags & 0xC0 != 0 { return; }

            // Y inverted in PS/2
            dy = -dy;

            let old_x = state.x;
            let old_y = state.y;

            state.x = (state.x + dx).max(0).min(SCREEN_WIDTH as i16 - 1);
            state.y = (state.y + dy).max(0).min(SCREEN_HEIGHT as i16 - 1);

            let new_x = state.x;
            let new_y = state.y;

            // Mouse move
            if new_x != old_x || new_y != old_y {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseMove { x: new_x, y: new_y });
                }
            }

            // Button state transitions
            let buttons = flags & 0x07;
            let prev = state.prev_buttons;
            state.prev_buttons = buttons;

            // Left button
            if buttons & 0x01 != 0 && prev & 0x01 == 0 {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseDown { x: new_x, y: new_y, button: MouseButton::Left });
                }
            }
            if buttons & 0x01 == 0 && prev & 0x01 != 0 {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseUp { x: new_x, y: new_y, button: MouseButton::Left });
                }
            }

            // Right button
            if buttons & 0x02 != 0 && prev & 0x02 == 0 {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseDown { x: new_x, y: new_y, button: MouseButton::Right });
                }
            }
            if buttons & 0x02 == 0 && prev & 0x02 != 0 {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseUp { x: new_x, y: new_y, button: MouseButton::Right });
                }
            }
        }
        _ => {
            state.cycle = 0;
        }
    }
}
