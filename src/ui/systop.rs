use super::{
    palette,
    title::{self, TitleSegment, TitleStyle},
    wgt::render_rule_line,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Position, Rect},
    prelude::{Buffer, StatefulWidget, Widget},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarState, Sparkline},
};
use std::{cell::Cell, time::Instant};

const HISTORY_LEN: usize = 96;

#[derive(Debug, Default)]
pub struct SystemTopState {
    pub visible: bool,
    pub minion_id: String,
    pub host: String,
    pub snapshot: Option<libsysinspect::console::ConsoleMinionTopSnapshot>,
    pub cpu_history: Vec<f32>,
    pub rx_history: Vec<u64>,
    pub tx_history: Vec<u64>,
    pub last_rx_total: Option<u64>,
    pub last_tx_total: Option<u64>,
    pub last_fetch: Option<Instant>,
    pub processes_scroll: usize,
    pub process_viewport_rows: Cell<usize>,
}

impl SystemTopState {
    pub fn open(&mut self, minion_id: String, host: String) {
        self.visible = true;
        self.minion_id = minion_id;
        self.host = host;
        self.snapshot = None;
        self.cpu_history.clear();
        self.rx_history.clear();
        self.tx_history.clear();
        self.last_rx_total = None;
        self.last_tx_total = None;
        self.last_fetch = None;
        self.processes_scroll = 0;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn apply_snapshot(&mut self, snapshot: libsysinspect::console::ConsoleMinionTopSnapshot) {
        push_history(&mut self.cpu_history, snapshot.cpu_percent);

        let rx_rate = self.last_rx_total.map(|prev| snapshot.network_rx_total_bytes.saturating_sub(prev)).unwrap_or(0);
        let tx_rate = self.last_tx_total.map(|prev| snapshot.network_tx_total_bytes.saturating_sub(prev)).unwrap_or(0);
        self.last_rx_total = Some(snapshot.network_rx_total_bytes);
        self.last_tx_total = Some(snapshot.network_tx_total_bytes);
        push_history_u64(&mut self.rx_history, rx_rate);
        push_history_u64(&mut self.tx_history, tx_rate);
        self.last_fetch = Some(Instant::now());
        self.snapshot = Some(snapshot);
    }

    pub fn render(&self, parent: Rect, buf: &mut Buffer) {
        if !self.visible {
            return;
        }

        let border = palette::PROCESSING_GLOW;
        let title_style = TitleStyle::cyberpunk(border);
        let mut segments = vec![
            TitleSegment { text: " System top ".into(), bg: palette::PROCESSING_GLOW, fg: palette::FG, modifier: Modifier::empty() },
            TitleSegment { text: format!(" {} ", self.host), bg: palette::PROCESSING_HEAT, fg: palette::SUCCESS, modifier: Modifier::empty() },
        ];
        if let Some(snapshot) = &self.snapshot {
            segments.push(TitleSegment {
                text: format!(" {:.1} avg ", snapshot.load_avg_five),
                bg: palette::PROCESSING_PEAK,
                fg: palette::FG,
                modifier: Modifier::empty(),
            });
        }

        let min_width = title::ensure_inner_width(100, &title_style, &segments).saturating_add(2);
        let width = parent.width.saturating_sub(6).clamp(min_width, 160);
        let height = parent.height.saturating_sub(4).clamp(18, parent.height.saturating_sub(2));
        let x = parent.x + (parent.width.saturating_sub(width)) / 2;
        let y = parent.y + (parent.height.saturating_sub(height)) / 2;
        let canvas = Rect { x, y, width, height };

        Clear.render(canvas, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(palette::BG_2));
        let inner = block.inner(canvas);
        block.render(canvas, buf);
        title::overlay_gradient_title(buf, canvas, &title_style, &segments);

        if inner.height < 8 || inner.width < 60 {
            return;
        }

        let [cpu_area, lower_area]: [Rect; 2] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(7), Constraint::Min(0)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();
        let [left_area, right_area]: [Rect; 2] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length((lower_area.width * 2) / 5), Constraint::Min(0)])
            .split(lower_area)
            .as_ref()
            .try_into()
            .unwrap();
        let [mem_area, net_area]: [Rect; 2] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(left_area.height.saturating_sub(8)), Constraint::Min(8)])
            .split(left_area)
            .as_ref()
            .try_into()
            .unwrap();

        self.render_cpu(cpu_area, buf);
        self.render_memory_disks(mem_area, buf);
        self.render_network(net_area, buf);
        self.render_processes(right_area, buf);

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

    fn render_cpu(&self, area: Rect, buf: &mut Buffer) {
        render_rule_line(area, buf, "CPU", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
        let Some(snapshot) = &self.snapshot else {
            Widget::render(Paragraph::new(" collecting... ").style(Style::default().fg(palette::MUTED).bg(palette::BG_2)), area, buf);
            return;
        };
        let [graph_area, cores_area]: [Rect; 2] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length((area.width * 2) / 5), Constraint::Min(0)])
            .split(Rect { x: area.x, y: area.y + 1, width: area.width, height: area.height.saturating_sub(1) })
            .as_ref()
            .try_into()
            .unwrap();

        self.render_cpu_braille(graph_area, buf, snapshot);
        self.render_core_bars(cores_area, buf, snapshot);
    }

    fn render_cpu_braille(&self, area: Rect, buf: &mut Buffer, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) {
        if area.height < 4 || area.width < 10 {
            return;
        }
        let graph_y = area.y + 1;
        let flame = braille_line(&self.cpu_history, area.width.saturating_sub(2) as usize);
        let flame_color = flame_color(snapshot.cpu_percent);
        buf.set_string(area.x + 1, graph_y, flame, Style::default().fg(flame_color));
        let stats = format!(
            " CPU {:>5.1}%   Uptime {}   Load {:.2} {:.2} {:.2}",
            snapshot.cpu_percent,
            format_uptime(snapshot.uptime_secs),
            snapshot.load_avg_one,
            snapshot.load_avg_five,
            snapshot.load_avg_fifteen
        );
        buf.set_string(area.x + 1, graph_y + 2, truncate(&stats, area.width.saturating_sub(2) as usize), Style::default().fg(palette::FG));
    }

    fn render_core_bars(&self, area: Rect, buf: &mut Buffer, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) {
        let cols = 2usize;
        let rows = snapshot.cpu_per_core.len().div_ceil(cols);
        let col_w = (area.width / cols as u16).max(1);
        for (idx, cpu) in snapshot.cpu_per_core.iter().enumerate() {
            let col = idx / rows.max(1);
            let row = idx % rows.max(1);
            let x = area.x + col as u16 * col_w;
            let y = area.y + row as u16;
            if y >= area.bottom() {
                break;
            }
            let label = format!("C{:02}", idx);
            buf.set_string(x, y, &label, Style::default().fg(palette::FORM_LABEL));
            let bar_x = x + 4;
            let bar_w = col_w.saturating_sub(12).max(4);
            render_percent_bar(buf, Rect { x: bar_x, y, width: bar_w, height: 1 }, *cpu);
            let pct = format!("{:>4.0}%", cpu);
            buf.set_string(bar_x + bar_w + 1, y, pct, Style::default().fg(flame_color(*cpu)));
        }
    }

    fn render_memory_disks(&self, area: Rect, buf: &mut Buffer) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let [mem_head, disks_head, disks_body]: [Rect; 3] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(7), Constraint::Length(1), Constraint::Min(0)])
            .split(area)
            .as_ref()
            .try_into()
            .unwrap();
        render_rule_line(mem_head, buf, "Mem", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
        let total = snapshot.memory_total_bytes;
        let used = snapshot.memory_used_bytes;
        let avail = snapshot.memory_available_bytes;
        let mem_rows = [
            ("Total", format_bytes(total)),
            ("Used", format_bytes(used)),
            ("Avail", format_bytes(avail)),
            ("Swap", format!("{} / {}", format_bytes(snapshot.swap_used_bytes), format_bytes(snapshot.swap_total_bytes))),
        ];
        for (idx, (label, value)) in mem_rows.iter().enumerate() {
            let y = mem_head.y + 1 + idx as u16;
            if y >= mem_head.bottom() {
                break;
            }
            buf.set_string(mem_head.x + 1, y, format!("{label:<6}"), Style::default().fg(palette::FORM_LABEL));
            buf.set_string(mem_head.x + 8, y, value, Style::default().fg(palette::FG));
        }
        let mem_pct = percent(used, total);
        render_percent_bar(
            buf,
            Rect { x: mem_head.x + 1, y: mem_head.bottom().saturating_sub(1), width: mem_head.width.saturating_sub(2), height: 1 },
            mem_pct,
        );

        render_rule_line(disks_head, buf, "Disks", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
        for (idx, disk) in snapshot.disks.iter().take(disks_body.height as usize).enumerate() {
            let y = disks_body.y + idx as u16;
            let name = if disk.mount_point.is_empty() { disk.name.as_str() } else { disk.mount_point.as_str() };
            let line =
                format!(" {} {:>5.0}% {} / {}", truncate(name, 10), disk.used_percent, format_bytes(disk.used_bytes), format_bytes(disk.total_bytes));
            buf.set_string(disks_body.x + 1, y, truncate(&line, disks_body.width.saturating_sub(2) as usize), Style::default().fg(palette::FG));
        }
    }

    fn render_network(&self, area: Rect, buf: &mut Buffer) {
        render_rule_line(area, buf, "Net", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
        let inner = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(1) };
        if inner.height < 3 || inner.width < 8 {
            return;
        }
        let [head_area, rx_area, tx_area]: [Rect; 3] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length((inner.height.saturating_sub(1)) / 2), Constraint::Min(1)])
            .split(inner)
            .as_ref()
            .try_into()
            .unwrap();
        let rx_now = self.rx_history.last().copied().unwrap_or(0);
        let tx_now = self.tx_history.last().copied().unwrap_or(0);
        buf.set_string(
            head_area.x,
            head_area.y,
            truncate(&format!("Down {}  Up {}", format_rate(rx_now), format_rate(tx_now)), head_area.width as usize),
            Style::default().fg(palette::FG),
        );
        Sparkline::default().data(&self.rx_history).style(Style::default().fg(palette::PROCESSING_HEAT)).render(rx_area, buf);
        Sparkline::default().data(&self.tx_history).style(Style::default().fg(palette::HIGHLIGHT)).render(tx_area, buf);
    }

    fn render_processes(&self, area: Rect, buf: &mut Buffer) {
        render_rule_line(area, buf, "Proc", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let body = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(1) };
        if body.height < 3 || body.width < 20 {
            return;
        }
        self.process_viewport_rows.set(body.height.saturating_sub(1) as usize);
        let pid_w = 7usize;
        let cpu_w = 6usize;
        let mem_w = 10usize;
        let name_w = body.width.saturating_sub((pid_w + cpu_w + mem_w + 6) as u16).max(10) as usize;
        buf.set_string(
            body.x,
            body.y,
            format!("{:>pid_w$} {:<name_w$} {:>cpu_w$} {:>mem_w$}", "PID", "Program", "CPU%", "Mem"),
            Style::default().fg(palette::FORM_LABEL),
        );
        let rows = snapshot.processes.len();
        let view_h = body.height.saturating_sub(1) as usize;
        let max_scroll = rows.saturating_sub(view_h);
        let start = self.processes_scroll.min(max_scroll);
        for (i, proc_row) in snapshot.processes.iter().skip(start).take(view_h).enumerate() {
            let y = body.y + 1 + i as u16;
            let line = format!(
                "{:>pid_w$} {:<name_w$} {:>cpu_w$.1} {:>mem_w$}",
                proc_row.pid,
                truncate(&proc_row.name, name_w),
                proc_row.cpu_percent,
                format_bytes(proc_row.memory_bytes)
            );
            buf.set_string(body.x, y, truncate(&line, body.width as usize), Style::default().fg(palette::FG));
        }
        let scroll_area = Rect { x: area.right().saturating_sub(1), y: body.y + 1, width: 1, height: body.height.saturating_sub(1) };
        let mut scroll = ScrollbarState::default().content_length(rows.max(1)).position(start);
        Scrollbar::default()
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("⣿"))
            .thumb_symbol("█")
            .track_style(Style::default().fg(palette::BG_3))
            .thumb_style(Style::default().fg(palette::GRAY_1))
            .render(scroll_area, buf, &mut scroll);
    }
}

