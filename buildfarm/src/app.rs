use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};

use crate::{
    runner::{BuildJob, BuildPlan},
    ui::BuildScreen,
};

pub struct BuildfarmApp {
    plan: BuildPlan,
    states: Vec<JobState>,
}

impl BuildfarmApp {
    pub fn new(plan: BuildPlan) -> Self {
        Self {
            states: plan.jobs().iter().map(JobState::from_job).collect(),
            plan,
        }
    }

    pub fn run(&mut self) -> Result<i32, String> {
        TerminalGuard::enter().and_then(|mut terminal| {
            JobSupervisor::new(&self.plan).spawn().and_then(|events| {
                InputReader::spawn().and_then(|keys| {
                    AppLoop::new(&mut self.states, events, keys, terminal.terminal_mut()).run()
                })
            })
        })
    }
}

struct AppLoop<'a> {
    states: &'a mut [JobState],
    events: Receiver<JobEvent>,
    keys: Receiver<KeyPress>,
    terminal: &'a mut Terminal<CrosstermBackend<std::io::Stdout>>,
    active_pane: usize,
    scrollbacks: Vec<usize>,
    popup_open: bool,
    popup_dismissed: bool,
}

impl<'a> AppLoop<'a> {
    fn new(
        states: &'a mut [JobState],
        events: Receiver<JobEvent>,
        keys: Receiver<KeyPress>,
        terminal: &'a mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Self {
        let pane_count = states.len();

        Self {
            states,
            events,
            keys,
            terminal,
            active_pane: 0,
            scrollbacks: vec![0; pane_count],
            popup_open: false,
            popup_dismissed: false,
        }
    }

    fn run(&mut self) -> Result<i32, String> {
        loop {
            self.drain_events();
            self.refresh_logs();
            self.refresh_popup();
            self.render()?;

            if let Some(key) = self.key_pressed() {
                if self.handle_key(key) {
                    return Ok(self.exit_code_or_abort(key));
                }
            }
            thread::sleep(Duration::from_millis(16));
        }
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.states[event.index].apply(event);
        }
    }

    fn refresh_logs(&mut self) {
        self.states.iter_mut().for_each(JobState::refresh_log);
    }

    fn render(&mut self) -> Result<(), String> {
        self.terminal
            .draw(|frame| {
                BuildScreen::from_states(
                    self.states,
                    self.active_pane,
                    &self.scrollbacks,
                    self.popup_open,
                )
                .render(frame)
            })
            .map_err(|err| format!("buildfarm: failed to render TUI: {err}"))
            .map(|_| ())
    }

    fn refresh_popup(&mut self) {
        if self.all_finished() && !self.popup_dismissed {
            self.popup_open = true;
        }
    }

    fn all_finished(&self) -> bool {
        self.states.iter().all(JobState::is_finished)
    }

    fn exit_code(&self) -> i32 {
        self.states
            .iter()
            .find(|state| !state.is_success())
            .map(|state| state.status_code())
            .unwrap_or(0)
    }

    fn exit_code_or_abort(&self, key: KeyPress) -> i32 {
        (!self.all_finished() && key.should_quit())
            .then_some(130)
            .unwrap_or_else(|| self.exit_code())
    }

    fn key_pressed(&self) -> Option<KeyPress> {
        self.keys.try_recv().ok()
    }

    fn handle_key(&mut self, key: KeyPress) -> bool {
        if !self.all_finished() {
            return self.handle_live_key(key);
        }
        if self.popup_open {
            if key.should_close_finished() {
                return true;
            }
            self.popup_open = false;
            self.popup_dismissed = true;
            return false;
        }
        self.handle_live_key(key)
    }

    fn handle_live_key(&mut self, key: KeyPress) -> bool {
        if key.should_quit() {
            return true;
        }
        key
            .navigation()
            .into_iter()
            .for_each(|direction| self.move_active_pane(direction));
        key
            .scroll()
            .into_iter()
            .for_each(|scroll| self.scroll_active_pane(scroll));
        false
    }

    fn move_active_pane(&mut self, direction: PaneDirection) {
        if self.states.is_empty() {
            return;
        }
        self.active_pane = direction.next_index(self.active_pane, self.states.len());
    }

    fn scroll_active_pane(&mut self, scroll: PaneScroll) {
        let page_height = self.active_viewport_height();

        self.scrollbacks
            .get_mut(self.active_pane)
            .into_iter()
            .for_each(|scrollback| *scrollback = scroll.next_offset(*scrollback, page_height));
    }

    fn active_viewport_height(&self) -> usize {
        self.terminal
            .size()
            .ok()
            .map(|size| {
                BuildScreen::viewport_height(
                    self.states.len(),
                    self.active_pane,
                    Rect::new(0, 0, size.width, size.height),
                )
            })
            .unwrap_or(10)
    }
}

