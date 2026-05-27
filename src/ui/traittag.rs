use super::SysInspectUX;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
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

        let bg = Color::Gray;
        let field_bg = Color::Cyan;
        let width = 60;
        let height = 9;
        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);

        let block = Block::default()
            .title(" Tag Minion ")
            .title_alignment(Alignment::Center)
            .title_style(Style::default().fg(Color::Black).bg(Color::Gray))
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(Color::Black))
            .style(Style::default().bg(bg));

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
            ])
            .split(inner);

        let lbl = Style::default().fg(Color::Black).bg(bg);
        let field = Style::default().fg(Color::Black).bg(field_bg);

        Paragraph::new(KEY_LABEL).style(lbl).render(vert[0], buf);
        Self::_render_input_field(&self.tag_key_buf, self.tag_pos, self.tag_focus == 0, field, vert[1], buf);

        Paragraph::new(VAL_LABEL).style(lbl).render(vert[2], buf);
        Self::_render_input_field(&self.tag_val_buf, self.tag_pos, self.tag_focus == 1, field, vert[3], buf);

        Paragraph::new(HINT).style(Style::default().fg(Color::Black).bg(bg)).render(vert[4], buf);

        let ok_label = Self::format_button(OK_LABEL);
        let cancel_label = Self::format_button(CANCEL_LABEL);
        let btn_gap = 3u16;
        let ok_w = ok_label.len() as u16;
        let cancel_w = cancel_label.len() as u16;
        let total_btn = ok_w + btn_gap + cancel_w;
        let btn_start = vert[6].x + (vert[6].width.saturating_sub(total_btn)) / 2;

        let ok_style = if self.tag_focus == 2 {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Black).bg(bg)
        };
        let cancel_style = if self.tag_focus == 3 {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Black).bg(bg)
        };

        Paragraph::new(ok_label).style(ok_style).render(Rect { x: btn_start, y: vert[6].y, width: ok_w, height: 1 }, buf);
        Paragraph::new(cancel_label)
            .style(cancel_style)
            .render(Rect { x: btn_start + ok_w + btn_gap, y: vert[6].y, width: cancel_w, height: 1 }, buf);

        for idx in 0..width {
            let cell = buf.cell_mut(Position::new(x + 2 + idx, y + height)).unwrap();
            cell.set_bg(Color::Black);
            cell.set_fg(Color::DarkGray);
        }
        for offset in 0..2 {
            for idx in 0..height {
                let cell = buf.cell_mut(Position::new(x + width + offset, y + idx + 1)).unwrap();
                cell.set_bg(Color::Black);
                cell.set_fg(Color::DarkGray);
            }
        }
    }

    fn _render_input_field(text: &str, pos: usize, active: bool, field_style: Style, area: Rect, buf: &mut Buffer) {
        let char_pos = text[..pos.min(text.len())].chars().count();
        let block = '█';
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

        for cx in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut(Position::new(cx, area.y)) {
                cell.set_style(field_style);
            }
        }

        for (i, ch) in display.chars().enumerate() {
            let x = area.x + i as u16;
            if x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut(Position::new(x, area.y)) {
                if ch == block {
                    cell.set_style(Style::default().fg(Color::White).bg(Color::Black).add_modifier(Modifier::SLOW_BLINK));
                    cell.set_symbol(" ");
                } else {
                    cell.set_symbol(ch.to_string().as_str());
                    cell.set_style(field_style);
                }
            }
        }
    }
}
