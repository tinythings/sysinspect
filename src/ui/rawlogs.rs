use super::{
    SysInspectUX, palette,
    title::{self, TitleSegment, TitleStyle},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Position},
    prelude::{Buffer, Rect, StatefulWidget},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarState, Widget},
};
use ratatui_cheese::input::{Input, InputState, InputStyles};

impl SysInspectUX {
    pub fn dialog_minion_logs(&self, parent: Rect, buf: &mut Buffer) {
        if !self.minion_logs_visible {
            return;
        }

        let border = if self.minion_logs_online { palette::PROCESSING_GLOW } else { palette::GRAY_0 };
        let title_style = TitleStyle::cyberpunk(border);
        let bg = palette::BG_2;
        let (logs_bg, logs_fg) = if self.minion_logs_online { (palette::PROCESSING_GLOW, palette::FG) } else { (palette::GRAY_0, palette::FG) };
        let (host_bg, host_fg) =
            if self.minion_logs_online { (palette::PROCESSING_HEAT, palette::SUCCESS) } else { (palette::GRAY_1, palette::ERROR) };
        let (kind_bg, kind_fg) = if self.minion_logs_online { (palette::PROCESSING_PEAK, palette::FG) } else { (palette::GRAY_2, palette::FG) };
        let (poll_bg, poll_fg) = if self.minion_logs_online { (palette::PROCESSING, palette::BG_1) } else { (palette::FG, palette::BG_1) };
        let mut segments = vec![
            TitleSegment { text: " Logs ".into(), bg: logs_bg, fg: logs_fg },
            TitleSegment { text: format!(" {} ", self.minion_logs_host), bg: host_bg, fg: host_fg },
            TitleSegment { text: format!(" {} ", self.minion_logs_source_kind), bg: kind_bg, fg: kind_fg },
        ];
        if self.minion_logs_polling {
            segments.push(TitleSegment { text: " \u{27F3} ".into(), bg: poll_bg, fg: poll_fg });
        }
        let min_width = title::ensure_inner_width(60, &title_style, &segments).saturating_add(2);
        let width = parent.width.saturating_sub(6).clamp(min_width, 140);
        let height = parent.height.saturating_sub(4).clamp(10, parent.height.saturating_sub(2));
        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(bg));
        let inner = block.inner(canvas);
        block.render(canvas, buf);

        title::overlay_gradient_title(buf, canvas, &title_style, &segments);

        if inner.height < 5 {
            return;
        }

        let [filter_area, path_area, content_area]: [Rect; 3] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();

        Self::_render_logs_filter(filter_area, buf, self.minion_logs_filter_focus, &self.minion_logs_filter);

        let path_line = Line::styled(format!(" {} ", self.minion_logs_path), Style::default().fg(palette::MUTED).bg(bg).add_modifier(Modifier::DIM));
        Widget::render(Paragraph::new(path_line), path_area, buf);

        let filter_val = self.minion_logs_filter.value().to_lowercase();
        let filtered_lines: Vec<&String> = if filter_val.is_empty() {
            self.minion_logs_lines.iter().collect()
        } else {
            self.minion_logs_lines.iter().filter(|l| l.to_lowercase().contains(&filter_val)).collect()
        };

        let text_area = Rect { x: content_area.x, y: content_area.y, width: content_area.width.saturating_sub(1), height: content_area.height };
        self.minion_logs_viewport_rows.set(text_area.height as usize);

        let rendered_lines = self.filtered_rendered_log_lines();
        let text_h = text_area.height as usize;
        let max_top = rendered_lines.len().saturating_sub(text_h);
        let start = if self.minion_logs_scroll == usize::MAX { max_top } else { self.minion_logs_scroll.min(max_top) };
        let end = (start + text_h).min(rendered_lines.len());
        if filtered_lines.is_empty() {
            let msg = if self.minion_logs_lines.is_empty() { "(no log lines)" } else { "(no matches)" };
            Widget::render(Paragraph::new(msg).style(Style::default().fg(palette::FG).bg(bg)), text_area, buf);
        } else {
            let visible: Vec<Line> = rendered_lines[start..end].to_vec();
            Widget::render(Paragraph::new(visible).style(Style::default().bg(bg)), text_area, buf);
        }

