use super::{
    palette,
    title::{self, TitleSegment, TitleStyle},
    wgt::render_rule_line,
};
use ratatui::{
    layout::{Position, Rect},
    prelude::{Buffer, StatefulWidget, Widget},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarState},
};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

const HISTORY_LEN: usize = 96;

#[derive(Debug, Default, Clone)]
struct NetworkHistory {
    rx_history: Vec<u64>,
    tx_history: Vec<u64>,
    last_rx_total: Option<u64>,
    last_tx_total: Option<u64>,
    rx_rate: u64,
    tx_rate: u64,
    peak_rate: u64,
    rx_total: u64,
    tx_total: u64,
}

#[derive(Debug, Default)]
pub struct SystemTopState {
    pub visible: bool,
    pub minion_id: String,
    pub host: String,
    pub snapshot: Option<libsysinspect::console::ConsoleMinionTopSnapshot>,
    pub cpu_history: Vec<f32>,
    total_network: NetworkHistory,
    interface_networks: BTreeMap<String, NetworkHistory>,
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
        self.total_network = NetworkHistory::default();
        self.interface_networks.clear();
        self.last_fetch = None;
        self.processes_scroll = 0;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn apply_snapshot(&mut self, snapshot: libsysinspect::console::ConsoleMinionTopSnapshot) {
        push_history(&mut self.cpu_history, snapshot.cpu_percent);

        update_network_history(&mut self.total_network, snapshot.network_rx_total_bytes, snapshot.network_tx_total_bytes);

        let mut present = BTreeSet::new();
        for iface in &snapshot.network_interfaces {
            present.insert(iface.name.clone());
            let history = self.interface_networks.entry(iface.name.clone()).or_default();
            update_network_history(history, iface.rx_total_bytes, iface.tx_total_bytes);
        }
        self.interface_networks.retain(|name, _| present.contains(name));

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

        let min_width = title::ensure_inner_width(112, &title_style, &segments).saturating_add(2);
        let width = parent.width.saturating_sub(6).clamp(min_width, 164);
        let height = parent.height.saturating_sub(4).clamp(20, parent.height.saturating_sub(2));
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

        if inner.height < 10 || inner.width < 72 {
            return;
        }

        let cpu_h = 5;
        let cpu_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: cpu_h.min(inner.height) };
        let lower_area = Rect { x: inner.x, y: inner.y + cpu_area.height, width: inner.width, height: inner.height.saturating_sub(cpu_area.height) };
        let left_w = ((lower_area.width * 2) / 5).clamp(30, lower_area.width.saturating_sub(24));
        let left_area = Rect { x: lower_area.x, y: lower_area.y, width: left_w, height: lower_area.height };
        let right_area =
            Rect { x: lower_area.x + left_w, y: lower_area.y, width: lower_area.width.saturating_sub(left_w), height: lower_area.height };
        let mem_h = 4.min(left_area.height);
        let mut net_h = 8.min(left_area.height.saturating_sub(mem_h));
        let disk_h = left_area.height.saturating_sub(mem_h + net_h);
        if disk_h < 4 && net_h > 4 {
            net_h = net_h.saturating_sub(4 - disk_h);
        }
        let disk_h = left_area.height.saturating_sub(mem_h + net_h);
        let mem_area = Rect { x: left_area.x, y: left_area.y, width: left_area.width, height: mem_h };
        let disks_area = Rect { x: left_area.x, y: left_area.y + mem_h, width: left_area.width, height: disk_h };
        let net_area = Rect { x: left_area.x, y: left_area.y + mem_h + disk_h, width: left_area.width, height: net_h };