fn percent(used: u64, total: u64) -> f32 {
    if total == 0 { 0.0 } else { (used as f64 / total as f64 * 100.0) as f32 }
}

fn format_bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = value as f64;
    let mut idx = 0usize;
    while v >= 1024.0 && idx < UNITS.len() - 1 {
        v /= 1024.0;
        idx += 1;
    }
    if idx == 0 { format!("{} {}", value, UNITS[idx]) } else { format!("{v:.1} {}", UNITS[idx]) }
}

fn format_rate(value: u64) -> String {
    format!("{}/s", format_bytes(value))
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 { format!("{days}d {hours:02}:{mins:02}") } else { format!("{hours:02}:{mins:02}") }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width { text.to_string() } else { format!("{}…", text.chars().take(width.saturating_sub(1)).collect::<String>()) }
}

fn flame_color(value: f32) -> ratatui::style::Color {
    if value >= 85.0 {
        palette::ERROR_PEAK
    } else if value >= 60.0 {
        palette::WARNING_PEAK
    } else if value >= 35.0 {
        palette::PROCESSING_HEAT
    } else {
        palette::PROCESSING_GLOW
    }
}

fn render_percent_bar(buf: &mut Buffer, area: Rect, pct: f32) {
    let pct = pct.clamp(0.0, 100.0);
    let filled = ((area.width as f32) * (pct / 100.0)).round() as u16;
    for i in 0..area.width {
        let x = area.x + i;
        let style = if i < filled { Style::default().fg(flame_color(pct)) } else { Style::default().fg(palette::GRAY_0) };
        buf.set_string(x, area.y, if i < filled { "█" } else { "░" }, style);
    }
}

