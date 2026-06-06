use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Color,
};

pub struct TitleSegment {
    pub text: String,
    pub bg: Color,
    pub fg: Color,
}

pub struct TitleStyle {
    pub border_color: Color,
    pub gradient_target: Option<Color>,
    pub left_cap: &'static str,
    pub join_glyph: &'static str,
    pub gradient_glyph: &'static str,
    pub return_gradient_stops: u16,
    pub return_gradient_step_width: u16,
}

impl TitleStyle {
    pub const fn cyberpunk(border_color: Color) -> Self {
        Self {
            border_color,
            gradient_target: None,
            left_cap: "\u{E0B2}",
            join_glyph: "\u{E0B0}",
            gradient_glyph: "\u{2500}",
            return_gradient_stops: 4,
            return_gradient_step_width: 3,
        }
    }
}

/// Overlay a multi-segment gradient bubble title on the top border row
/// of an already-rendered Block. Call immediately after `block.render(rect, buf)`.
///
/// Renders:
///   ╭Seg0TextSeg1TextSeg2Text────[gradient back to border]────╮
///
/// * `block_rect` – the full Rect passed to `block.render()`.
/// * `style` – glyphs and gradient behavior for the rendered title.
/// * `segments` – one or more text segments, each with its own bg/fg.
pub fn overlay_gradient_title(buf: &mut Buffer, block_rect: Rect, style: &TitleStyle, segments: &[TitleSegment]) {
    if segments.is_empty() || block_rect.width < 3 {
        return;
    }

    let row_y = block_rect.y;
    let x_start = block_rect.x + 1;
    let x_end = block_rect.right().saturating_sub(1);
    let mut cx = x_start;

    if cx >= x_end {
        return;
    }

    // Left cap: colored glyph only, background stays whatever the block already painted.
    if cx < x_end {
        cell_set_symbol_fg(buf, cx, row_y, style.left_cap, segments[0].bg);
        cx += 1;
    }

    // Segments with proper powerline-style separators.
    for (i, seg) in segments.iter().enumerate() {
        // Inter-segment separator ( from previous bg to current bg).
        if i > 0 && cx < x_end {
            let prev_bg = segments[i - 1].bg;
            cell_set_symbol_style(buf, cx, row_y, style.join_glyph, prev_bg, seg.bg);
            cx += 1;
        }

        // Segment text
        for ch in seg.text.chars() {
            if cx >= x_end {
                break;
            }
            cell_set_symbol_style(buf, cx, row_y, ch.to_string().as_str(), seg.fg, seg.bg);
            cx += 1;
        }

        if cx >= x_end {
            return;
        }
    }

    // Right cap: colored glyph only, background stays whatever the block already painted.
    let last_bg = segments.last().unwrap().bg;
    if cx < x_end {
        cell_set_symbol_fg(buf, cx, row_y, style.join_glyph, last_bg);
        cx += 1;
    }

    // Gradient fill: return from the last segment color to the first/left border
    // color so the top edge lands back on the same hue as the left corner.
    let remaining = x_end.saturating_sub(cx);
    if remaining == 0 {
        return;
    }

    let gradient_target = style.gradient_target.unwrap_or_else(|| segments.first().map(|seg| seg.bg).unwrap_or(style.border_color));
    let gradient_cells = remaining.min(style.return_gradient_stops * style.return_gradient_step_width);
    let gradient_stops = remaining.div_ceil(style.return_gradient_step_width).min(style.return_gradient_stops);

    for i in 0..remaining {
        let x = cx + i;
        if x >= x_end {
            break;
        }
        let grad = if i < gradient_cells {
            let stop = i / style.return_gradient_step_width;
            let t = if gradient_stops > 1 { stop as f32 / (gradient_stops - 1) as f32 } else { 1.0 };
            interpolate_indexed(last_bg, gradient_target, t)
        } else {
            gradient_target
        };
        cell_set_symbol_fg(buf, x, row_y, style.gradient_glyph, grad);
    }
}

/// Interpolate between indexed colors in RGB space, then quantize back to the
/// nearest xterm-256 entry. Interpolating raw palette indexes directly produces
/// visibly wrong jumps across unrelated colors.
pub fn interpolate_indexed(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (from, to) {
        (Color::Indexed(f), Color::Indexed(t_val)) => {
            let (fr, fg, fb) = indexed_to_rgb(f);
            let (tr, tg, tb) = indexed_to_rgb(t_val);
            let rgb = (lerp_u8(fr, tr, t), lerp_u8(fg, tg, t), lerp_u8(fb, tb, t));
            Color::Indexed(nearest_indexed(rgb))
        }
        _ => to,
    }
}

fn lerp_u8(from: u8, to: u8, t: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * t).round() as u8
}

fn indexed_to_rgb(index: u8) -> (u8, u8, u8) {
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

fn nearest_indexed(target: (u8, u8, u8)) -> u8 {
    let mut best_index = 0u8;
    let mut best_distance = u32::MAX;

    for index in 0u8..=255 {
        let (r, g, b) = indexed_to_rgb(index);
        let dr = r as i32 - target.0 as i32;
        let dg = g as i32 - target.1 as i32;
        let db = b as i32 - target.2 as i32;
        let distance = (dr * dr + dg * dg + db * db) as u32;
        if distance < best_distance {
            best_distance = distance;
            best_index = index;
        }
    }

    best_index
}

fn cell_set_symbol_fg(buf: &mut Buffer, x: u16, y: u16, symbol: &str, fg: Color) {
    if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
        cell.set_symbol(symbol);
        cell.set_fg(fg);
    }
}

fn cell_set_symbol_style(buf: &mut Buffer, x: u16, y: u16, symbol: &str, fg: Color, bg: Color) {
    if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
        cell.set_symbol(symbol);
        cell.set_fg(fg);
        cell.set_bg(bg);
    }
}