        self.render_cpu(cpu_area, buf);
        self.render_memory(mem_area, buf);
        self.render_disks(disks_area, buf);
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
        let Some(snapshot) = &self.snapshot else {
            render_rule_line(area, buf, "CPU", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
            Widget::render(Paragraph::new(" collecting... ").style(Style::default().fg(palette::FG).bg(palette::BG_2)), area, buf);
            return;
        };

        let title = format!(
            " {:>4.1}%  Uptime:{}  Load {:.2}/{:.2}/{:.2} ",
            snapshot.cpu_percent,
            format_uptime(snapshot.uptime_secs),
            snapshot.load_avg_one,
            snapshot.load_avg_five,
            snapshot.load_avg_fifteen
        );
        render_rule_line_parts(
            area,
            buf,
            &[(" CPU ", Style::default().fg(palette::HIGHLIGHT).add_modifier(Modifier::BOLD)), (&title, Style::default().fg(palette::FG))],
            palette::PRIMARY,
            palette::PROCESSING_DIMMED,
        );

        let body = Rect { x: area.x, y: area.y + 1, width: area.width, height: area.height.saturating_sub(1) };
        if body.height == 0 {
            return;
        }
        let graph_w = ((body.width * 2) / 5).clamp(22, body.width.saturating_sub(20));
        let graph_area = Rect { x: body.x, y: body.y, width: graph_w, height: body.height };
        let cores_area = Rect { x: body.x + graph_w, y: body.y, width: body.width.saturating_sub(graph_w), height: body.height };

        self.render_cpu_braille(graph_area, buf, snapshot);
        self.render_core_bars(cores_area, buf, snapshot);
    }

    fn render_cpu_braille(&self, area: Rect, buf: &mut Buffer, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) {
        if area.height < 2 || area.width < 12 {
            return;
        }
        let flame = braille_line(&self.cpu_history, area.width.saturating_sub(2) as usize);
        let flame_color = flame_color(snapshot.cpu_percent);
        buf.set_string(area.x + 1, area.y, flame, Style::default().fg(flame_color));
        if area.height > 1 {
            let detail = format!("Trend {:>4.1}%  Cores {}", snapshot.cpu_percent, snapshot.cpu_per_core.len());
            buf.set_string(
                area.x + 1,
                area.y + area.height.saturating_sub(1),
                truncate(&detail, area.width.saturating_sub(2) as usize),
                Style::default().fg(palette::FG),
            );
        }
    }

    fn render_core_bars(&self, area: Rect, buf: &mut Buffer, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) {
        if area.height == 0 || area.width < 14 {
            return;
        }
        let max_rows = area.height as usize;
        let cols = snapshot.cpu_per_core.len().div_ceil(max_rows.max(1)).clamp(1, 4);
        let rows = snapshot.cpu_per_core.len().div_ceil(cols).max(1);
        let col_w = (area.width / cols as u16).max(1);
        for (idx, cpu) in snapshot.cpu_per_core.iter().enumerate() {
            let col = idx / rows;
            let row = idx % rows;
            let x = area.x + col as u16 * col_w;
            let y = area.y + row as u16;
            if y >= area.bottom() {
                break;
            }
            let label = format!("C{:02}", idx);
            buf.set_string(x, y, &label, Style::default().fg(palette::FORM_LABEL));
            let bar_x = x + 4;
            let show_pct = col_w >= 15;
            let pct_w = if show_pct { 6 } else { 0 };
            let bar_w = col_w.saturating_sub(5 + pct_w).max(3);
            render_percent_bar(buf, Rect { x: bar_x, y, width: bar_w, height: 1 }, *cpu);
            if show_pct {
                let pct = format!("{:>4.0}%", cpu);
                buf.set_string(bar_x + bar_w + 1, y, pct, Style::default().fg(flame_color(*cpu)));
            }
        }
    }