fn braille_line(history: &[f32], width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(width);
    let start = history.len().saturating_sub(width * 2);
    let points = &history[start..];
    for i in 0..width {
        let left = points.get(i * 2).copied().unwrap_or(0.0);
        let right = points.get(i * 2 + 1).copied().unwrap_or(left);
        out.push(braille_pair(left, right));
    }
    out
}

fn braille_pair(left: f32, right: f32) -> char {
    let left_h = ((left.clamp(0.0, 100.0) / 25.0).round() as u8).min(4);
    let right_h = ((right.clamp(0.0, 100.0) / 25.0).round() as u8).min(4);
    let mut dots = 0u32;
    dots |= match left_h {
        0 => 0,
        1 => 1 << 6,
        2 => (1 << 2) | (1 << 6),
        3 => (1 << 1) | (1 << 2) | (1 << 6),
        _ => (1 << 0) | (1 << 1) | (1 << 2) | (1 << 6),
    };
    dots |= match right_h {
        0 => 0,
        1 => 1 << 7,
        2 => (1 << 5) | (1 << 7),
        3 => (1 << 4) | (1 << 5) | (1 << 7),
        _ => (1 << 3) | (1 << 4) | (1 << 5) | (1 << 7),
    };
    char::from_u32(0x2800 + dots).unwrap_or(' ')
}

fn push_history(history: &mut Vec<f32>, value: f32) {
    history.push(value.clamp(0.0, 100.0));
    if history.len() > HISTORY_LEN {
        history.drain(0..history.len() - HISTORY_LEN);
    }
}

fn push_history_u64(history: &mut Vec<u64>, value: u64) {
    history.push(value);
    if history.len() > HISTORY_LEN {
        history.drain(0..history.len() - HISTORY_LEN);
    }
}
