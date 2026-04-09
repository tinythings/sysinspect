use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier},
};

use crate::{
    model::BuildfarmConfig,
    runner::BuildPlan,
    ui::{BuildScreen, GridShape, TileLayout, TileStatus, TileViewport},
};

#[test]
fn grid_shape_tiles_one_target() {
    let shape = GridShape::from_count(1);

    assert_eq!(shape.rows(Rect::new(0, 0, 80, 24)).len(), 1);
    assert_eq!(shape.cols(Rect::new(0, 0, 80, 24)).len(), 1);
}

#[test]
fn grid_shape_tiles_four_targets_into_two_by_two() {
    let shape = GridShape::from_count(4);

    assert_eq!(shape.rows(Rect::new(0, 0, 120, 40)).len(), 2);
    assert_eq!(shape.cols(Rect::new(0, 0, 120, 40)).len(), 2);
}

#[test]
fn tile_layout_reserves_status_bar_below_viewport() {
    let layout = TileLayout::new(Rect::new(0, 0, 80, 20)).split();

    assert_eq!(layout.status().height, 4);
    assert_eq!(layout.viewport().height, 16);
    assert_eq!(layout.viewport().y, 0);
    assert_eq!(layout.status().y, 16);
}

#[test]
fn build_screen_creates_one_tile_per_job() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let screen = BuildScreen::from_plan(&plan);

    assert_eq!(screen.tiles().len(), 2);
    assert!(screen.tiles()[0].status().title().contains("local"));
    assert!(screen.tiles()[1].status().title().contains("192.168.122.122:work/sysinspect-buildfarm"));
}

#[test]
fn build_screen_renders_tiled_terminal_viewports() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let screen = BuildScreen::from_plan(&plan);
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");

    terminal
        .draw(|frame| screen.render(frame))
        .expect("screen should render");
}

#[test]
fn tile_status_renders_cyan_bar_with_yellow_text() {
    let backend = TestBackend::new(80, 6);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");

    terminal
        .draw(|frame| {
            TileStatus::from_job(&Fixture::new().plan().jobs()[1]).render(frame, Rect::new(0, 0, 80, 4));
        })
        .expect("status should render");

    assert_eq!(terminal.backend().buffer().cell((0, 0)).expect("cell should exist").fg, Color::Cyan);
    assert_eq!(terminal.backend().buffer().cell((1, 1)).expect("cell should exist").fg, Color::Yellow);
}

#[test]
fn tile_viewport_renders_ansi_colors() {
    let backend = TestBackend::new(40, 6);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");

    terminal
        .draw(|frame| {
            TileViewport::from_ansi("\u{1b}[1;92mOK\u{1b}[0m").render(frame, Rect::new(0, 0, 20, 4));
        })
        .expect("viewport should render");

    assert_eq!(terminal.backend().buffer().cell((1, 1)).expect("cell should exist").fg, Color::LightGreen);
    assert!(terminal.backend().buffer().cell((1, 1)).expect("cell should exist").modifier.contains(Modifier::BOLD));
}

struct Fixture {
    config: BuildfarmConfig,
}

impl Fixture {
    fn new() -> Self {
        Self {
            config: BuildfarmConfig::parse("local\nFreeBSD amd64 192.168.122.122:work/sysinspect-buildfarm\n")
                .expect("config should parse"),
        }
    }

    fn plan(&self) -> BuildPlan {
        BuildPlan::new(
            &self.config,
            "dev",
            std::path::Path::new("/tmp/sysinspect"),
            std::path::Path::new("/tmp/logs"),
            "make",
        )
    }
}