    fn render_memory(&self, area: Rect, buf: &mut Buffer) {
        render_rule_line(area, buf, "Mem", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if area.height < 4 || area.width < 24 {
            return;
        }
        let body = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(1) };
        let left_w = (body.width / 2).max(1);
        let right_w = body.width.saturating_sub(left_w);
        render_labeled_value(buf, body.x, body.y, left_w, "Total", &format_bytes(snapshot.memory_total_bytes));
        render_labeled_value(buf, body.x + left_w, body.y, right_w, "Used", &format_bytes(snapshot.memory_used_bytes));
        if body.height > 1 {
            render_labeled_value(buf, body.x, body.y + 1, left_w, "Avail", &format_bytes(snapshot.memory_available_bytes));
            render_labeled_value(
                buf,
                body.x + left_w,
                body.y + 1,
                right_w,
                "Swap",
                &format!("{} / {}", format_bytes(snapshot.swap_used_bytes), format_bytes(snapshot.swap_total_bytes)),
            );
        }
        if body.height > 2 {
            let ram_pct = percent(snapshot.memory_used_bytes, snapshot.memory_total_bytes);
            let swap_pct = percent(snapshot.swap_used_bytes, snapshot.swap_total_bytes);
            let left_bar = Rect { x: body.x, y: body.y + 2, width: left_w.saturating_sub(1), height: 1 };
            let right_bar = Rect { x: body.x + left_w, y: body.y + 2, width: right_w, height: 1 };
            render_meter_row(buf, left_bar, "RAM", ram_pct, flame_color(ram_pct));
            render_meter_row(buf, right_bar, "SWP", swap_pct, palette::PROCESSING_HEAT);
        }
    }

    fn render_disks(&self, area: Rect, buf: &mut Buffer) {
        render_rule_line(area, buf, "Disks", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let body = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(1) };
        if body.height == 0 || body.width < 18 {
            return;
        }

        let rows_per_disk = if body.height >= 6 { 2 } else { 1 };
        let visible = (body.height as usize / rows_per_disk).max(1);
        for (idx, disk) in snapshot.disks.iter().take(visible).enumerate() {
            let base_y = body.y + (idx * rows_per_disk) as u16;
            let name = if disk.mount_point.is_empty() { disk.name.as_str() } else { disk.mount_point.as_str() };
            let title =
                format!("{} {:>5.0}% {} / {}", truncate(name, 10), disk.used_percent, format_bytes(disk.used_bytes), format_bytes(disk.total_bytes));
            buf.set_string(body.x, base_y, truncate(&title, body.width as usize), Style::default().fg(palette::FG));
            if rows_per_disk == 2 && base_y + 1 < body.bottom() {
                let line = Rect { x: body.x, y: base_y + 1, width: body.width, height: 1 };
                render_dot_bar(buf, line, disk.used_percent, flame_color(disk.used_percent), palette::GRAY_0);
            }
        }
    }

    fn render_network(&self, area: Rect, buf: &mut Buffer) {
        let Some(snapshot) = &self.snapshot else {
            render_rule_line(area, buf, "Net", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
            return;
        };
        let primary_name = self.primary_interface_name(snapshot).unwrap_or_else(|| "all".to_string());
        render_rule_line(
            area,
            buf,
            &format!("Net {}", primary_name),
            Style::default().fg(palette::HIGHLIGHT),
            palette::PRIMARY,
            palette::PROCESSING_DIMMED,
        );
        let inner = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(1) };
        if inner.height < 5 || inner.width < 20 {
            return;
        }

        let info_w = (inner.width / 3).clamp(14, 20);
        let chart_area = Rect { x: inner.x, y: inner.y, width: inner.width.saturating_sub(info_w), height: inner.height };
        let info_area = Rect { x: inner.right().saturating_sub(info_w), y: inner.y, width: info_w, height: inner.height };

        let primary = self.primary_network_history(snapshot);
        let primary_history = primary.unwrap_or((&primary_name, &self.total_network));
        render_mirrored_net_chart(buf, chart_area, primary_history.1);
        self.render_network_details(info_area, snapshot, primary_history.0, primary_history.1, &primary_name, buf);
    }