#[derive(Clone, Copy)]
struct KeyPress {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyPress {
    fn from_event(evt: Event) -> Option<Self> {
        match evt {
            Event::Key(key) => Some(Self::from_key(key)),
            _ => None,
        }
    }

    fn from_key(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            modifiers: key.modifiers,
        }
    }

    fn should_quit(&self) -> bool {
        self.is_escape()
            || self.is_quit_char()
            || self.is_ctrl_c()
    }

    fn should_close_finished(&self) -> bool {
        self.should_quit() || self.is_enter()
    }

    fn navigation(&self) -> Option<PaneDirection> {
        match self.code {
            KeyCode::BackTab => Some(PaneDirection::Prev),
            KeyCode::Tab => Some(PaneDirection::Next),
            _ => None,
        }
    }

    fn scroll(&self) -> Option<PaneScroll> {
        match self.code {
            KeyCode::Up => Some(PaneScroll::LineUp),
            KeyCode::Down => Some(PaneScroll::LineDown),
            KeyCode::PageUp if self.modifiers.contains(KeyModifiers::SHIFT) => Some(PaneScroll::Top),
            KeyCode::PageDown if self.modifiers.contains(KeyModifiers::SHIFT) => Some(PaneScroll::Bottom),
            KeyCode::Home => Some(PaneScroll::Top),
            KeyCode::End => Some(PaneScroll::Bottom),
            KeyCode::PageUp => Some(PaneScroll::PageUp),
            KeyCode::PageDown => Some(PaneScroll::PageDown),
            _ => None,
        }
    }

    fn is_ctrl_c(&self) -> bool {
        self.code == KeyCode::Char('c') && self.modifiers.contains(KeyModifiers::CONTROL)
    }

    fn is_enter(&self) -> bool {
        matches!(
            self.code,
            KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r')
        )
    }

    fn is_escape(&self) -> bool {
        self.code == KeyCode::Esc
    }

