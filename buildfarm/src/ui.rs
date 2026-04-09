use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::ansi::AnsiDocument;
use crate::runner::BuildPlan;

pub struct BuildScreen<'a> {
    tiles: Vec<BuildTile<'a>>,
}

impl<'a> BuildScreen<'a> {
    pub fn from_plan(plan: &'a BuildPlan) -> Self {
        Self {
            tiles: plan.jobs().iter().map(BuildTile::from_job).collect(),
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        TileGrid::new(self.tiles.len())
            .split(frame.area())
            .iter()
            .zip(self.tiles.iter())
            .for_each(|(area, tile)| tile.render(frame, *area));
    }

    pub fn tiles(&self) -> &[BuildTile<'a>] {
        &self.tiles
    }
}

pub struct BuildTile<'a> {
    status: TileStatus<'a>,
    viewport: TileViewport<'a>,
}

impl<'a> BuildTile<'a> {
    pub fn from_job(job: &'a crate::runner::BuildJob) -> Self {
        Self {
            status: TileStatus::from_job(job),
            viewport: TileViewport::empty(),
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let layout = TileLayout::new(area).split();
        self.viewport.render(frame, layout.viewport());
        self.status.render(frame, layout.status());
    }

    pub fn status(&self) -> &TileStatus<'a> {
        &self.status
    }
}

pub struct TileStatus<'a> {
    os: &'a str,
    arch: &'a str,
    destination: &'a str,
    summary: &'a str,
}

impl<'a> TileStatus<'a> {
    pub fn from_job(job: &'a crate::runner::BuildJob) -> Self {
        Self {
            os: job.target().os(),
            arch: job.target().arch(),
            destination: job.target().destination(),
            summary: "pending",
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    format!("{} {} {}", self.os, self.arch, self.destination),
                    Style::default().fg(Color::Yellow),
                ),
                Line::styled(self.summary, Style::default().fg(Color::Yellow)),
            ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("status")
                        .border_style(Style::default().fg(Color::Cyan))
                        .title_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub fn title(&self) -> &str {
        self.destination
    }
}

pub struct TileViewport<'a> {
    source: &'a str,
}

impl<'a> TileViewport<'a> {
    pub fn empty() -> Self {
        Self { source: "" }
    }

    pub fn from_ansi(source: &'a str) -> Self {
        Self { source }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(
            Paragraph::new(AnsiDocument::parse(self.source).lines())
                .block(Block::default().borders(Borders::ALL).title("viewport"))
                .wrap(Wrap { trim: false }),
            area,
        );
    }
}

pub struct TileLayout {
    area: Rect,
}

impl TileLayout {
    pub fn new(area: Rect) -> Self {
        Self { area }
    }

    pub fn split(&self) -> SplitTileLayout {
        Layout::vertical([Constraint::Min(1), Constraint::Length(4)])
            .split(self.area)
            .to_vec()
            .pipe_ref(|chunks| SplitTileLayout::new(chunks[0], chunks[1]))
    }
}

pub struct SplitTileLayout {
    viewport: Rect,
    status: Rect,
}

impl SplitTileLayout {
    pub fn new(viewport: Rect, status: Rect) -> Self {
        Self { viewport, status }
    }

    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    pub fn status(&self) -> Rect {
        self.status
    }
}

pub struct TileGrid {
    count: usize,
}

impl TileGrid {
    pub fn new(count: usize) -> Self {
        Self { count }
    }

    pub fn split(&self, area: Rect) -> Vec<Rect> {
        GridShape::from_count(self.count)
            .rows(area)
            .iter()
            .flat_map(|row| GridShape::from_count(self.count).cols(*row))
            .take(self.count)
            .collect()
    }
}

pub struct GridShape {
    rows: usize,
    cols: usize,
}

impl GridShape {
    pub fn from_count(count: usize) -> Self {
        ((count.max(1) as f64).sqrt().ceil() as usize).pipe_ref(|cols| Self {
            cols: *cols,
            rows: count.max(1).div_ceil(*cols),
        })
    }

    pub fn rows(&self, area: Rect) -> Vec<Rect> {
        Layout::vertical((0..self.rows).map(|_| Constraint::Ratio(1, self.rows as u32)).collect::<Vec<_>>()).split(area).to_vec()
    }

    pub fn cols(&self, area: Rect) -> Vec<Rect> {
        Layout::horizontal((0..self.cols).map(|_| Constraint::Ratio(1, self.cols as u32)).collect::<Vec<_>>()).split(area).to_vec()
    }
}

trait PipeRef: Sized {
    fn pipe_ref<T>(self, f: impl FnOnce(&Self) -> T) -> T {
        f(&self)
    }
}

impl<T> PipeRef for T {}
