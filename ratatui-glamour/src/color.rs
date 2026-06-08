use ratatui::style::Color;

/// Convert any ratatui Color to (R, G, B) components.
pub fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(n) => indexed_to_rgb(n),
        _ => (255, 255, 255),
    }
}

/// Linearly interpolate between two Colors in RGB space.
/// Output is always `Color::Rgb` for true-color gradient fidelity.
pub fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (fr, fg, fb) = color_to_rgb(from);
    let (tr, tg, tb) = color_to_rgb(to);
    Color::Rgb(lerp_u8(fr, tr, t), lerp_u8(fg, tg, t), lerp_u8(fb, tb, t))
}

pub fn lerp_u8(from: u8, to: u8, t: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * t).round() as u8
}

/// Convert an xterm-256 indexed color to 24-bit RGB.
pub fn indexed_to_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        0 => (0x00, 0x00, 0x00),
        1 => (0x80, 0x00, 0x00),
        2 => (0x00, 0x80, 0x00),
        3 => (0x80, 0x80, 0x00),
        4 => (0x00, 0x00, 0x80),
        5 => (0x80, 0x00, 0x80),
        6 => (0x00, 0x80, 0x80),
        7 => (0xc0, 0xc0, 0xc0),
        8 => (0x80, 0x80, 0x80),
        9 => (0xff, 0x00, 0x00),
        10 => (0x00, 0xff, 0x00),
        11 => (0xff, 0xff, 0x00),
        12 => (0x00, 0x00, 0xff),
        13 => (0xff, 0x00, 0xff),
        14 => (0x00, 0xff, 0xff),
        15 => (0xff, 0xff, 0xff),
        16..=231 => {
            let cube = index - 16;
            let r = cube / 36;
            let g = (cube / 6) % 6;
            let b = cube % 6;
            const STEPS: [u8; 6] = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
            (STEPS[r as usize], STEPS[g as usize], STEPS[b as usize])
        }
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        }
    }
}