    fn render_network_details(
        &self, area: Rect, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot, iface_name: &str, primary: &NetworkHistory,
        primary_name: &str, buf: &mut Buffer,
    ) {
        if area.width < 10 || area.height == 0 {
            return;
        }
        let mut y = area.y;
        let rows = [
            (iface_name, Style::default().fg(palette::FORM_LABEL).add_modifier(Modifier::BOLD)),
            (&format!("Dn {}", format_rate(primary.rx_rate)), Style::default().fg(palette::SUCCESS)),
            (&format!("Up {}", format_rate(primary.tx_rate)), Style::default().fg(palette::PRIMARY)),
            (&format!("Pk {}", format_rate(primary.peak_rate)), Style::default().fg(palette::FG)),
            (&format!("Rx {}", format_bytes(primary.rx_total)), Style::default().fg(palette::FG)),
            (&format!("Tx {}", format_bytes(primary.tx_total)), Style::default().fg(palette::FG)),
        ];
        for (text, style) in rows {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(area.x, y, truncate(text, area.width as usize), style);
            y += 1;
        }

        if y < area.bottom() {
            buf.set_string(area.x, y, "Ifaces", Style::default().fg(palette::HIGHLIGHT));
            y += 1;
        }
        for (name, hist) in self.hottest_interfaces(snapshot, 3) {
            if y >= area.bottom() {
                break;
            }
            let marker = if name == primary_name { '>' } else { ' ' };
            let line = format!("{marker}{} {}", truncate(name, 7), format_rate(hist.rx_rate.saturating_add(hist.tx_rate)));
            buf.set_string(area.x, y, truncate(&line, area.width as usize), Style::default().fg(palette::FG));
            y += 1;
        }
    }

    fn render_processes(&self, area: Rect, buf: &mut Buffer) {
        render_rule_line(area, buf, "Proc", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let body = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(1) };
        if body.height < 3 || body.width < 32 {
            return;
        }
        self.process_viewport_rows.set(body.height.saturating_sub(1) as usize);
        let pid_w = 7usize;
        let user_w = if body.width >= 58 { 10usize } else { 0usize };
        let cpu_w = 6usize;
        let mem_w = 10usize;
        let spacing = if user_w > 0 { 5 } else { 4 };
        let name_w = body.width.saturating_sub((pid_w + user_w + cpu_w + mem_w + spacing) as u16).max(10) as usize;
        let header = if user_w > 0 {
            format!("{:>pid_w$} {:<name_w$} {:<user_w$} {:>mem_w$} {:>cpu_w$}", "PID", "Program", "User", "Mem", "CPU%")
        } else {
            format!("{:>pid_w$} {:<name_w$} {:>mem_w$} {:>cpu_w$}", "PID", "Program", "Mem", "CPU%")
        };
        buf.set_string(body.x, body.y, truncate(&header, body.width as usize), Style::default().fg(palette::FORM_LABEL));

        let rows = snapshot.processes.len();
        let view_h = body.height.saturating_sub(1) as usize;
        let max_scroll = rows.saturating_sub(view_h);
        let start = self.processes_scroll.min(max_scroll);
        for (i, proc_row) in snapshot.processes.iter().skip(start).take(view_h).enumerate() {
            let y = body.y + 1 + i as u16;
            let line = if user_w > 0 {
                format!(
                    "{:>pid_w$} {:<name_w$} {:<user_w$} {:>mem_w$} {:>cpu_w$.1}",
                    proc_row.pid,
                    truncate(&proc_row.name, name_w),
                    truncate(&proc_row.user, user_w),
                    format_bytes(proc_row.memory_bytes),
                    proc_row.cpu_percent
                )
            } else {
                format!(
                    "{:>pid_w$} {:<name_w$} {:>mem_w$} {:>cpu_w$.1}",
                    proc_row.pid,
                    truncate(&proc_row.name, name_w),
                    format_bytes(proc_row.memory_bytes),
                    proc_row.cpu_percent
                )
            };
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

    fn primary_interface_name(&self, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) -> Option<String> {
        self.hottest_interfaces(snapshot, 1).into_iter().next().map(|(name, _)| name.to_string())
    }

    fn primary_network_history<'a>(
        &'a self, snapshot: &'a libsysinspect::console::ConsoleMinionTopSnapshot,
    ) -> Option<(&'a str, &'a NetworkHistory)> {
        self.hottest_interfaces(snapshot, 1).into_iter().next()
    }

    fn hottest_interfaces<'a>(
        &'a self, snapshot: &'a libsysinspect::console::ConsoleMinionTopSnapshot, limit: usize,
    ) -> Vec<(&'a str, &'a NetworkHistory)> {
        let mut rows: Vec<_> = snapshot
            .network_interfaces
            .iter()
            .filter_map(|iface| self.interface_networks.get(&iface.name).map(|hist| (iface.name.as_str(), hist)))
            .collect();
        rows.sort_by(|a, b| b.1.rx_rate.saturating_add(b.1.tx_rate).cmp(&a.1.rx_rate.saturating_add(a.1.tx_rate)).then_with(|| a.0.cmp(b.0)));
        rows.truncate(limit);
        rows
    }
}

