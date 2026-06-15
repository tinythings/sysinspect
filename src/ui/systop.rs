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
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

const HISTORY_LEN: usize = 96;
const METER_CELL: &str = "■";

#[derive(Debug, Default, Clone)]
struct NetworkHistory {
    rx_history: Vec<u64>,
    tx_history: Vec<u64>,
    rx_rate: u64,
    tx_rate: u64,
    peak_rate: u64,
    rx_total: u64,
    tx_total: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ProcessSort {
    Pid,
    Name,
    Mem,
    #[default]
    Cpu,
}

/// Live state for the System top popup.
#[derive(Debug, Default)]
pub struct SystemTopState {
    pub visible: bool,
    pub minion_id: String,
    pub host: String,
    pub snapshot: Option<libsysinspect::console::ConsoleMinionTopSnapshot>,
    pub network_interface: Option<String>,
    pub cpu_history: Vec<f32>,
    total_network: NetworkHistory,
    interface_networks: BTreeMap<String, NetworkHistory>,
    pub last_fetch: Option<Instant>,
    pub processes_scroll: usize,
    pub process_viewport_rows: Cell<usize>,
    process_sort: ProcessSort,
    process_sort_asc: bool,
}

impl SystemTopState {
    /// Open the popup for one selected minion.
    pub fn open(&mut self, minion_id: String, host: String) {
        self.visible = true;
        self.minion_id = minion_id;
        self.host = host;
        self.snapshot = None;
        self.network_interface = None;
        self.cpu_history.clear();
        self.total_network = NetworkHistory::default();
        self.interface_networks.clear();
        self.last_fetch = None;
        self.processes_scroll = 0;
        self.process_sort = ProcessSort::Cpu;
        self.process_sort_asc = false;
    }

    /// Close the popup.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Apply one fresh snapshot.
    pub fn apply_snapshot(&mut self, snapshot: libsysinspect::console::ConsoleMinionTopSnapshot) {
        push_history(&mut self.cpu_history, snapshot.cpu_percent);
        update_network_history(
            &mut self.total_network,
            snapshot.network_rx_rate_bytes_per_sec,
            snapshot.network_tx_rate_bytes_per_sec,
            snapshot.network_rx_total_bytes,
            snapshot.network_tx_total_bytes,
        );

        let mut present = BTreeSet::new();
        for iface in &snapshot.network_interfaces {
            present.insert(iface.name.clone());
            update_network_history(
                self.interface_networks.entry(iface.name.clone()).or_default(),
                iface.rx_rate_bytes_per_sec,
                iface.tx_rate_bytes_per_sec,
                iface.rx_total_bytes,
                iface.tx_total_bytes,
            );
        }
        if self.network_interface.as_ref().is_some_and(|iface| !present.contains(iface)) {
            self.network_interface = None;
        }
        if self.network_interface.is_none() {
            self.network_interface = snapshot.network_interfaces.iter().map(|iface| iface.name.clone()).min();
        }
        self.interface_networks.retain(|name, _| present.contains(name));
        self.last_fetch = Some(Instant::now());
        self.snapshot = Some(snapshot);
    }

    /// Cycle the selected network interface.
    pub fn cycle_network_interface(&mut self, forward: bool) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let names = self.ordered_interfaces(snapshot).into_iter().map(|(name, _)| name.to_string()).collect::<Vec<_>>();
        if names.is_empty() {
            self.network_interface = None;
            return;
        }
        let current_idx = self
            .network_interface
            .as_deref()
            .or_else(|| names.first().map(String::as_str))
            .and_then(|iface| names.iter().position(|name| name == iface))
            .unwrap_or(0);
        let next_idx = if forward { (current_idx + 1) % names.len() } else { current_idx.checked_sub(1).unwrap_or(names.len() - 1) };
        self.network_interface = Some(names[next_idx].clone());
    }

