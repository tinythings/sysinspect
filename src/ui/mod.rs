use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::Rng;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarState, StatefulWidget, Table,
        Widget,
    },
};
use std::io;

pub fn run() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let r = App::default().run(&mut terminal);
    ratatui::restore();
    r
}

#[derive(Debug, Clone)]
pub struct Cycle {
    pub id: u32,
}

impl Cycle {
    pub fn get_title(&self) -> String {
        format!("cycle {}", self.id)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ActiveBox {
    Cycles,
    Minions,
    Events,
}

#[derive(Debug)]
pub struct App {
    exit: bool,
    pub selected_cycle: usize,
    pub selected_minion: usize,
    pub selected_event: usize,

    pub minions: Vec<String>,
    pub events: Vec<String>,
    pub active_box: ActiveBox,

    pub status_text: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            exit: false,
            selected_cycle: 0,
            selected_minion: 0,
            selected_event: 0,
            minions: Vec::new(),
            events: Vec::new(),
            active_box: ActiveBox::Cycles,

            status_text: "Init".to_string(),
        }
    }
}

impl App {
    pub fn run(&mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            term.draw(|frame| self.draw(frame))?;
            self.on_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        //frame.render_widget(self, frame.area());

        // Split the entire area into main UI and a one-line status bar.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
            .split(frame.area());
        let main_area = chunks[0];
        let status_area = chunks[1];

        // Render the main widget (your left/middle/right boxes) in main_area.
        frame.render_widget(self, main_area);

        // NEW: Create a status bar with custom text, color, and attributes.
        let status_paragraph = Paragraph::new(self.status_text.as_str())
            .style(Style::default().fg(Color::Yellow).bg(Color::Blue).add_modifier(Modifier::BOLD));
        frame.render_widget(status_paragraph, status_area);
    }

    fn on_events(&mut self) -> io::Result<()> {
        if let Event::Key(e) = event::read()? {
            if e.kind == KeyEventKind::Press {
                self.on_key(e);
            }
        }
        Ok(())
    }

    fn shift_next(&mut self) {
        match self.active_box {
            ActiveBox::Cycles => self.active_box = ActiveBox::Minions,
            ActiveBox::Minions => self.active_box = ActiveBox::Events,
            ActiveBox::Events => self.active_box = ActiveBox::Cycles,
        };
    }

