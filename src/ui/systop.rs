use super::{
    palette,
    title::{self, TitleSegment, TitleStyle},
    wgt::render_rule_line,
};
use ratatui::{
    layout::{Position, Rect},
    prelude::{Buffer, StatefulWidget, Widget},
    style::{Color, Modifier, Style},
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
const CPU_FLAME_PALETTE: [Color; 6] =
    [Color::Indexed(198), Color::Indexed(204), Color::Indexed(210), Color::Indexed(216), Color::Indexed(222), Color::Indexed(228)];
const CPU_AVG_LINE_COLOR: Color = Color::Indexed(193);

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChartMode {
    #[default]
    Blocks,
    Line,
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
    cpu_core_history: Vec<Vec<f32>>,
    chart_mode: ChartMode,
    default_chart_mode: ChartMode,
    total_network: NetworkHistory,
    interface_networks: BTreeMap<String, NetworkHistory>,
    pub last_fetch: Option<Instant>,
    pub processes_scroll: usize,
    pub process_viewport_rows: Cell<usize>,
    process_sort: ProcessSort,
    default_process_sort: ProcessSort,
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
        self.cpu_core_history.clear();
        self.chart_mode = ChartMode::Blocks;
        self.total_network = NetworkHistory::default();
        self.interface_networks.clear();
        self.last_fetch = None;
        self.processes_scroll = 0;
        self.process_sort = self.default_process_sort;
        self.process_sort_asc = matches!(self.process_sort, ProcessSort::Name | ProcessSort::Pid);
        self.chart_mode = self.default_chart_mode;
    }

    /// Close the popup.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Apply one fresh snapshot.
    pub fn apply_snapshot(&mut self, snapshot: libsysinspect::console::ConsoleMinionTopSnapshot) {
        push_history(&mut self.cpu_history, snapshot.cpu_percent);
        if self.cpu_core_history.len() != snapshot.cpu_per_core.len() {
            self.cpu_core_history = vec![Vec::new(); snapshot.cpu_per_core.len()];
        }
        for (history, value) in self.cpu_core_history.iter_mut().zip(&snapshot.cpu_per_core) {
            push_history(history, *value);
        }
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

    pub(crate) fn set_chart_mode(&mut self, mode: ChartMode) {
        self.chart_mode = mode;
        self.default_chart_mode = mode;
    }

    pub(crate) fn set_persisted_preferences(
        &mut self, sort: libsysinspect::cfg::mmconf::ConsoleSystemTopSort, graph: libsysinspect::cfg::mmconf::ConsoleSystemTopGraph,
    ) {
        self.default_process_sort = match sort {
            libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Cpu => ProcessSort::Cpu,
            libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Mem => ProcessSort::Mem,
            libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Pid => ProcessSort::Pid,
            libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Name => ProcessSort::Name,
        };
        self.process_sort = self.default_process_sort;
        self.process_sort_asc = matches!(self.process_sort, ProcessSort::Name | ProcessSort::Pid);
        self.default_chart_mode = match graph {
            libsysinspect::cfg::mmconf::ConsoleSystemTopGraph::Blocks => ChartMode::Blocks,
            libsysinspect::cfg::mmconf::ConsoleSystemTopGraph::Line => ChartMode::Line,
        };
        self.chart_mode = self.default_chart_mode;
    }

    pub(crate) fn persisted_sort(&self) -> libsysinspect::cfg::mmconf::ConsoleSystemTopSort {
        match self.default_process_sort {
            ProcessSort::Cpu => libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Cpu,
            ProcessSort::Mem => libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Mem,
            ProcessSort::Pid => libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Pid,
            ProcessSort::Name => libsysinspect::cfg::mmconf::ConsoleSystemTopSort::Name,
        }
    }

    pub(crate) fn persisted_graph(&self) -> libsysinspect::cfg::mmconf::ConsoleSystemTopGraph {
        match self.default_chart_mode {
            ChartMode::Blocks => libsysinspect::cfg::mmconf::ConsoleSystemTopGraph::Blocks,
            ChartMode::Line => libsysinspect::cfg::mmconf::ConsoleSystemTopGraph::Line,
        }
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
            self.default_process_sort = self.process_sort;
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

        let core_count = self.snapshot.as_ref().map_or(1usize, |snapshot| snapshot.cpu_per_core.len().max(1));
        let max_cpu_h = inner.height.saturating_sub(8).max(5);
        let cpu_body_rows = choose_cpu_body_rows(core_count, inner.width, max_cpu_h.saturating_sub(1) as usize);
        let cpu_h = (cpu_body_rows as u16 + 1).clamp(5, max_cpu_h);
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
        if body.height < 2 {
            return;
        }
        let (_, cols, col_w) = cpu_core_layout(snapshot.cpu_per_core.len().max(1), body.width, body.height as usize);
        let cores_w = cols as u16 * col_w;
        let chart_area = Rect { x: body.x, y: body.y + 1, width: body.width.saturating_sub(cores_w), height: body.height.saturating_sub(1) };
        let cores_area = Rect { x: body.right().saturating_sub(cores_w), y: body.y + 1, width: cores_w, height: body.height.saturating_sub(1) };
        let divider_x = chart_area.right();
        let left_label = match self.chart_mode {
            ChartMode::Blocks => " CPU Flame ",
            ChartMode::Line => " Average Load ",
        };
        buf.set_string(
            body.x + 1,
            body.y,
            truncate(left_label, chart_area.width.saturating_sub(2) as usize),
            Style::default().fg(palette::HIGHLIGHT),
        );
        if divider_x < body.right() {
            buf.set_string(divider_x, body.y, "│", Style::default().fg(palette::GRAY_0));
            for y in chart_area.y..chart_area.bottom() {
                buf.set_string(divider_x, y, "│", Style::default().fg(palette::GRAY_0));
            }
        }
        buf.set_string(
            divider_x.saturating_add(2),
            body.y,
            truncate(" Per Core ", cores_area.width.saturating_sub(2) as usize),
            Style::default().fg(palette::HIGHLIGHT),
        );
        match self.chart_mode {
            ChartMode::Blocks => self.render_cpu_flame(chart_area, buf, snapshot),
            ChartMode::Line => self.render_cpu_average_line(chart_area, buf, snapshot),
        }
        self.render_core_compact(cores_area, buf, snapshot, cols, col_w);
    }

    fn render_cpu_flame(&self, area: Rect, buf: &mut Buffer, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) {
        if area.height < 2 || area.width < 12 {
            return;
        }
        let plot = Rect { x: area.x + 1, y: area.y, width: area.width.saturating_sub(2), height: area.height };
        if plot.width == 0 || plot.height == 0 || self.cpu_core_history.is_empty() {
            return;
        }
        let core_samples: Vec<_> = self.cpu_core_history.iter().map(|history| sample_history_f32(history, plot.width as usize)).collect();
        let avg_samples = sample_history_f32(&self.cpu_history, plot.width as usize);
        let temp_factor =
            self.snapshot.as_ref().and_then(|snapshot| snapshot.cpu_temp_celsius).map(|temp| ((temp - 45.0) / 45.0).clamp(0.0, 1.0)).unwrap_or(0.0);
        for x_idx in 0..plot.width as usize {
            let mut values = core_samples.iter().filter_map(|samples| samples.get(x_idx).copied()).filter(|value| *value > 0.0).collect::<Vec<_>>();
            if values.is_empty() {
                continue;
            }
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
            let avg_cpu = avg_samples.get(x_idx).copied().unwrap_or(snapshot.cpu_percent).clamp(0.0, 100.0);
            let filled_rows = (((avg_cpu / 100.0) * plot.height as f32).round() as u16).min(plot.height);
            if filled_rows == 0 {
                continue;
            }
            let bands = proportional_rows(&values, filled_rows);
            let mut y = plot.bottom().saturating_sub(1);
            for (value, rows) in values.iter().zip(bands.iter()) {
                for _ in 0..*rows {
                    if y < plot.y {
                        break;
                    }
                    let vertical_t =
                        if plot.height <= 1 { 1.0 } else { 1.0 - (y.saturating_sub(plot.y) as f32 / plot.height.saturating_sub(1) as f32) };
                    let heat = ((*value / 100.0) * 0.75 + (avg_cpu / 100.0) * 0.15 + vertical_t * 0.05 + temp_factor * 0.05).clamp(0.0, 1.0);
                    let color = cpu_flame_color(heat);
                    buf.set_string(plot.x + x_idx as u16, y, "█", Style::default().fg(color));
                    if y == 0 {
                        break;
                    }
                    y = y.saturating_sub(1);
                }
                if y < plot.y {
                    break;
                }
            }
        }
    }

    fn render_cpu_average_line(&self, area: Rect, buf: &mut Buffer, _snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot) {
        let plot = Rect { x: area.x + 1, y: area.y, width: area.width.saturating_sub(2), height: area.height };
        render_braille_line_overlay(buf, plot, &self.cpu_history, CPU_AVG_LINE_COLOR);
    }

    fn render_core_compact(
        &self, area: Rect, buf: &mut Buffer, snapshot: &libsysinspect::console::ConsoleMinionTopSnapshot, cols: usize, col_w: u16,
    ) {
        if area.height == 0 || area.width < 14 {
            return;
        }
        let rows = snapshot.cpu_per_core.len().div_ceil(cols.max(1)).max(1);
        let per_col = rows;
        for (idx, cpu) in snapshot.cpu_per_core.iter().enumerate() {
            let col = idx / per_col;
            let row = idx % per_col;
            let x = area.x + col as u16 * col_w;
            let y = area.y + row as u16;
            if y >= area.bottom() {
                break;
            }
            let label = format!("C{:02}", idx);
            let pct_text = format!("{:>3.0}%", cpu);
            let meter_w = col_w.saturating_sub(9).clamp(4, 7) as usize;
            let meter = compact_core_meter(*cpu, meter_w);
            buf.set_string(x, y, &label, Style::default().fg(palette::FORM_LABEL));
            for (offset, ch) in meter.chars().enumerate() {
                let style = if ch == '█' { Style::default().fg(cpu_chart_color(*cpu)) } else { Style::default().fg(palette::GRAY_0) };
                buf.set_string(x + 4 + offset as u16, y, ch.to_string(), style);
            }
            buf.set_string(x + 5 + meter_w as u16, y, pct_text, Style::default().fg(cpu_chart_color(*cpu)));
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
        match self.chart_mode {
            ChartMode::Blocks => render_mirrored_net_chart(buf, chart_area, primary.1),
            ChartMode::Line => render_mirrored_net_line_chart(buf, chart_area, primary.1),
        }
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

fn choose_cpu_body_rows(core_count: usize, area_width: u16, max_body_rows: usize) -> usize {
    let mut body_rows = 4usize;
    let max_body_rows = max_body_rows.max(body_rows);
    while body_rows < max_body_rows {
        let (_, cols, col_w) = cpu_core_layout(core_count, area_width, body_rows);
        if cols as u16 * col_w <= area_width / 2 {
            break;
        }
        body_rows += 1;
    }
    body_rows
}

fn cpu_core_layout(core_count: usize, area_width: u16, body_rows: usize) -> (usize, usize, u16) {
    let core_rows = body_rows.saturating_sub(1).max(1);
    let cols = core_count.div_ceil(core_rows).max(1);
    let max_core_width = (area_width / 2).max(16);
    let col_w = (max_core_width / cols as u16).clamp(14, 18);
    (core_rows, cols, col_w)
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width { text.to_string() } else { format!("{}…", text.chars().take(width.saturating_sub(1)).collect::<String>()) }
}

fn cpu_chart_color(value: f32) -> ratatui::style::Color {
    ratatui_glamour::color::lerp_color(palette::SUCCESS_BASE, palette::ERROR, value.clamp(0.0, 100.0) / 100.0)
}

fn cpu_flame_color(value: f32) -> ratatui::style::Color {
    let idx = ((value.clamp(0.0, 1.0) * (CPU_FLAME_PALETTE.len().saturating_sub(1)) as f32).round() as usize)
        .min(CPU_FLAME_PALETTE.len().saturating_sub(1));
    CPU_FLAME_PALETTE[idx]
}

fn compact_core_meter(pct: f32, width: usize) -> String {
    let filled = (((pct.clamp(0.0, 100.0) / 100.0) * width as f32).round() as usize).min(width);
    let mut out = String::with_capacity(width);
    for idx in 0..width {
        out.push(if idx < filled { '█' } else { '░' });
    }
    out
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
    let rx_samples = sample_history_u64(&history.rx_history, plot.width as usize);
    let tx_samples = sample_history_u64(&history.tx_history, plot.width as usize);
    let max_rate = rx_samples.iter().chain(tx_samples.iter()).copied().max().unwrap_or(0).max(1);

    buf.set_string(area.x, plot.y, "Dn", Style::default().fg(palette::SUCCESS));
    buf.set_string(area.x, plot.bottom().saturating_sub(1), "Up", Style::default().fg(palette::PRIMARY));

    for (idx, sample) in rx_samples.iter().enumerate() {
        render_vertical_fill_up(
            buf,
            plot.x + idx as u16,
            split_y.saturating_sub(1),
            top_rows,
            scale_rows(*sample, top_rows, max_rate),
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
            scale_rows(*sample, bottom_rows, max_rate),
            palette::PROCESSING_BASE,
            palette::PROCESSING_PEAK,
        );
    }
}

fn render_mirrored_net_line_chart(buf: &mut Buffer, area: Rect, history: &NetworkHistory) {
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
    let rx_plot = Rect { x: plot.x, y: plot.y, width: plot.width, height: top_rows };
    let tx_plot = Rect { x: plot.x, y: plot.y + top_rows, width: plot.width, height: bottom_rows };
    let rx_samples = sample_history_u64(&history.rx_history, plot.width as usize * 2);
    let tx_samples = sample_history_u64(&history.tx_history, plot.width as usize * 2);
    let max_rate = rx_samples.iter().chain(tx_samples.iter()).copied().max().unwrap_or(0).max(1);

    buf.set_string(area.x, plot.y, "Dn", Style::default().fg(palette::SUCCESS));
    buf.set_string(area.x, plot.bottom().saturating_sub(1), "Up", Style::default().fg(palette::PRIMARY));

    render_braille_u64_line_overlay(buf, rx_plot, &rx_samples, max_rate, palette::SUCCESS_PEAK, false);
    render_braille_u64_line_overlay(buf, tx_plot, &tx_samples, max_rate, palette::PROCESSING_PEAK, true);
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

fn sample_history_f32(history: &[f32], width: usize) -> Vec<f32> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = vec![0.0; width];
    let start = history.len().saturating_sub(width);
    let slice = &history[start..];
    let offset = width.saturating_sub(slice.len());
    for (idx, value) in slice.iter().enumerate() {
        out[offset + idx] = *value;
    }
    out
}

fn proportional_rows(values: &[f32], total_rows: u16) -> Vec<u16> {
    if values.is_empty() || total_rows == 0 {
        return Vec::new();
    }
    let sum = values.iter().sum::<f32>();
    if sum <= f32::EPSILON {
        return vec![0; values.len()];
    }
    let mut rows = vec![0u16; values.len()];
    let mut fractions = Vec::with_capacity(values.len());
    let mut assigned = 0u16;
    for (idx, value) in values.iter().enumerate() {
        let exact = (*value / sum) * total_rows as f32;
        rows[idx] = exact.floor() as u16;
        assigned = assigned.saturating_add(rows[idx]);
        fractions.push((idx, exact - rows[idx] as f32));
    }
    fractions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    for (idx, _) in fractions.into_iter().take(total_rows.saturating_sub(assigned) as usize) {
        rows[idx] = rows[idx].saturating_add(1);
    }
    rows
}

fn render_braille_line_overlay(buf: &mut Buffer, plot: Rect, history: &[f32], color: Color) {
    render_braille_line_overlay_oriented(buf, plot, history, color, false);
}

fn render_braille_u64_line_overlay(buf: &mut Buffer, plot: Rect, history: &[u64], max_value: u64, color: Color, from_top: bool) {
    if max_value == 0 {
        return;
    }
    let scaled = history.iter().map(|value| ((*value as f64 / max_value as f64) * 100.0) as f32).collect::<Vec<_>>();
    render_braille_line_overlay_oriented(buf, plot, &scaled, color, from_top);
}

fn render_braille_line_overlay_oriented(buf: &mut Buffer, plot: Rect, history: &[f32], color: Color, from_top: bool) {
    if plot.width == 0 || plot.height == 0 {
        return;
    }
    let samples = sample_history_f32(history, plot.width as usize * 2);
    if samples.is_empty() {
        return;
    }
    let total_subrows = plot.height as usize * 4;
    let mut cells = BTreeMap::<(u16, u16), u8>::new();
    let mut prev = None;
    for (x_sub, value) in samples.iter().enumerate() {
        let y_sub = if total_subrows <= 1 {
            0usize
        } else {
            (((value.clamp(0.0, 100.0) / 100.0) * (total_subrows - 1) as f32).round() as usize).min(total_subrows - 1)
        };
        if let Some((px, py)) = prev {
            let steps = (x_sub.saturating_sub(px)).max(y_sub.abs_diff(py)).max(1);
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let xs = px as f32 + (x_sub as f32 - px as f32) * t;
                let ys = py as f32 + (y_sub as f32 - py as f32) * t;
                set_braille_subpoint(&mut cells, plot, xs.round() as usize, ys.round() as usize, from_top);
            }
        } else {
            set_braille_subpoint(&mut cells, plot, x_sub, y_sub, from_top);
        }
        prev = Some((x_sub, y_sub));
    }
    for ((x, y), dots) in cells {
        buf.set_string(x, y, char::from_u32(0x2800 + dots as u32).unwrap_or(' ').to_string(), Style::default().fg(color));
    }
}

fn set_braille_subpoint(cells: &mut BTreeMap<(u16, u16), u8>, plot: Rect, x_sub: usize, y_sub: usize, from_top: bool) {
    if plot.width == 0 || plot.height == 0 {
        return;
    }
    let cell_x = plot.x + (x_sub / 2).min(plot.width.saturating_sub(1) as usize) as u16;
    let cell_y = if from_top {
        plot.y + (y_sub / 4).min(plot.height.saturating_sub(1) as usize) as u16
    } else {
        plot.bottom().saturating_sub(1 + (y_sub / 4).min(plot.height.saturating_sub(1) as usize) as u16)
    };
    let mask = match (x_sub % 2, y_sub % 4) {
        (0, 0) => 1 << 6,
        (0, 1) => 1 << 2,
        (0, 2) => 1 << 1,
        (0, 3) => 1 << 0,
        (1, 0) => 1 << 7,
        (1, 1) => 1 << 5,
        (1, 2) => 1 << 4,
        (1, 3) => 1 << 3,
        _ => 0,
    };
    cells.entry((cell_x, cell_y)).and_modify(|dots| *dots |= mask).or_insert(mask);
}

fn scale_rows(value: u64, rows: u16, max_value: u64) -> u16 {
    if rows == 0 || max_value == 0 {
        return 0;
    }
    let filled = (((value as f64 / max_value as f64) * rows as f64).round() as u16).min(rows);
    if value > 0 && filled == 0 { 1 } else { filled }
}

fn render_vertical_fill_up(
    buf: &mut Buffer, x: u16, baseline_y: u16, rows: u16, filled_rows: u16, cold: ratatui::style::Color, hot: ratatui::style::Color,
) {
    for row in 0..filled_rows.min(rows) {
        let y = baseline_y.saturating_sub(row);
        let t = if rows > 1 { row as f32 / (rows - 1) as f32 } else { 1.0 };
        buf.set_string(x, y, "█", Style::default().fg(ratatui_glamour::color::lerp_color(hot, cold, t)));
    }
}

fn render_vertical_fill_down(
    buf: &mut Buffer, x: u16, start_y: u16, rows: u16, filled_rows: u16, cold: ratatui::style::Color, hot: ratatui::style::Color,
) {
    for row in 0..filled_rows.min(rows) {
        let y = start_y.saturating_add(row);
        let t = if rows > 1 { row as f32 / (rows - 1) as f32 } else { 1.0 };
        buf.set_string(x, y, "█", Style::default().fg(ratatui_glamour::color::lerp_color(hot, cold, t)));
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