fn update_network_history(history: &mut NetworkHistory, rx_total: u64, tx_total: u64) {
    history.rx_rate = history.last_rx_total.map(|prev| rx_total.saturating_sub(prev)).unwrap_or(0);
    history.tx_rate = history.last_tx_total.map(|prev| tx_total.saturating_sub(prev)).unwrap_or(0);
    history.last_rx_total = Some(rx_total);
    history.last_tx_total = Some(tx_total);
    history.rx_total = rx_total;
    history.tx_total = tx_total;
    history.peak_rate = history.peak_rate.max(history.rx_rate.max(history.tx_rate));
    push_history_u64(&mut history.rx_history, history.rx_rate);
    push_history_u64(&mut history.tx_history, history.tx_rate);
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

fn render_dot_bar(buf: &mut Buffer, area: Rect, pct: f32, active: ratatui::style::Color, inactive: ratatui::style::Color) {
    if area.width == 0 {
        return;
    }
    let filled = ((area.width as f32) * (pct.clamp(0.0, 100.0) / 100.0)).round() as u16;
    for idx in 0..area.width {
        let x = area.x + idx;
        let (symbol, style) = if idx < filled { ("•", Style::default().fg(active)) } else { ("·", Style::default().fg(inactive)) };
        buf.set_string(x, area.y, symbol, style);
    }
}

fn render_labeled_value(buf: &mut Buffer, x: u16, y: u16, width: u16, label: &str, value: &str) {
    if width == 0 {
        return;
    }
    let label_text = format!("{label}:");
    buf.set_string(x, y, truncate(&label_text, width as usize), Style::default().fg(palette::FORM_LABEL));
    let label_w = label_text.chars().count() as u16;
    if label_w + 1 < width {
        buf.set_string(x + label_w + 1, y, truncate(value, width.saturating_sub(label_w + 1) as usize), Style::default().fg(palette::FG));
    }
}

fn render_meter_row(buf: &mut Buffer, area: Rect, label: &str, pct: f32, color: ratatui::style::Color) {
    if area.width < 8 {
        return;
    }
    let label_text = format!("{label} ");
    buf.set_string(area.x, area.y, &label_text, Style::default().fg(palette::FORM_LABEL));
    let bar_x = area.x + label_text.chars().count() as u16;
    let pct_text = format!(" {:>3.0}%", pct);
    let pct_w = pct_text.chars().count() as u16;
    let bar_w = area.width.saturating_sub(label_text.chars().count() as u16 + pct_w).max(1);
    render_dot_bar(buf, Rect { x: bar_x, y: area.y, width: bar_w, height: 1 }, pct, color, palette::GRAY_0);
    buf.set_string(bar_x + bar_w, area.y, pct_text, Style::default().fg(palette::FG));
}

fn render_rule_line_parts(area: Rect, buf: &mut Buffer, parts: &[(&str, Style)], grad_start: ratatui::style::Color, grad_end: ratatui::style::Color) {
    if area.width < 6 {
        return;
    }
    let mut used = 0u16;
    for (text, style) in parts {
        let remaining = area.width.saturating_sub(1).saturating_sub(used) as usize;
        if remaining == 0 {
            break;
        }
        let out = truncate(text, remaining);
        let out_w = out.chars().count() as u16;
        buf.set_string(area.x + used, area.y, &out, *style);
        used += out_w;
    }
    let fill_start = area.x.saturating_add(used);
    let fill_end = area.right().saturating_sub(1);
    let fill_len = (fill_end.saturating_sub(fill_start)).max(1) as f64;
    for x in fill_start..fill_end.min(fill_start.saturating_add(area.width)) {
        let t = (x.saturating_sub(fill_start)) as f64 / fill_len;
        let color = ratatui_glamour::color::lerp_color(grad_start, grad_end, t as f32);
        buf.set_string(x, area.y, "/", Style::default().fg(color));
    }
}

fn render_mirrored_net_chart(buf: &mut Buffer, area: Rect, history: &NetworkHistory) {
    if area.width < 12 || area.height < 5 {
        return;
    }
    let label_w = 4u16;
    let plot = Rect { x: area.x + label_w, y: area.y, width: area.width.saturating_sub(label_w), height: area.height };
    if plot.width == 0 || plot.height < 3 {
        return;
    }
    let top_rows = (plot.height.saturating_sub(1) / 2).max(1);
    let bottom_rows = plot.height.saturating_sub(1).saturating_sub(top_rows).max(1);
    let center_y = plot.y + top_rows;
    let max_rate = history.rx_history.iter().chain(history.tx_history.iter()).copied().max().unwrap_or(0).max(1);
    let rx_samples = sample_history_u64(&history.rx_history, plot.width as usize);
    let tx_samples = sample_history_u64(&history.tx_history, plot.width as usize);

    buf.set_string(area.x, plot.y, "Dn", Style::default().fg(palette::SUCCESS));
    buf.set_string(area.x, center_y, "--", Style::default().fg(palette::PROCESSING_DIMMED));
    if center_y + 1 < plot.bottom() {
        buf.set_string(area.x, plot.bottom().saturating_sub(1), "Up", Style::default().fg(palette::PRIMARY));
    }
    for x in 0..plot.width {
        buf.set_string(plot.x + x, center_y, "─", Style::default().fg(palette::PROCESSING_DIMMED));
    }

    for (idx, sample) in rx_samples.iter().enumerate() {
        let x = plot.x + idx as u16;
        let units = scale_units(*sample, top_rows * 2, max_rate);
        render_vertical_fill_up(buf, x, center_y.saturating_sub(1), top_rows, units, palette::SUCCESS_PEAK);
    }
    for (idx, sample) in tx_samples.iter().enumerate() {
        let x = plot.x + idx as u16;
        let units = scale_units(*sample, bottom_rows * 2, max_rate);
        render_vertical_fill_down(buf, x, center_y + 1, bottom_rows, units, palette::PROCESSING_HEAT);
    }
}

fn sample_history_u64(history: &[u64], width: usize) -> Vec<u64> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = vec![0; width];
    let start = history.len().saturating_sub(width);
    let slice = &history[start..];
    let offset = width.saturating_sub(slice.len());
    for (idx, value) in slice.iter().enumerate() {
        out[offset + idx] = *value;
    }
    out
}

