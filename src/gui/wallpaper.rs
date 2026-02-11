use alloc::vec::Vec;

const PATH1_D: &str = "M568.23,1442.47c-111.71,79.07-246.9,197.83-168.4,348.89-147.13-75.76-196.49-248.1-125.32-394.89,61.31-126.46,224.54-267.71,342.66-341.82l451.92-283.57c107.53-67.47,210.11-132.21,307.9-212.43,122.09-100.15,250.72-257.91,113.92-411.25,104.79,34.44,189.46,106.98,248.3,195.84,82.06,123.93,76.14,276.54-9.3,396.76-49.31,69.38-109.54,124.67-179.42,175.23-102.66,74.28-209.63,135.29-324.97,189.34l-352.45,165.18c-107.41,50.34-208.28,104.35-304.85,172.7Z";

const PATH2_D: &str = "M107.46,1483.1c-148.34-218.21-17.66-441.66,140.01-609.3,92.1-97.93,190.94-182.8,298.19-263.82l172.35-130.21c96.12-72.62,189.19-143.91,276.23-226.91,40.8-38.9,119.78-122.81,87.2-175.22-32.03-51.53-188.62-30.12-243.13-17.17C533.32,132.93,278.27,341.31,146.69,625.45c-30.63,66.15-54.03,130.08-72.21,200.52L5.9,1091.62C-37.76,699.99,164.24,326.37,502.93,132.98,715.67,11.52,966.51-30.18,1206.85,22.17c52.61,11.46,101.51,29.56,143.13,59.52,71.63,51.56,93.93,137.9,57.84,218.16-30.37,67.53-77.68,122.99-134.52,172.42-89.24,77.59-183.09,143.56-283.66,206.86l-328.28,206.61c-109.96,69.21-211.31,143.75-307.71,230.29-128.83,115.66-239.21,284.24-164.63,462.35-37.34-26.51-57.67-60.17-81.55-95.28Z";

const PATH3_D: &str = "M495.53,1582.27c8.16,96.48,127.55,166.89,207.35,197.62,193.88,74.65,406.81,73.11,603.92,5.02,349.82-120.84,588.65-443.5,614.7-812.28,7.93-112.22-2.33-218.31-26.03-328.2-7.52-34.85-16.3-69.48-15.61-104.25,136.12,242.62,155.4,527.88,63.78,787.39-138.34,391.9-512.22,659.4-927.6,669.11-144.9,3.39-282.22-24.41-411.27-86.01-57.64-27.51-107.05-63.61-145.01-114.12-52.98-70.5-34.97-166.8,35.76-214.27Z";

const SVG_W: f32 = 2000.0;
const SVG_H: f32 = 1997.0;
const BEZIER_STEPS: usize = 8;

fn parse_f32(bytes: &[u8]) -> f32 {
    let mut neg = false;
    let mut i = 0;
    if i < bytes.len() && bytes[i] == b'-' { neg = true; i += 1; }
    else if i < bytes.len() && bytes[i] == b'+' { i += 1; }

    let mut result: f32 = 0.0;
    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        result = result * 10.0 + (bytes[i] - b'0') as f32;
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let mut frac = 0.1f32;
        while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
            result += (bytes[i] - b'0') as f32 * frac;
            frac *= 0.1;
            i += 1;
        }
    }
    if neg { -result } else { result }
}

fn next_number(bytes: &[u8], pos: &mut usize) -> Option<f32> {
    while *pos < bytes.len() && (bytes[*pos] == b',' || bytes[*pos] == b' ') {
        *pos += 1;
    }
    if *pos >= bytes.len() { return None; }
    if bytes[*pos].is_ascii_alphabetic() { return None; }

    let start = *pos;
    if bytes[*pos] == b'-' || bytes[*pos] == b'+' {
        *pos += 1;
    }
    while *pos < bytes.len() && bytes[*pos] >= b'0' && bytes[*pos] <= b'9' {
        *pos += 1;
    }
    if *pos < bytes.len() && bytes[*pos] == b'.' {
        *pos += 1;
        while *pos < bytes.len() && bytes[*pos] >= b'0' && bytes[*pos] <= b'9' {
            *pos += 1;
        }
    }
    if *pos == start { return None; }
    Some(parse_f32(&bytes[start..*pos]))
}

fn read2(bytes: &[u8], pos: &mut usize) -> Option<(f32, f32)> {
    let a = next_number(bytes, pos)?;
    let b = next_number(bytes, pos)?;
    Some((a, b))
}

fn read6(bytes: &[u8], pos: &mut usize) -> Option<(f32, f32, f32, f32, f32, f32)> {
    let a = next_number(bytes, pos)?;
    let b = next_number(bytes, pos)?;
    let c = next_number(bytes, pos)?;
    let d = next_number(bytes, pos)?;
    let e = next_number(bytes, pos)?;
    let f = next_number(bytes, pos)?;
    Some((a, b, c, d, e, f))
}

fn cubic_bezier(t: f32, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) -> (f32, f32) {
    let u = 1.0 - t;
    let uu = u * u;
    let uuu = uu * u;
    let tt = t * t;
    let ttt = tt * t;
    (
        uuu * p0.0 + 3.0 * uu * t * p1.0 + 3.0 * u * tt * p2.0 + ttt * p3.0,
        uuu * p0.1 + 3.0 * uu * t * p1.1 + 3.0 * u * tt * p2.1 + ttt * p3.1,
    )
}