    fn on_key(&mut self, e: event::KeyEvent) {
        match e.code {
            KeyCode::Up => {
                match self.active_box {
                    ActiveBox::Cycles => {
                        if self.selected_cycle > 0 {
                            self.selected_cycle -= 1;
                        }
                    }
                    ActiveBox::Minions => {
                        if self.selected_minion > 0 {
                            self.selected_minion -= 1;
                        }
                    }
                    ActiveBox::Events => {
                        if self.selected_event > 0 {
                            self.selected_event -= 1;
                        }
                    }
                };
            }
            KeyCode::Down => {
                match self.active_box {
                    ActiveBox::Cycles => {
                        let cycles = self.get_cycles();
                        if self.selected_cycle < cycles.len().saturating_sub(1) {
                            self.selected_cycle += 1;
                        }
                    }
                    ActiveBox::Minions => {
                        if self.selected_minion < self.minions.len().saturating_sub(1) {
                            self.selected_minion += 1;
                        }
                    }
                    ActiveBox::Events => {
                        if self.selected_event < self.events.len().saturating_sub(1) {
                            self.selected_event += 1;
                        }
                    }
                };
            }
            KeyCode::Right => {
                self.shift_next();
            }
            KeyCode::Left => {
                match self.active_box {
                    ActiveBox::Cycles => self.active_box = ActiveBox::Events,
                    ActiveBox::Minions => self.active_box = ActiveBox::Cycles,
                    ActiveBox::Events => self.active_box = ActiveBox::Minions,
                };
            }
            KeyCode::Enter => {
                if self.active_box == ActiveBox::Cycles {
                    let cycles = self.get_cycles();
                    if !cycles.is_empty() {
                        self.minions = self.get_minions();
                        self.selected_minion = 0;
                    }
                    self.shift_next();
                } else if self.active_box == ActiveBox::Minions {
                    if !self.minions.is_empty() {
                        self.events = self.get_events();
                        self.selected_event = 0;
                    }
                    self.shift_next();
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => self.exit(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
    /// Returns a vector of cycle names.
    pub fn get_cycles(&self) -> Vec<Cycle> {
        (0..100).map(|id| Cycle { id }).collect()
    }

    /// Returns a vector of minion names (random IDs).
    pub fn get_minions(&self) -> Vec<String> {
        (0..100).map(|x| format!("minion-{}-{}", x, rand::rng().random_range(0..100))).collect()
    }

    /// Returns a vector of events (random IDs)
    pub fn get_events(&self) -> Vec<String> {
        (0..100).map(|x| format!("event-{}-{}", x, rand::rng().random_range(0..100))).collect()
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [cycles_a, minions_a, events_a]: [Rect; 3] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Length(30), Constraint::Min(0)])
            .split(area)
            .as_ref()
            .try_into()
            .unwrap();

        //----------------------------------
        // Left column: Cycles
        //----------------------------------
        let cycles_block = if self.active_box == ActiveBox::Cycles {
            Block::default()
                .borders(Borders::ALL)
                .title("Cycles")
                .title_style(Style::default().fg(Color::LightYellow))
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::White))
        } else {
            Block::default()
                .borders(Borders::ALL)
                .title("Cycles")
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
        };
        Widget::render(&cycles_block, cycles_a, buf);

        let cycles_inner = cycles_block.inner(cycles_a);
        let cycles = self.get_cycles();
        let items: Vec<ListItem> = cycles
            .iter()
            .map(|cycle| {
                let line = Line::from(vec![
                    Span::styled(">", Style::default().fg(Color::White)),
                    Span::raw(" "),
                    Span::styled(cycle.get_title(), Style::default().fg(Color::LightYellow)), // title in light yellow
                ]);
                ListItem::new(line)
            })
            .collect();
        let mut list_state = ListState::default();
        if !cycles.is_empty() {
            list_state.select(Some(self.selected_cycle));
        }

        let cycles_list =
            List::new(items).highlight_style(Style::default().fg(Color::White).bg(Color::Green).add_modifier(Modifier::BOLD));

        StatefulWidget::render(cycles_list, cycles_inner, buf, &mut list_state);

        let mut cycles_scroll_state = ScrollbarState::default().content_length(cycles.len()).position(self.selected_cycle);
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            cycles_inner,
            buf,
            &mut cycles_scroll_state,
        );

        //----------------------------------
        // Middle column: Minions
        //----------------------------------
        let minions_block = if self.active_box == ActiveBox::Minions {
            Block::default().borders(Borders::ALL).title("Minions").border_type(BorderType::Double)
        } else {
            Block::default().borders(Borders::ALL).title("Minions")
        };
        Widget::render(&minions_block, minions_a, buf);

        let minions_inner = minions_block.inner(minions_a);
        let minion_items: Vec<ListItem> = self.minions.iter().map(|m| ListItem::new(m.as_str())).collect();
        let mut minions_state = ListState::default();
        if !self.minions.is_empty() {
            minions_state.select(Some(self.selected_minion));
        }
        let minions_list = List::new(minion_items)
            .highlight_style(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        StatefulWidget::render(minions_list, minions_inner, buf, &mut minions_state);

        let mut minions_scroll_state = ScrollbarState::default()
            .content_length(self.minions.len())
            .position(if self.active_box == ActiveBox::Minions { self.selected_minion } else { 0 });
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            minions_inner,
            buf,
            &mut minions_scroll_state,
        );

        //----------------------------------
        // Right column: Some table
        //----------------------------------
        let [model_events, event_data]: [Rect; 2] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(events_a)
            .as_ref()
            .try_into()
            .unwrap();

        let right_block = if self.active_box == ActiveBox::Events {
            Block::default().borders(Borders::ALL).title("Events").border_type(BorderType::Double)
        } else {
            Block::default().borders(Borders::ALL).title("Right Block")
        };
        Widget::render(&right_block, model_events, buf);

        let events_inner = right_block.inner(model_events);
        let events_items: Vec<ListItem> = self.events.iter().map(|m| ListItem::new(m.as_str())).collect();
        let mut events_state = ListState::default();
        if !self.events.is_empty() {
            events_state.select(Some(self.selected_event));
        }
        let events_list = List::new(events_items)
            .highlight_style(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        StatefulWidget::render(events_list, events_inner, buf, &mut events_state);

        let mut events_scroll_state = ScrollbarState::default()
            .content_length(self.events.len())
            .position(if self.active_box == ActiveBox::Events { self.selected_event } else { 0 });
        Scrollbar::default().begin_symbol(None).end_symbol(None).track_symbol(Some("░")).thumb_symbol("█").render(
            events_inner,
            buf,
            &mut events_scroll_state,
        );

        let bottom_block = Block::default().borders(Borders::ALL).title("Bottom Right");

        let header = Row::new(vec!["Key", "Value"]).style(Style::default().fg(Color::Yellow)).bottom_margin(1);
        let row1 = Row::new(vec!["foo", "bar"]);
        let row2 = Row::new(vec!["baz", "toto"]);
        let table = Table::new(vec![header, row1, row2], &[Constraint::Length(10), Constraint::Length(10)])
            .block(bottom_block)
            .column_spacing(1);
        Widget::render(table, event_data, buf);
    }
}
