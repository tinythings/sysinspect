use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier},
};

use crate::{
    app::JobStage,
    model::{BuildfarmConfig, ResultMirrorPlan},
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

    assert_eq!(layout.status().height, 1);
    assert_eq!(layout.viewport().height, 19);
    assert_eq!(layout.viewport().y, 0);
    assert_eq!(layout.status().y, 19);
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
fn tile_status_renders_black_bar_with_yellow_text_when_building() {
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");

    terminal
        .draw(|frame| {
            TileStatus::from_job(&Fixture::new().plan().jobs()[1]).render(frame, Rect::new(0, 0, 80, 1));
        })
        .expect("status should render");

    assert_eq!(terminal.backend().buffer().cell((1, 0)).expect("cell should exist").fg, Color::Yellow);
    assert_eq!(terminal.backend().buffer().cell((1, 0)).expect("cell should exist").bg, Color::Black);
}

#[test]
fn tile_status_colors_match_stage() {
    assert_eq!(
        TileStatus::from_fixture("pending", JobStage::Pending).style().bg,
        Some(Color::Black)
    );
    assert_eq!(
        TileStatus::from_fixture("building", JobStage::Building).style().fg,
        Some(Color::Yellow)
    );
    assert_eq!(
        TileStatus::from_fixture("finished", JobStage::Success).style().bg,
        Some(Color::Green)
    );
    assert_eq!(
        TileStatus::from_fixture("finished", JobStage::Success).style().fg,
        Some(Color::White)
    );
    assert_eq!(
        TileStatus::from_fixture("failed", JobStage::Failed).style().fg,
        Some(Color::White)
    );
}

#[test]
fn tile_viewport_renders_ansi_colors() {
    let backend = TestBackend::new(40, 6);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");

    terminal
        .draw(|frame| {
            TileViewport::from_ansi("\u{1b}[1;92mOK\u{1b}[0m", 0)
                .render(frame, Rect::new(0, 0, 20, 4), false);
        })
        .expect("viewport should render");

    assert!(terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|cell| cell.symbol() == "O" && cell.fg == Color::LightGreen && cell.modifier.contains(Modifier::BOLD)));
}

#[test]
fn tile_viewport_scrolls_to_bottom_like_terminal_tail() {
    let viewport = TileViewport::from_ansi("1\n2\n3\n4\n5\n6", 0);

    assert_eq!(viewport.scroll_y(6, Rect::new(0, 0, 20, 3)), 3);
}

#[test]
fn build_screen_renders_finish_popup_when_requested() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let states = plan.jobs().iter().map(crate::app::JobState::from_job).collect::<Vec<_>>();
    let screen = BuildScreen::from_states(&states, 0, &[0, 0], true);
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");

    terminal
        .draw(|frame| screen.render(frame))
        .expect("screen should render popup");

    assert!(terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|cell| cell.symbol() == "q" && cell.bg == Color::Cyan && cell.fg == Color::White));
}

#[test]
fn tile_viewport_marks_active_frame_bright_green() {
    let backend = TestBackend::new(20, 4);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");

    terminal
        .draw(|frame| {
            TileViewport::from_ansi("line", 0).render(frame, Rect::new(0, 0, 20, 3), true);
        })
        .expect("viewport should render");

    assert_eq!(
        terminal
            .backend()
            .buffer()
            .cell((0, 0))
            .expect("cell should exist")
            .fg,
        Color::LightGreen
    );
}

#[test]
fn tile_viewport_page_up_moves_above_tail() {
    let viewport = TileViewport::from_ansi("1\n2\n3\n4\n5\n6", 10);

    assert_eq!(viewport.scroll_y(6, Rect::new(0, 0, 20, 3)), 0);
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
            ResultMirrorPlan::disabled(std::path::PathBuf::from("/tmp/buildfarm"), "dev"),
        )
    }
}
