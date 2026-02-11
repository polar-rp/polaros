use super::framebuffer::Framebuffer;

pub fn fill_rect(fb: &mut Framebuffer, x: i16, y: i16, w: u16, h: u16, color: u8) {
    for dy in 0..h as i16 {
        for dx in 0..w as i16 {
            fb.set_pixel(x + dx, y + dy, color);
        }
    }
}

pub fn draw_rect(fb: &mut Framebuffer, x: i16, y: i16, w: u16, h: u16, color: u8) {
    draw_hline(fb, x, y, w, color);
    draw_hline(fb, x, y + h as i16 - 1, w, color);
    draw_vline(fb, x, y, h, color);
    draw_vline(fb, x + w as i16 - 1, y, h, color);
}

pub fn draw_hline(fb: &mut Framebuffer, x: i16, y: i16, w: u16, color: u8) {
    for dx in 0..w as i16 {
        fb.set_pixel(x + dx, y, color);
    }
}

pub fn draw_vline(fb: &mut Framebuffer, x: i16, y: i16, h: u16, color: u8) {
    for dy in 0..h as i16 {
        fb.set_pixel(x, y + dy, color);
    }
}

pub fn draw_line(fb: &mut Framebuffer, x0: i16, y0: i16, x1: i16, y1: i16, color: u8) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i16 = if x0 < x1 { 1 } else { -1 };
    let sy: i16 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        fb.set_pixel(x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}
