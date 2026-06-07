use super::{SysInspectUX, palette};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

static KEY_LABEL: &str = "Key:";
static VAL_LABEL: &str = "Value:";
static HINT: &str = "To delete a key, leave value empty.";
static OK_LABEL: &str = "OK";
static CANCEL_LABEL: &str = "Cancel";

impl SysInspectUX {
    pub fn dialog_trait_tag(&self, parent: Rect, buf: &mut Buffer) {
        if !self.tag_visible {
            return;
        }

        let width = 60;
        let height = 10;
        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let block = Block::default()
            .title(Line::from(vec![
                Span::styled("\u{E0B2}", Style::default().fg(palette::BORDER)),
                Span::styled(" Tag Minion ", Style::default().fg(palette::BLACK).bg(palette::BORDER)),
                Span::styled("\u{E0B0}", Style::default().fg(palette::BORDER)),
            ]))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(palette::BORDER))
            .style(Style::default().bg(palette::POPUP_BG_BASE));

        let inner = block.inner(canvas);
        block.render(canvas, buf);

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        let lbl = Style::default().fg(palette::FG).bg(palette::POPUP_BG_BASE);
        let hint = Style::default().fg(palette::MUTED).bg(palette::POPUP_BG_BASE);
        let active_field = Style::default().fg(palette::WHITE).bg(palette::HIGHLIGHT);
        let inactive_field = Style::default().fg(palette::BG_1).bg(palette::GRAY_1);

        Paragraph::new(KEY_LABEL).style(lbl).render(vert[0], buf);
        Self::_render_input_field(&self.tag_key_buf, self.tag_pos, self.tag_focus == 0, active_field, inactive_field, vert[1], buf);

        Paragraph::new(VAL_LABEL).style(lbl).render(vert[2], buf);
        Self::_render_input_field(&self.tag_val_buf, self.tag_pos, self.tag_focus == 1, active_field, inactive_field, vert[3], buf);

        Paragraph::new(HINT).style(hint).render(vert[5], buf);

        let ok_label = Self::format_button(OK_LABEL);
        let cancel_label = Self::format_button(CANCEL_LABEL);
        let btn_gap = 3u16;
        let ok_w = ok_label.len() as u16;
        let cancel_w = cancel_label.len() as u16;
        let total_btn = ok_w + btn_gap + cancel_w;
        let btn_start = vert[7].x + (vert[7].width.saturating_sub(total_btn)) / 2;

        let b_selected = Style::default().fg(palette::WHITE).bg(palette::PROCESSING_HEAT);
        let b_unselected = Style::default().fg(palette::FG).bg(palette::BG_2);

        let ok_style = if self.tag_focus == 2 { b_selected } else { b_unselected };
        let cancel_style = if self.tag_focus == 3 { b_selected } else { b_unselected };

        Paragraph::new(ok_label).style(ok_style).render(Rect { x: btn_start, y: vert[7].y, width: ok_w, height: 1 }, buf);
        Paragraph::new(cancel_label)
            .style(cancel_style)
            .render(Rect { x: btn_start + ok_w + btn_gap, y: vert[7].y, width: cancel_w, height: 1 }, buf);

        let buf_area = buf.area();
        let max_x = buf_area.right().saturating_sub(1);
        let max_y = buf_area.bottom().saturating_sub(1);

        for idx in 0..width {
            let sx = x.saturating_add(2).saturating_add(idx);
            let sy = y.saturating_add(height);
            if sx > max_x || sy > max_y {
                continue;
            }
            if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                cell.set_bg(palette::SHADOW_BG);
                cell.set_fg(palette::SHADOW_FG);
            }
        }
        for offset in 0..2u16 {
            for idx in 0..height {
                let sx = x.saturating_add(width).saturating_add(offset);
                let sy = y.saturating_add(idx).saturating_add(1);
                if sx > max_x || sy > max_y {
                    continue;
                }
                if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
                    cell.set_bg(palette::SHADOW_BG);
                    cell.set_fg(palette::SHADOW_FG);
                }
            }
        }
    }

    fn _render_input_field(text: &str, pos: usize, active: bool, active_style: Style, inactive_style: Style, area: Rect, buf: &mut Buffer) {
        let field_bg = if active { active_style } else { inactive_style };
        let char_pos = text[..pos.min(text.len())].chars().count();
        let block = '█';

        for cx in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut(Position::new(cx, area.y)) {
                cell.set_style(field_bg);
            }
        }

        let mut display = String::with_capacity(area.width as usize);
        let mut i = 0usize;

        for ch in text.chars() {
            if display.len() >= area.width as usize {
                break;
            }
            if active && i == char_pos {
                display.push(block);
                if display.len() >= area.width as usize {
                    break;
                }
            }
            display.push(ch);
            i += 1;
        }
        if active && i == char_pos && display.len() < area.width as usize {
            display.push(block);
        }

        for (i, ch) in display.chars().enumerate() {
            let x = area.x + i as u16;
            if x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut(Position::new(x, area.y)) {
                if ch == block {
                    cell.set_style(Style::default().fg(palette::WHITE).bg(palette::BLACK).add_modifier(Modifier::SLOW_BLINK));
                    cell.set_symbol(" ");
                } else {
                    cell.set_symbol(ch.to_string().as_str());
                    cell.set_style(field_bg);
                }
            }
        }
    }
}
