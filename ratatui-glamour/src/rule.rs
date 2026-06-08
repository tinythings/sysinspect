use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

use crate::color::lerp_color;

/// Render a dashed title line with gradient fill:
///
/// ```text
///  Tools ///////////////////////////////
/// ```
///
/// The `/` characters interpolate in RGB space from `gradient_start` (first `/`)
/// to `gradient_end` (last `/`).  The label ` {text} ` is rendered in `text_fg`.
pub fn dashed_title(area: Rect, buf: &mut Buffer, text: &str, text_fg: Color, gradient_start: Color, gradient_end: Color) {
    if area.width < 6 {
        return;
    }

    let label = format!(" {text} ");
    let label_w = label.len() as u16;
    buf.set_string(area.x, area.y, &label, Style::default().fg(text_fg));

    let fill_start = area.x.saturating_add(label_w);
    let fill_end = area.right().saturating_sub(1);
    let fill_count = fill_end.saturating_sub(fill_start);

    if fill_count == 0 {
        return;
    }

    for i in 0..fill_count {
        let x = fill_start + i;
        if x >= fill_end || x >= fill_start.saturating_add(area.width) {
            break;
        }
        let t = if fill_count > 1 { i as f32 / (fill_count - 1) as f32 } else { 0.0 };
        let color = lerp_color(gradient_start, gradient_end, t);
        buf.set_string(x, area.y, "/", Style::default().fg(color));
    }
}
