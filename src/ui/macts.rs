use super::{
    SysInspectUX, palette,
    title::{self, TitleSegment, TitleStyle},
};
use ratatui::{
    layout::Position,
    prelude::{Buffer, Rect},
    style::Style,
    widgets::{Block, BorderType, Borders, Clear, Widget},
};
use ratatui_glamour::color::blend_2d;
use ratatui_glamour::rule::dashed_title;
use unicode_width::UnicodeWidthStr;

struct MenuSection {
    title: &'static str,
    items: &'static [(&'static str, &'static str)],
}

const MENU_SECTIONS: &[MenuSection] = &[
    MenuSection { title: "Tools", items: &[("System logs", "^L"), ("Defined traits", "^T")] },
    MenuSection {
        title: "Minion Operations",
        items: &[("Remote start", "^S"), ("Shutdown minion", "^D"), ("Force re-connect", "^F"), ("Delete minion", "DEL")],
    },
    MenuSection {
        title: "Cluster Operations",
        items: &[("Shutdown everything", "^X"), ("Reconnect all minions", "^A"), ("Register a new minion", "INS")],
    },
];

const MASTER_MENU_SECTIONS: &[MenuSection] = &[
    MenuSection {
        title: "Operations",
        items: &[("View master logs online", "^O"), ("View local logs", "^L"), ("Register a minion", "^R"), ("Repository manager", "^G")],
    },
    MenuSection { title: "System", items: &[("Start", "^T"), ("Stop", "^S"), ("Restart", "^E")] },
];

pub(crate) fn total_menu_items() -> usize {
    MENU_SECTIONS.iter().map(|s| s.items.len()).sum()
}

pub(crate) fn total_master_menu_items() -> usize {
    MASTER_MENU_SECTIONS.iter().map(|s| s.items.len()).sum()
}