    fn is_quit_char(&self) -> bool {
        match self.code {
            KeyCode::Char(ch) => ch.eq_ignore_ascii_case(&'q'),
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
enum PaneDirection {
    Prev,
    Next,
}

impl PaneDirection {
    fn next_index(&self, active: usize, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        match self {
            Self::Prev => active.checked_sub(1).unwrap_or(count - 1),
            Self::Next => (active + 1) % count,
        }
    }
}

#[derive(Clone, Copy)]
enum PaneScroll {
    LineUp,
    LineDown,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

impl PaneScroll {
    fn next_offset(&self, current: usize, page_height: usize) -> usize {
        match self {
            Self::LineUp => current.saturating_add(1),
            Self::LineDown => current.saturating_sub(1),
            Self::PageUp => current.saturating_add(page_height.max(1)),
            Self::PageDown => current.saturating_sub(page_height.max(1)),
            Self::Top => usize::MAX,
            Self::Bottom => 0,
        }
    }
}

struct JobSupervisor<'a> {
    plan: &'a BuildPlan,
}

struct InputReader;

impl InputReader {
    fn spawn() -> Result<Receiver<KeyPress>, String> {
        let (tx, rx) = mpsc::channel();

        thread::Builder::new()
            .name("buildfarm-input".to_string())
            .spawn(move || Self::run(tx))
            .map_err(|err| format!("buildfarm: failed to start input thread: {err}"))?;

        Ok(rx)
    }

    fn run(tx: Sender<KeyPress>) {
        loop {
            let event = event::read();

            if let Ok(evt) = event {
                if let Some(key) = KeyPress::from_event(evt) {
                    if tx.send(key).is_err() {
                        return;
                    }
                }
            }
        }
    }
}

impl<'a> JobSupervisor<'a> {
    fn new(plan: &'a BuildPlan) -> Self {
        Self { plan }
    }

    fn spawn(&self) -> Result<Receiver<JobEvent>, String> {
        let (tx, rx) = mpsc::channel();

        self.plan
            .jobs()
            .iter()
            .cloned()
            .enumerate()
            .for_each(|(index, job)| JobWorker::new(index, job, tx.clone()).spawn());

        Ok(rx)
    }
}

struct JobWorker {
    index: usize,
    job: BuildJob,
    tx: Sender<JobEvent>,
}

impl JobWorker {
    fn new(index: usize, job: BuildJob, tx: Sender<JobEvent>) -> Self {
        Self { index, job, tx }
    }

    fn spawn(self) {
        thread::spawn(move || {
            let _ = self.tx.send(JobEvent::started(self.index));
            let _ = self.tx.send(
                self.job
                    .run()
                    .map(|result| JobEvent::finished(self.index, result.status()))
                    .unwrap_or_else(|err| JobEvent::failed(self.index, err)),
            );
        });
    }
}

#[derive(Clone)]
pub struct JobState {
    title: String,
    log_path: PathBuf,
    log_text: String,
    log_offset: u64,
    stage: JobStage,
    status_code: i32,
}

impl JobState {
    pub(crate) fn from_job(job: &BuildJob) -> Self {
        Self {
            title: format!(
                "{} {} {}",
                job.target().os(),
                job.target().arch(),
                job.target().destination()
            ),
            log_path: job.log_path().to_path_buf(),
            log_text: String::new(),
            log_offset: 0,
            stage: JobStage::Pending,
            status_code: 0,
        }
    }

    fn apply(&mut self, event: JobEvent) {
        self.stage = event.stage;
        self.status_code = event.status_code;
        if let Some(message) = event.error {
            self.log_text.push_str(&format!("\n{message}\n"));
        }
    }

    fn refresh_log(&mut self) {
        self.log_path
            .exists()
            .then_some(self.read_new_log_bytes().ok())
            .flatten()
            .filter(|bytes| !bytes.is_empty())
            .into_iter()
            .for_each(|bytes| self.log_text.push_str(&String::from_utf8_lossy(&bytes)));
    }

    fn read_new_log_bytes(&mut self) -> Result<Vec<u8>, String> {
        File::open(&self.log_path)
            .map_err(|err| format!("buildfarm: failed to open log file: {err}"))
            .and_then(|mut file| {
                file.seek(SeekFrom::Start(self.log_offset))
                    .map_err(|err| format!("buildfarm: failed to seek log file: {err}"))?;
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)
                    .map_err(|err| format!("buildfarm: failed to read log file: {err}"))?;
                self.log_offset = self
                    .log_offset
                    .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
                Ok(bytes)
            })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn log_text(&self) -> &str {
        &self.log_text
    }

    pub fn summary(&self) -> &str {
        self.stage.label()
    }

    pub fn stage(&self) -> JobStage {
        self.stage
    }

    pub fn is_finished(&self) -> bool {
        self.stage.is_finished()
    }

    pub fn is_success(&self) -> bool {
        self.stage.is_success()
    }

    pub fn status_code(&self) -> i32 {
        self.status_code
    }
}

#[derive(Clone, Copy)]
pub enum JobStage {
    Pending,
    Running,
    Success,
    Failed,
}

impl JobStage {
    pub(crate) fn label(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "finished",
            Self::Failed => "failed",
        }
    }

    fn is_finished(&self) -> bool {
        matches!(self, Self::Success | Self::Failed)
    }

    fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

struct JobEvent {
    index: usize,
    stage: JobStage,
    status_code: i32,
    error: Option<String>,
}

impl JobEvent {
    fn started(index: usize) -> Self {
        Self {
            index,
            stage: JobStage::Running,
            status_code: 0,
            error: None,
        }
    }

    fn finished(index: usize, status_code: i32) -> Self {
        Self {
            index,
            stage: (status_code == 0)
                .then_some(JobStage::Success)
                .unwrap_or(JobStage::Failed),
            status_code,
            error: None,
        }
    }

    fn failed(index: usize, error: String) -> Self {
        Self {
            index,
            stage: JobStage::Failed,
            status_code: 1,
            error: Some(error),
        }
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl TerminalGuard {
    fn enter() -> Result<Self, String> {
        enable_raw_mode().map_err(|err| format!("buildfarm: failed to enable raw mode: {err}"))?;
        execute!(std::io::stdout(), EnterAlternateScreen)
            .map_err(|err| format!("buildfarm: failed to enter alternate screen: {err}"))?;
        Terminal::new(CrosstermBackend::new(std::io::stdout()))
            .map_err(|err| format!("buildfarm: failed to create terminal: {err}"))
            .map(|terminal| Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<std::io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}