    /// Apply one process sort key.
    pub fn apply_sort_key(&mut self, key: char) {
        let next = match key.to_ascii_lowercase() {
            'c' => Some(ProcessSort::Cpu),
            'm' => Some(ProcessSort::Mem),
            'p' => Some(ProcessSort::Pid),
            'n' => Some(ProcessSort::Name),
            _ => None,
        };
        if let Some(next) = next {
            if self.process_sort == next {
                self.process_sort_asc = !self.process_sort_asc;
            } else {
                self.process_sort = next;
                self.process_sort_asc = matches!(next, ProcessSort::Name | ProcessSort::Pid);
            }
            self.processes_scroll = 0;
        }
    }

    /// Render the popup.
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

        let cpu_h = 5.min(inner.height);
        let cpu_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: cpu_h };
        let lower_area = Rect { x: inner.x, y: inner.y + cpu_h, width: inner.width, height: inner.height.saturating_sub(cpu_h) };
        let left_w = ((lower_area.width * 43) / 100).clamp(34, lower_area.width.saturating_sub(28));
        let left_area = Rect { x: lower_area.x, y: lower_area.y, width: left_w, height: lower_area.height };
        let right_area = Rect { x: left_area.right(), y: lower_area.y, width: lower_area.width.saturating_sub(left_w), height: lower_area.height };
        let mem_h = 4.min(left_area.height);
        let left_rest = left_area.height.saturating_sub(mem_h);
        let min_net = left_rest.min(8);
        let max_disk = left_rest.saturating_sub(min_net);
        let want_disk = self.snapshot.as_ref().map_or(4, |snapshot| {
            let rows_per_disk = if max_disk >= 8 { 2 } else { 1 };
            let body_rows = rows_per_disk * snapshot.disks.len().min(4) as u16;
            1 + body_rows
        });
        let disk_h = if left_rest <= 4 { left_rest } else { want_disk.clamp(4, max_disk.max(4)) };
        let net_h = left_rest.saturating_sub(disk_h);
        let mem_area = Rect { x: left_area.x, y: left_area.y, width: left_area.width, height: mem_h };
        let disks_area = Rect { x: left_area.x, y: left_area.y + mem_h, width: left_area.width, height: disk_h };
        let net_area = Rect { x: left_area.x, y: disks_area.bottom(), width: left_area.width, height: net_h };

        self.render_cpu(cpu_area, buf);
        self.render_memory(mem_area, buf);
        self.render_disks(disks_area, buf);
        self.render_network(net_area, buf);
        self.render_processes(right_area, buf);
        render_shadow(buf, canvas);
    }

    fn render_cpu(&self, area: Rect, buf: &mut Buffer) {
        let Some(snapshot) = &self.snapshot else {
            render_rule_line(area, buf, "CPU", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
            Widget::render(Paragraph::new(" collecting... ").style(Style::default().fg(palette::FG).bg(palette::BG_2)), area, buf);
            return;
        };
        let mut title = format!(
            " {:>4.1}%  Uptime:{}  Load {:.2}/{:.2}/{:.2} ",
            snapshot.cpu_percent,
            format_uptime(snapshot.uptime_secs),
            snapshot.load_avg_one,
            snapshot.load_avg_five,
            snapshot.load_avg_fifteen
        );
        if let Some(temp) = snapshot.cpu_temp_celsius {
            title = format!(
                " {:>4.1}%  Temp:{temp:>4.0}C  Uptime:{}  Load {:.2}/{:.2}/{:.2} ",
                snapshot.cpu_percent,
                format_uptime(snapshot.uptime_secs),
                snapshot.load_avg_one,
                snapshot.load_avg_five,
                snapshot.load_avg_fifteen
            );
        }
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
        let graph_w = ((body.width * 2) / 5).clamp(22, body.width.saturating_sub(18));
        let graph_area = Rect { x: body.x, y: body.y, width: graph_w, height: body.height };
        let cores_area = Rect { x: graph_area.right(), y: body.y, width: body.width.saturating_sub(graph_w), height: body.height };
        self.render_cpu_braille(graph_area, buf, snapshot);
        self.render_core_bars(cores_area, buf, snapshot);
    }

    fn render_cpu_braille(&self, area: Rect, buf: &mut Buffer, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) {
        if area.height < 2 || area.width < 12 {
            return;
        }
        let flame = braille_line(&self.cpu_history, area.width.saturating_sub(2) as usize);
        buf.set_string(area.x + 1, area.y, flame, Style::default().fg(cpu_chart_color(snapshot.cpu_percent)));
        if area.height > 1 {
            let mut detail = format!("Trend {:>4.1}%  Cores {}", snapshot.cpu_percent, snapshot.cpu_per_core.len());
            if let Some(temp) = snapshot.cpu_temp_celsius {
                detail = format!("Trend {:>4.1}%  Cores {}  Temp {temp:>4.0}C", snapshot.cpu_percent, snapshot.cpu_per_core.len());
            }
            buf.set_string(
                area.x + 1,
                area.bottom().saturating_sub(1),
                truncate(&detail, area.width.saturating_sub(2) as usize),
                Style::default().fg(palette::FG),
            );
        }
    }

    fn render_core_bars(&self, area: Rect, buf: &mut Buffer, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) {
        if area.height == 0 || area.width < 14 {
            return;
        }
        let rows = area.height as usize;
        let cols = snapshot.cpu_per_core.len().div_ceil(rows.max(1)).clamp(1, 4);
        let per_col = snapshot.cpu_per_core.len().div_ceil(cols).max(1);
        let col_w = (area.width / cols as u16).max(1);
        for (idx, cpu) in snapshot.cpu_per_core.iter().enumerate() {
            let col = idx / per_col;
            let row = idx % per_col;
            let x = area.x + col as u16 * col_w;
            let y = area.y + row as u16;
            if y >= area.bottom() {
                break;
            }
            let show_pct = col_w >= 15;
            let label = format!("C{:02}", idx);
            let pct_w = if show_pct { 5 } else { 0 };
            let bar_x = x + 4;
            let bar_w = col_w.saturating_sub(5 + pct_w).max(3);
            buf.set_string(x, y, &label, Style::default().fg(palette::FORM_LABEL));
            render_percent_bar(buf, Rect { x: bar_x, y, width: bar_w, height: 1 }, *cpu);
            if show_pct {
                buf.set_string(bar_x + bar_w + 1, y, format!("{:>3.0}%", cpu), Style::default().fg(cpu_chart_color(*cpu)));
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
            render_meter_row(buf, Rect { x: body.x, y: body.y + 2, width: left_w.saturating_sub(1), height: 1 }, "RAM", ram_pct);
            render_meter_row(buf, Rect { x: body.x + left_w, y: body.y + 2, width: right_w, height: 1 }, "SWP", swap_pct);
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
        let rows_per_disk = if body.height >= 7 { 2 } else { 1 };
        let visible = (body.height as usize / rows_per_disk).max(1);
        for (idx, disk) in snapshot.disks.iter().take(visible).enumerate() {
            let y = body.y + (idx * rows_per_disk) as u16;
            let name = if disk.mount_point.is_empty() { disk.name.as_str() } else { disk.mount_point.as_str() };
            let line =
                format!("{} {:>5.0}% {} / {}", truncate(name, 10), disk.used_percent, format_bytes(disk.used_bytes), format_bytes(disk.total_bytes));
            buf.set_string(body.x, y, truncate(&line, body.width as usize), Style::default().fg(palette::FG));
            if rows_per_disk == 2 && y + 1 < body.bottom() {
                render_square_meter(buf, Rect { x: body.x, y: y + 1, width: body.width, height: 1 }, disk.used_percent, palette::GRAY_0);
            }
        }
    }

    fn render_network(&self, area: Rect, buf: &mut Buffer) {
        let Some(snapshot) = &self.snapshot else {
            render_rule_line(area, buf, "Net", Style::default().fg(palette::HIGHLIGHT), palette::PRIMARY, palette::PROCESSING_DIMMED);
            return;
        };
        let primary_name = self.primary_interface_name(snapshot).unwrap_or_else(|| "all".to_string());
        render_rule_line_parts(
            area,
            buf,
            &[
                ("Net ", Style::default().fg(palette::HIGHLIGHT)),
                (primary_name.as_str(), Style::default().fg(palette::PRIMARY).add_modifier(Modifier::BOLD)),
            ],
            palette::PRIMARY,
            palette::PROCESSING_DIMMED,
        );
        let inner = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(1) };
        if inner.height < 6 || inner.width < 24 {
            return;
        }
        let info_w = (inner.width / 3).clamp(14, 18);
        let chart_area = Rect { x: inner.x, y: inner.y, width: inner.width.saturating_sub(info_w), height: inner.height };
        let info_area = Rect { x: chart_area.right(), y: inner.y, width: info_w, height: inner.height };
        let primary = self.primary_network_history(snapshot).unwrap_or((&primary_name, &self.total_network));
        render_mirrored_net_chart(buf, chart_area, primary.1);
        self.render_network_details(info_area, snapshot, primary.0, primary.1, &primary_name, buf);
    }

    fn render_network_details(
        &self, area: Rect, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot, iface_name: &str, primary: &NetworkHistory,
        primary_name: &str, buf: &mut Buffer,
    ) {
        if area.width < 10 || area.height == 0 {
            return;
        }
        let mut y = area.y;
        for (text, style) in [
            (iface_name.to_string(), Style::default().fg(palette::FORM_LABEL).add_modifier(Modifier::BOLD)),
            (format!("▼ {}", format_rate(primary.rx_rate)), Style::default().fg(palette::SUCCESS)),
            (format!("▲ {}", format_rate(primary.tx_rate)), Style::default().fg(palette::PRIMARY)),
            (format!("Pk {}", format_rate(primary.peak_rate)), Style::default().fg(palette::FG)),
            (format!("Rx {}", format_bytes(primary.rx_total)), Style::default().fg(palette::FG)),
            (format!("Tx {}", format_bytes(primary.tx_total)), Style::default().fg(palette::FG)),
        ] {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(area.x, y, truncate(&text, area.width as usize), style);
            y += 1;
        }
        if y < area.bottom() {
            buf.set_string(area.x, y, "Ifaces", Style::default().fg(palette::HIGHLIGHT));
            y += 1;
        }
        let visible = area.bottom().saturating_sub(y) as usize;
        let rows = self.visible_interfaces(snapshot, visible);
        let name_w = rows.iter().map(|(name, _)| name.chars().count()).max().unwrap_or(0).min(area.width.saturating_sub(6) as usize).max(1);
        let marker_x = area.x;
        let name_x = area.x + 2;
        let rate_x = name_x + name_w as u16 + 1;
        let rate_w = area.right().saturating_sub(rate_x) as usize;
        for (name, hist) in rows {
            if y >= area.bottom() {
                break;
            }
            let marker = if name == primary_name { '▶' } else { ' ' };
            buf.set_string(marker_x, y, marker.to_string(), Style::default().fg(palette::FG));
            let name_text = truncate(name, name_w);
            buf.set_string(name_x, y, format!("{name_text:<name_w$}"), Style::default().fg(palette::GRAY_1));
            let rate_text = format_rate(hist.rx_rate.saturating_add(hist.tx_rate));
            if rate_w > 0 {
                let out = truncate(&rate_text, rate_w);
                buf.set_string(rate_x, y, format!("{out:>rate_w$}"), Style::default().fg(palette::FG));
            }
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
        let pid_x = body.x;
        let name_x = pid_x + pid_w as u16 + 1;
        let user_x = if user_w > 0 { Some(name_x + name_w as u16 + 1) } else { None };
        let mem_x = user_x.map_or(name_x + name_w as u16 + 1, |x| x + user_w as u16 + 1);
        let cpu_x = mem_x + mem_w as u16 + 1;
        render_header_cell(
            buf,
            Rect { x: pid_x, y: body.y, width: pid_w as u16, height: 1 },
            "PID",
            self.process_sort == ProcessSort::Pid,
            self.process_sort_asc,
            true,
        );
        render_header_cell(
            buf,
            Rect { x: name_x, y: body.y, width: name_w as u16, height: 1 },
            "Program",
            self.process_sort == ProcessSort::Name,
            self.process_sort_asc,
            false,
        );
        if let Some(user_x) = user_x {
            buf.set_string(user_x, body.y, format!("{:<user_w$}", "User"), Style::default().fg(palette::FORM_LABEL));
        }
        render_header_cell(
            buf,
            Rect { x: mem_x, y: body.y, width: mem_w as u16, height: 1 },
            "Mem",
            self.process_sort == ProcessSort::Mem,
            self.process_sort_asc,
            true,
        );
        render_header_cell(
            buf,
            Rect { x: cpu_x, y: body.y, width: cpu_w as u16, height: 1 },
            "CPU%",
            self.process_sort == ProcessSort::Cpu,
            self.process_sort_asc,
            true,
        );

        let rows = self.sorted_processes(snapshot);
        let view_h = body.height.saturating_sub(1) as usize;
        let max_scroll = rows.len().saturating_sub(view_h);
        let start = self.processes_scroll.min(max_scroll);
        for (idx, proc_row) in rows.iter().skip(start).take(view_h).enumerate() {
            let y = body.y + 1 + idx as u16;
            render_value_cell(buf, pid_x, y, pid_w, &format!("{:>pid_w$}", proc_row.pid), self.process_sort == ProcessSort::Pid);
            render_value_cell(
                buf,
                name_x,
                y,
                name_w,
                &format!("{:<name_w$}", truncate(&proc_row.name, name_w)),
                self.process_sort == ProcessSort::Name,
            );
            if let Some(user_x) = user_x {
                buf.set_string(user_x, y, format!("{:<user_w$}", truncate(&proc_row.user, user_w)), Style::default().fg(palette::GRAY_1));
            }
            render_value_cell(
                buf,
                mem_x,
                y,
                mem_w,
                &format!("{:>mem_w$}", format_bytes(proc_row.memory_bytes)),
                self.process_sort == ProcessSort::Mem,
            );
            render_value_cell(buf, cpu_x, y, cpu_w, &format!("{:>cpu_w$.1}", proc_row.cpu_percent), self.process_sort == ProcessSort::Cpu);
        }
        let scroll_area = Rect { x: area.right().saturating_sub(1), y: body.y + 1, width: 1, height: body.height.saturating_sub(1) };
        let mut scroll = ScrollbarState::default().content_length(rows.len().max(1)).position(start);
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
        self.primary_network_history(snapshot).map(|(name, _)| name.to_string())
    }

    fn primary_network_history<'a>(
        &'a self, snapshot: &'a libsysinspect::console::ConsoleMinionTopSnapshot,
    ) -> Option<(&'a str, &'a NetworkHistory)> {
        if let Some(selected) = self.network_interface.as_deref()
            && let Some(row) = snapshot.network_interfaces.iter().find(|iface| iface.name == selected)
            && let Some(history) = self.interface_networks.get(&row.name)
        {
            return Some((row.name.as_str(), history));
        }
        self.ordered_interfaces(snapshot).into_iter().next()
    }

    fn ordered_interfaces<'a>(&'a self, snapshot: &'a libsysinspect::console::ConsoleMinionTopSnapshot) -> Vec<(&'a str, &'a NetworkHistory)> {
        let mut rows: Vec<_> = snapshot
            .network_interfaces
            .iter()
            .filter_map(|iface| self.interface_networks.get(&iface.name).map(|hist| (iface.name.as_str(), hist)))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        rows
    }

    fn visible_interfaces<'a>(
        &'a self, snapshot: &'a libsysinspect::console::ConsoleMinionTopSnapshot, limit: usize,
    ) -> Vec<(&'a str, &'a NetworkHistory)> {
        let rows = self.ordered_interfaces(snapshot);
        if rows.len() <= limit || limit == 0 {
            return rows;
        }
        if let Some(selected) = self.network_interface.as_deref()
            && let Some(pos) = rows.iter().position(|(name, _)| *name == selected)
        {
            let mut start = pos.saturating_sub(limit.saturating_sub(1));
            let end = (start + limit).min(rows.len());
            start = end.saturating_sub(limit);
            return rows[start..end].to_vec();
        }
        rows.into_iter().take(limit).collect()
    }

    fn sorted_processes<'a>(
        &self, snapshot: &'a libsysinspect::console::ConsoleMinionTopSnapshot,
    ) -> Vec<&'a libsysinspect::console::ConsoleMinionTopProcess> {
        let mut rows: Vec<_> = snapshot.processes.iter().collect();
        rows.sort_by(|a, b| {
            let ord = match self.process_sort {
                ProcessSort::Cpu => a.cpu_percent.partial_cmp(&b.cpu_percent).unwrap_or(Ordering::Equal),
                ProcessSort::Mem => a.memory_bytes.cmp(&b.memory_bytes),
                ProcessSort::Pid => a.pid.cmp(&b.pid),
                ProcessSort::Name => a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()),
            };
            let ord = if self.process_sort_asc { ord } else { ord.reverse() };
            ord.then_with(|| a.pid.cmp(&b.pid))
        });
        rows
    }
}