fn to_screen(p: (f32, f32), scale: f32, ox: f32, oy: f32) -> (i16, i16) {
    ((p.0 * scale + ox) as i16, (p.1 * scale + oy) as i16)
}

fn sample_bezier(
    verts: &mut Vec<(i16, i16)>,
    p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32),
    scale: f32, ox: f32, oy: f32,
) {
    for i in 1..=BEZIER_STEPS {
        let t = i as f32 / BEZIER_STEPS as f32;
        let p = cubic_bezier(t, p0, p1, p2, p3);
        verts.push(to_screen(p, scale, ox, oy));
    }
}

fn parse_path_to_polygon(d: &str, scale: f32, ox: f32, oy: f32) -> Vec<(i16, i16)> {
    let bytes = d.as_bytes();
    let mut pos = 0;
    let mut cursor = (0.0f32, 0.0f32);
    let mut _path_start = cursor;
    let mut verts: Vec<(i16, i16)> = Vec::new();
    let mut cmd = b'M';

    while pos < bytes.len() {
        let ch = bytes[pos];
        if ch.is_ascii_alphabetic() {
            cmd = ch;
            pos += 1;
            if cmd == b'Z' || cmd == b'z' {
                break;
            }
            continue;
        }

        match cmd {
            b'M' => {
                if let Some((x, y)) = read2(bytes, &mut pos) {
                    cursor = (x, y);
                    _path_start = cursor;
                    verts.push(to_screen(cursor, scale, ox, oy));
                    cmd = b'L';
                } else { break; }
            }
            b'm' => {
                if let Some((dx, dy)) = read2(bytes, &mut pos) {
                    cursor.0 += dx;
                    cursor.1 += dy;
                    _path_start = cursor;
                    verts.push(to_screen(cursor, scale, ox, oy));
                    cmd = b'l';
                } else { break; }
            }
            b'c' => {
                if let Some((dx1, dy1, dx2, dy2, dx, dy)) = read6(bytes, &mut pos) {
                    let cp1 = (cursor.0 + dx1, cursor.1 + dy1);
                    let cp2 = (cursor.0 + dx2, cursor.1 + dy2);
                    let end = (cursor.0 + dx, cursor.1 + dy);
                    sample_bezier(&mut verts, cursor, cp1, cp2, end, scale, ox, oy);
                    cursor = end;
                } else { break; }
            }
            b'C' => {
                if let Some((x1, y1, x2, y2, x, y)) = read6(bytes, &mut pos) {
                    sample_bezier(&mut verts, cursor, (x1, y1), (x2, y2), (x, y), scale, ox, oy);
                    cursor = (x, y);
                } else { break; }
            }
            b'l' => {
                if let Some((dx, dy)) = read2(bytes, &mut pos) {
                    cursor.0 += dx;
                    cursor.1 += dy;
                    verts.push(to_screen(cursor, scale, ox, oy));
                } else { break; }
            }
            b'L' => {
                if let Some((x, y)) = read2(bytes, &mut pos) {
                    cursor = (x, y);
                    verts.push(to_screen(cursor, scale, ox, oy));
                } else { break; }
            }
            _ => { pos += 1; }
        }
    }

    verts
}

fn fill_polygon(buffer: &mut [u8], width: usize, height: usize, vertices: &[(i16, i16)], color: u8) {
    if vertices.len() < 3 { return; }

    let min_y = vertices.iter().map(|v| v.1).min().unwrap().max(0);
    let max_y = vertices.iter().map(|v| v.1).max().unwrap().min(height as i16 - 1);

    let n = vertices.len();

    for y in min_y..=max_y {
        let mut intersections = [0i16; 64];
        let mut n_ix = 0usize;

        for i in 0..n {
            let (x0, y0) = vertices[i];
            let (x1, y1) = vertices[(i + 1) % n];

            if y0 == y1 { continue; }

            if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                let x = x0 as i32
                    + (y as i32 - y0 as i32) * (x1 as i32 - x0 as i32)
                        / (y1 as i32 - y0 as i32);
                if n_ix < 64 {
                    intersections[n_ix] = x as i16;
                    n_ix += 1;
                }
            }
        }

        // Insertion sort
        for i in 1..n_ix {
            let key = intersections[i];
            let mut j = i;
            while j > 0 && intersections[j - 1] > key {
                intersections[j] = intersections[j - 1];
                j -= 1;
            }
            intersections[j] = key;
        }

        // Fill between pairs
        let mut i = 0;
        while i + 1 < n_ix {
            let x_start = intersections[i].max(0) as usize;
            let x_end = (intersections[i + 1].max(0) as usize).min(width - 1);
            for x in x_start..=x_end {
                buffer[y as usize * width + x] = color;
            }
            i += 2;
        }
    }
}

pub fn render_wallpaper(width: u16, height: u16, bg_color: u8, logo_color: u8) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;

    let mut buffer = Vec::with_capacity(w * h);
    buffer.resize(w * h, bg_color);

    // Scale SVG to fit screen area
    let scale_x = width as f32 / SVG_W;
    let scale_y = height as f32 / SVG_H;
    let scale = if scale_x < scale_y { scale_x } else { scale_y };
    let ox = (width as f32 - SVG_W * scale) / 2.0;
    let oy = (height as f32 - SVG_H * scale) / 2.0;

    for path_d in &[PATH1_D, PATH2_D, PATH3_D] {
        let polygon = parse_path_to_polygon(path_d, scale, ox, oy);
        fill_polygon(&mut buffer, w, h, &polygon, logo_color);
    }

    buffer
}