fn scale_units(value: u64, max_units: u16, max_value: u64) -> u16 {
    if max_units == 0 || max_value == 0 {
        return 0;
    }
    (((value as f64 / max_value as f64) * max_units as f64).round() as u16).min(max_units)
}

fn render_vertical_fill_up(buf: &mut Buffer, x: u16, baseline_y: u16, rows: u16, units: u16, color: ratatui::style::Color) {
    let mut remaining = units;
    for row in 0..rows {
        let y = baseline_y.saturating_sub(row);
        let symbol = if remaining >= 2 {
            remaining -= 2;
            Some("█")
        } else if remaining == 1 {
            remaining = 0;
            Some("▄")
        } else {
            None
        };
        if let Some(symbol) = symbol {
            buf.set_string(x, y, symbol, Style::default().fg(color));
        }
    }
}

fn render_vertical_fill_down(buf: &mut Buffer, x: u16, start_y: u16, rows: u16, units: u16, color: ratatui::style::Color) {
    let mut remaining = units;
    for row in 0..rows {
        let y = start_y.saturating_add(row);
        let symbol = if remaining >= 2 {
            remaining -= 2;
            Some("█")
        } else if remaining == 1 {
            remaining = 0;
            Some("▀")
        } else {
            None
        };
        if let Some(symbol) = symbol {
            buf.set_string(x, y, symbol, Style::default().fg(color));
        }
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