#[allow(clippy::too_many_arguments)]
fn render_menu_popup(
    parent: Rect, buf: &mut Buffer, sections: &[MenuSection], sel: usize, title_segments: &[TitleSegment], title_style: &TitleStyle,
    max_item_w: usize, disabled: &[bool],
) {
    let inner_w = title::ensure_inner_width(max_item_w as u16, title_style, title_segments);

    let section_headers = sections.len() as u16;
    let item_rows: u16 = sections.iter().map(|s| s.items.len() as u16).sum();
    let inner_h = section_headers + item_rows + 2;

    let w = (inner_w + 2).min(parent.width.saturating_sub(8)).max(20);
    let h = (inner_h + 2).min(parent.height.saturating_sub(6)).max(5);
    let x = parent.x + (parent.width.saturating_sub(w)) / 2;
    let y = parent.y + (parent.height.saturating_sub(h)) / 2;
    let canvas = Rect { x, y, width: w, height: h };

    Clear.render(canvas, buf);

    let grad_colors = blend_2d(canvas.width as usize, canvas.height as usize, 10.0, &[palette::GRAY_0, palette::BG_2] as &[ratatui::style::Color]);
    for row in 0..canvas.height {
        for col in 0..canvas.width {
            let idx = row as usize * canvas.width as usize + col as usize;
            if let Some(cell) = buf.cell_mut(Position::new(canvas.x + col, canvas.y + row)) {
                cell.set_bg(grad_colors[idx]);
            }
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(palette::PROCESSING_GLOW))
        .style(Style::default());
    let inner = block.inner(canvas);
    block.render(canvas, buf);

    title::overlay_gradient_title(buf, canvas, title_style, title_segments);

    let hint_style = Style::default().fg(palette::PRIMARY);

    let mut row_y = inner.y;
    let mut flat_idx: usize = 0;

    for (si, section) in sections.iter().enumerate() {
        if row_y >= inner.bottom() {
            break;
        }
        if si > 0 && row_y < inner.bottom() {
            row_y += 1;
        }
        dashed_title(
            Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
            buf,
            section.title,
            palette::PROCESSING,
            palette::PRIMARY,
            palette::PROCESSING_DIMMED,
        );
        row_y += 1;

        for &(label, key) in section.items {
            if row_y >= inner.bottom() {
                break;
            }
            let selected = flat_idx == sel;
            let is_disabled = disabled.get(flat_idx).copied().unwrap_or(false);
            let item_style = if is_disabled {
                Style::default().fg(palette::MUTED)
            } else if selected {
                Style::default().fg(palette::BLACK).bg(palette::HIGHLIGHT)
            } else {
                Style::default().fg(palette::FG)
            };

            let hint = key;
            let padding = (inner.width as usize).saturating_sub(UnicodeWidthStr::width(label) + 1 + UnicodeWidthStr::width(hint)).saturating_sub(2);
            let line = format!(" {label}{}{hint} ", " ".repeat(padding));
            buf.set_string(inner.x, row_y, &line, item_style);

            // Re-paint just the key hint with its own style on top
            if !hint.is_empty() {
                let hint_x = inner.x + (inner.width.saturating_sub(UnicodeWidthStr::width(hint) as u16 + 2));
                let hint_sel_style = if selected && !is_disabled { Style::default().fg(palette::BG_0).bg(palette::HIGHLIGHT) } else { hint_style };
                buf.set_string(hint_x, row_y, hint, hint_sel_style);
            }

            row_y += 1;
            flat_idx += 1;
        }
    }

    let buf_area = buf.area();
    let max_x = buf_area.right().saturating_sub(1);
    let max_y = buf_area.bottom().saturating_sub(1);

    for idx in 0..w {
        let sx = x.saturating_add(2).saturating_add(idx);
        let sy = y.saturating_add(h);
        if sx > max_x || sy > max_y {
            continue;
        }
        if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
            cell.set_bg(palette::SHADOW_BG);
            cell.set_fg(palette::SHADOW_FG);
        }
    }
    for offset in 0..2u16 {
        for idx in 0..h {
            let sx = x.saturating_add(w).saturating_add(offset);
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

impl SysInspectUX {
    pub fn minion_actions_menu(&self, parent: Rect, buf: &mut Buffer) {
        if !self.minions_menu_visible {
            return;
        }

        let host = self
            .minions_rows
            .iter()
            .find(|r| {
                let online: Vec<&libsysinspect::console::ConsoleOnlineMinionRow> = self.minions_rows.iter().filter(|r| r.alive).collect();
                let offline: Vec<&libsysinspect::console::ConsoleOnlineMinionRow> = self.minions_rows.iter().filter(|r| !r.alive).collect();
                match self.minions_focus {
                    1 => online.get(self.minions_online_sel).map(|m| m.minion_id == r.minion_id).unwrap_or(false),
                    2 => offline.get(self.minions_offline_sel).map(|m| m.minion_id == r.minion_id).unwrap_or(false),
                    _ => false,
                }
            })
            .map(Self::online_host)
            .unwrap_or_else(|| "unknown".to_string());

        let max_label_w = MENU_SECTIONS.iter().flat_map(|s| s.items.iter()).map(|(label, _)| UnicodeWidthStr::width(*label)).max().unwrap_or(10);
        let max_item_w = max_label_w + 34;

        let mut title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
        let is_cluster = self.minions_menu_sel >= 6;
        let mut segments = vec![TitleSegment { text: " Actions on ".into(), bg: palette::PROCESSING_GLOW, fg: palette::FG }];
        if is_cluster {
            segments.push(TitleSegment { text: " Cluster ".into(), bg: palette::PROCESSING_PEAK, fg: palette::FG });
            segments.push(TitleSegment { text: " ⚡⚡⚡ ".into(), bg: palette::ERROR_PEAK, fg: palette::WARNING_PEAK });
            title_style.gradient_target = Some(palette::ERROR_BASE);
        } else {
            segments.push(TitleSegment { text: format!(" {host} "), bg: palette::PROCESSING_HEAT, fg: palette::SUCCESS_PEAK });
        }

        render_menu_popup(parent, buf, MENU_SECTIONS, self.minions_menu_sel, &segments, &title_style, max_item_w, &[]);
    }

    pub fn master_actions_menu(&self, parent: Rect, buf: &mut Buffer) {
        if !self.master_menu_visible {
            return;
        }

        let max_label_w =
            MASTER_MENU_SECTIONS.iter().flat_map(|s| s.items.iter()).map(|(label, _)| UnicodeWidthStr::width(*label)).max().unwrap_or(10);
        let max_item_w = max_label_w + 20;

        let title_style = TitleStyle::cyberpunk(palette::PROCESSING_GLOW);
        let is_system = self.master_menu_sel >= 4;
        let sub_title = if is_system { " System " } else { " Operations " };
        let segments = vec![
            TitleSegment { text: " Master ".into(), bg: palette::PROCESSING_GLOW, fg: palette::FG },
            TitleSegment { text: sub_title.into(), bg: palette::PROCESSING_HEAT, fg: palette::FG },
        ];

        let local_logs_available = self.cfg.logfile_std().exists() || self.cfg.logfile_err().exists();
        let disabled = [!local_logs_available, false, false, false, false, false, false];

        render_menu_popup(parent, buf, MASTER_MENU_SECTIONS, self.master_menu_sel, &segments, &title_style, max_item_w, &disabled);
    }
}