        let scroll_area = Rect { x: content_area.right().saturating_sub(1), y: content_area.y, width: 1, height: content_area.height };
        let mut scroll = ScrollbarState::default().content_length(rendered_lines.len().max(1)).position(start);
        Scrollbar::default()
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{28FF}"))
            .thumb_symbol("█")
            .track_style(Style::default().bg(bg))
            .thumb_style(Style::default().fg(palette::GRAY_1))
            .render(scroll_area, buf, &mut scroll);

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

    fn _render_logs_filter(area: Rect, buf: &mut Buffer, focused: bool, filter_state: &InputState) {
        buf.set_string(area.x, area.y, "Filter: ", Style::default().fg(palette::FG));

        let input_x = area.x + 8u16;
        let input_w = area.width.saturating_sub(8);
        if input_w == 0 {
            return;
        }

        let field_bg = if focused { palette::HIGHLIGHT } else { palette::GRAY_1 };
        for cx in input_x..input_x + input_w {
            if let Some(cell) = buf.cell_mut(Position::new(cx, area.y)) {
                cell.set_bg(field_bg);
            }
        }

        let mut is = InputState::new();
        is.set_value(filter_state.value().to_string());
        is.set_focused(focused);
        let fc = filter_state.cursor_pos();
        while is.cursor_pos() < fc {
            is.move_right();
        }
        let styles = InputStyles { text: Style::default().fg(palette::BG_1), ..Default::default() };
        let inp = Input::new("").prompt("").placeholder("filter lines...").styles(styles);
        StatefulWidget::render(&inp, Rect::new(input_x, area.y, input_w, 1), buf, &mut is);
    }

    pub(crate) fn filtered_rendered_log_lines(&self) -> Vec<Line<'static>> {
        let filter_val = self.minion_logs_filter.value().to_lowercase();
        let filtered_lines: Vec<&String> = if filter_val.is_empty() {
            self.minion_logs_lines.iter().collect()
        } else {
            self.minion_logs_lines.iter().filter(|l| l.to_lowercase().contains(&filter_val)).collect()
        };

        filtered_lines.iter().flat_map(|s| render_log_line(s)).collect()
    }
}

fn colorize_log_line(line: &str) -> Line<'static> {
    let default = || Line::styled(line.to_string(), Style::default().fg(palette::FG));

    let Some((ts, rest)) = line.split_once(" - ") else {
        return default();
    };
    let Some((level, msg)) = rest.split_once(':') else {
        return Line::from(vec![
            Span::styled(ts.to_string(), Style::default().fg(palette::SECONDARY)),
            Span::styled(" - ", Style::default().fg(palette::FAINT)),
            Span::styled(rest.to_string(), Style::default().fg(palette::FG)),
        ]);
    };

    let level_trimmed = level.trim();
    let level_style = match level_trimmed {
        "ERROR" => Style::default().fg(palette::ERROR_PEAK).add_modifier(Modifier::BOLD),
        "WARN" | "WARNING" => Style::default().fg(palette::WARNING_PEAK).add_modifier(Modifier::BOLD),
        "INFO" => Style::default().fg(palette::SUCCESS).add_modifier(Modifier::BOLD),
        "DEBUG" => Style::default().fg(palette::PROCESSING_PEAK).add_modifier(Modifier::BOLD),
        "TRACE" => Style::default().fg(palette::PROCESSING_HEAT),
        _ => Style::default().fg(palette::FG),
    };

    let mut spans = vec![
        Span::styled(ts.to_string(), Style::default().fg(palette::SECONDARY)),
        Span::styled(" - ", Style::default().fg(palette::FAINT)),
        Span::styled(level_trimmed.to_string(), level_style),
        Span::styled(": ", Style::default().fg(palette::FAINT)),
    ];

    let msg = msg.trim_start();
    if let Some(rest) = msg.strip_prefix('[')
        && let Some((tag, tail)) = rest.split_once(']')
    {
        spans.push(Span::styled(format!("[{tag}]"), Style::default().fg(palette::HIGHLIGHT).add_modifier(Modifier::BOLD)));
        if !tail.trim_start().is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(tail.trim_start().to_string(), Style::default().fg(palette::FG)));
        }
        return Line::from(spans);
    }

    spans.push(Span::styled(msg.to_string(), Style::default().fg(palette::FG)));
    Line::from(spans)
}

fn render_log_line(line: &str) -> Vec<Line<'static>> {
    let mut lines_iter = line.split('\n');
    let first = lines_iter.next().unwrap_or("");
    let mut out = vec![colorize_log_line(first)];
    for cont in lines_iter {
        out.push(Line::styled(cont.to_string(), Style::default().fg(palette::MUTED)));
    }
    out
}
