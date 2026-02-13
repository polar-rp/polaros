use spin::Mutex;
use x86_64::instructions::port::Port;
use crate::gui::event::{Event, MouseButton, EVENT_QUEUE};
use crate::gui::framebuffer::{SCREEN_WIDTH, SCREEN_HEIGHT};

const PS2_STATUS_PORT: u16 = 0x64;
const PS2_DATA_PORT: u16 = 0x60;
const PS2_CMD_ENABLE_AUX: u8 = 0xA8;
const PS2_CMD_READ_CONFIG: u8 = 0x20;
const PS2_CMD_WRITE_CONFIG: u8 = 0x60;
const PS2_CMD_WRITE_MOUSE: u8 = 0xD4;
const MOUSE_CMD_DEFAULTS: u8 = 0xF6;
const MOUSE_CMD_ENABLE: u8 = 0xF4;
const STATUS_INPUT_FULL: u8 = 0x02;
const STATUS_OUTPUT_FULL: u8 = 0x01;

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
        let mut status_port = Port::<u8>::new(PS2_STATUS_PORT);
        let status: u8 = unsafe { status_port.read() };
        if status & STATUS_INPUT_FULL == 0 {
            return;
        }
    }
}

fn wait_output() {
    for _ in 0..100_000 {
        let mut status_port = Port::<u8>::new(PS2_STATUS_PORT);
        let status: u8 = unsafe { status_port.read() };
        if status & STATUS_OUTPUT_FULL != 0 {
            return;
        }
    }
}

fn command(cmd: u8) {
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(PS2_STATUS_PORT);
        command_port.write(PS2_CMD_WRITE_MOUSE);
    }
    wait_input();
    unsafe {
        let mut data_port = Port::<u8>::new(PS2_DATA_PORT);
        data_port.write(cmd);
    }
    wait_output();
    unsafe {
        let mut data_port = Port::<u8>::new(PS2_DATA_PORT);
        let _ack = data_port.read();
    }
}

pub fn init() {
    // Enable auxiliary device (mouse)
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(PS2_STATUS_PORT);
        command_port.write(PS2_CMD_ENABLE_AUX);
    }

    // Enable IRQ12 - read config byte
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(PS2_STATUS_PORT);
        command_port.write(PS2_CMD_READ_CONFIG);
    }
    wait_output();
    let mut config: u8;
    unsafe {
        let mut data_port = Port::<u8>::new(PS2_DATA_PORT);
        config = data_port.read();
    }
    config |= 0x02;
    config &= !0x20;
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(PS2_STATUS_PORT);
        command_port.write(PS2_CMD_WRITE_CONFIG);
    }
    wait_input();
    unsafe {
        let mut data_port = Port::<u8>::new(PS2_DATA_PORT);
        data_port.write(config);
    }

    command(MOUSE_CMD_DEFAULTS);
    command(MOUSE_CMD_ENABLE);
}

fn emit_button_event(buttons: u8, prev: u8, mask: u8, button: MouseButton, x: i16, y: i16) {
    if buttons & mask != 0 && prev & mask == 0 {
        if let Some(mut queue) = EVENT_QUEUE.try_lock() {
            queue.push(Event::MouseDown { x, y, button });
        }
    }
    if buttons & mask == 0 && prev & mask != 0 {
        if let Some(mut queue) = EVENT_QUEUE.try_lock() {
            queue.push(Event::MouseUp { x, y, button });
        }
    }
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

            emit_button_event(buttons, prev, 0x01, MouseButton::Left, new_x, new_y);
            emit_button_event(buttons, prev, 0x02, MouseButton::Right, new_x, new_y);
        }
        _ => {
            state.cycle = 0;
        }
    }
}