fn render_shadow(buf: &mut Buffer, canvas: Rect) {
    let buf_area = buf.area();
    let max_x = buf_area.right().saturating_sub(1);
    let max_y = buf_area.bottom().saturating_sub(1);
    for idx in 0..canvas.width {
        let sx = canvas.x.saturating_add(2).saturating_add(idx);
        let sy = canvas.y.saturating_add(canvas.height);
        if sx > max_x || sy > max_y {
            continue;
        }
        if let Some(cell) = buf.cell_mut(Position::new(sx, sy)) {
            cell.set_bg(palette::SHADOW_BG);
            cell.set_fg(palette::SHADOW_FG);
        }
    }
    for offset in 0..2u16 {
        for idx in 0..canvas.height {
            let sx = canvas.x.saturating_add(canvas.width).saturating_add(offset);
            let sy = canvas.y.saturating_add(idx).saturating_add(1);
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

fn update_network_history(history: &mut NetworkHistory, rx_rate: u64, tx_rate: u64, rx_total: u64, tx_total: u64) {
    history.rx_rate = rx_rate;
    history.tx_rate = tx_rate;
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

fn cpu_chart_color(value: f32) -> ratatui::style::Color {
    ratatui_glamour::color::lerp_color(palette::SUCCESS_BASE, palette::ERROR, value.clamp(0.0, 100.0) / 100.0)
}

fn meter_band_color(value: f32) -> ratatui::style::Color {
    match value {
        v if v < 20.0 => palette::PROCESSING_GLOW,
        v if v < 40.0 => palette::PROCESSING_PEAK,
        v if v < 60.0 => palette::WARNING_HEAT,
        v if v < 80.0 => palette::ERROR_HEAT,
        _ => palette::ERROR,
    }
}

fn render_percent_bar(buf: &mut Buffer, area: Rect, pct: f32) {
    let pct = pct.clamp(0.0, 100.0);
    let filled = ((area.width as f32) * (pct / 100.0)).round() as u16;
    for idx in 0..area.width {
        let x = area.x + idx;
        let style = if idx < filled { Style::default().fg(cpu_chart_color(pct)) } else { Style::default().fg(palette::GRAY_0) };
        buf.set_string(x, area.y, if idx < filled { "█" } else { "░" }, style);
    }
}

fn render_square_meter(buf: &mut Buffer, area: Rect, pct: f32, inactive: ratatui::style::Color) {
    if area.width == 0 {
        return;
    }
    let pct = pct.clamp(0.0, 100.0);
    let filled = ((area.width as f32) * (pct / 100.0)).round() as u16;
    let fill_color = meter_band_color(pct);
    for idx in 0..area.width {
        let x = area.x + idx;
        let style = if idx < filled { Style::default().fg(fill_color) } else { Style::default().fg(inactive) };
        buf.set_string(x, area.y, METER_CELL, style);
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

fn render_meter_row(buf: &mut Buffer, area: Rect, label: &str, pct: f32) {
    if area.width < 8 {
        return;
    }
    let label_text = format!("{label} ");
    buf.set_string(area.x, area.y, &label_text, Style::default().fg(palette::FORM_LABEL));
    let label_w = label_text.chars().count() as u16;
    let pct_text = format!(" {:>3.0}%", pct);
    let pct_w = pct_text.chars().count() as u16;
    let meter_w = area.width.saturating_sub(label_w + pct_w).max(1);
    render_square_meter(buf, Rect { x: area.x + label_w, y: area.y, width: meter_w, height: 1 }, pct, palette::GRAY_0);
    buf.set_string(area.x + label_w + meter_w, area.y, pct_text, Style::default().fg(palette::FG));
}

fn render_header_cell(buf: &mut Buffer, area: Rect, label: &str, active: bool, asc: bool, right_align: bool) {
    let arrow = if active { if asc { "↑" } else { "↓" } } else { "" };
    let width = area.width as usize;
    let base = truncate(label, width.saturating_sub(arrow.chars().count()));
    let used = base.chars().count() + arrow.chars().count();
    let start_x = if right_align { area.x + width.saturating_sub(used) as u16 } else { area.x };
    let style = if active { Style::default().fg(palette::SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(palette::FORM_LABEL) };
    buf.set_string(start_x, area.y, &base, style);
    if !arrow.is_empty() {
        buf.set_string(start_x + base.chars().count() as u16, area.y, arrow, Style::default().fg(palette::PRIMARY));
    }
}

fn render_value_cell(buf: &mut Buffer, x: u16, y: u16, width: usize, text: &str, active: bool) {
    let style = if active { Style::default().fg(palette::FG) } else { Style::default().fg(palette::GRAY_1) };
    buf.set_string(x, y, truncate(text, width), style);
}

fn render_rule_line_parts(area: Rect, buf: &mut Buffer, parts: &[(&str, Style)], grad_start: ratatui::style::Color, grad_end: ratatui::style::Color) {
    if area.width < 6 {
        return;
    }
    let mut used = 0u16;
    for (text, style) in parts {
        if text.is_empty() {
            continue;
        }
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
    if area.width < 12 || area.height < 4 {
        return;
    }
    let label_w = 4u16;
    let plot = Rect { x: area.x + label_w, y: area.y, width: area.width.saturating_sub(label_w), height: area.height };
    if plot.width == 0 || plot.height < 2 {
        return;
    }
    let top_rows = (plot.height / 2).max(1);
    let bottom_rows = plot.height.saturating_sub(top_rows).max(1);
    let split_y = plot.y + top_rows;
    let max_rate = history.rx_history.iter().chain(history.tx_history.iter()).copied().max().unwrap_or(0).max(1);
    let rx_samples = sample_history_u64(&history.rx_history, plot.width as usize);
    let tx_samples = sample_history_u64(&history.tx_history, plot.width as usize);

    buf.set_string(area.x, plot.y, "Dn", Style::default().fg(palette::SUCCESS));
    buf.set_string(area.x, plot.bottom().saturating_sub(1), "Up", Style::default().fg(palette::PRIMARY));

    for (idx, sample) in rx_samples.iter().enumerate() {
        render_vertical_fill_up(
            buf,
            plot.x + idx as u16,
            split_y.saturating_sub(1),
            top_rows,
            scale_units(*sample, top_rows * 2, max_rate),
            palette::SUCCESS_BASE,
            palette::SUCCESS_PEAK,
        );
    }
    for (idx, sample) in tx_samples.iter().enumerate() {
        render_vertical_fill_down(
            buf,
            plot.x + idx as u16,
            split_y,
            bottom_rows,
            scale_units(*sample, bottom_rows * 2, max_rate),
            palette::PROCESSING_BASE,
            palette::PROCESSING_PEAK,
        );
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

fn render_vertical_fill_up(
    buf: &mut Buffer, x: u16, baseline_y: u16, rows: u16, units: u16, cold: ratatui::style::Color, hot: ratatui::style::Color,
) {
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
            let t = if rows > 1 { row as f32 / (rows - 1) as f32 } else { 1.0 };
            buf.set_string(x, y, symbol, Style::default().fg(ratatui_glamour::color::lerp_color(hot, cold, t)));
        }
    }
}

fn render_vertical_fill_down(buf: &mut Buffer, x: u16, start_y: u16, rows: u16, units: u16, cold: ratatui::style::Color, hot: ratatui::style::Color) {
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
            let t = if rows > 1 { row as f32 / (rows - 1) as f32 } else { 1.0 };
            buf.set_string(x, y, symbol, Style::default().fg(ratatui_glamour::color::lerp_color(hot, cold, t)));
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
    for idx in 0..width {
        let left = points.get(idx * 2).copied().unwrap_or(0.0);
        let right = points.get(idx * 2 + 1).copied().unwrap_or(left);
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
