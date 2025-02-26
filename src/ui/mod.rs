use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::Rng;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Buffer, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Row, Scrollbar, ScrollbarState, StatefulWidget, Table, Widget,
    },
    DefaultTerminal, Frame,
};
use std::io;

pub fn run() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let r = App::default().run(&mut terminal);
    ratatui::restore();
    r
}

#[derive(Debug, PartialEq, Eq)]
pub enum ActiveBox {
    Cycles,
    Minions,
}

#[derive(Debug)]
pub struct App {
    exit: bool,
    pub selected_cycle: usize,
    pub selected_minion: usize,

    pub minions: Vec<String>,
    pub active_box: ActiveBox,
}

impl Default for App {
    fn default() -> Self {
        Self { exit: false, selected_cycle: 0, selected_minion: 0, minions: Vec::new(), active_box: ActiveBox::Cycles }
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
        frame.render_widget(self, frame.area());
    }

    fn on_events(&mut self) -> io::Result<()> {
        if let Event::Key(e) = event::read()? {
            if e.kind == KeyEventKind::Press {
                self.on_key(e);
            }
        }
        Ok(())
    }

    fn on_key(&mut self, e: event::KeyEvent) {
        match e.code {
            KeyCode::Up => {
                if self.active_box == ActiveBox::Cycles {
                    if self.selected_cycle > 0 {
                        self.selected_cycle -= 1;
                    }
                } else if self.selected_minion > 0 {
                    self.selected_minion -= 1;
                }
            }
            KeyCode::Down => {
                if self.active_box == ActiveBox::Cycles {
                    let cycles = self.get_cycles();
                    if self.selected_cycle < cycles.len().saturating_sub(1) {
                        self.selected_cycle += 1;
                    }
                } else if self.selected_minion < self.minions.len().saturating_sub(1) {
                    self.selected_minion += 1;
                }
            }
            KeyCode::Right => {
                self.active_box = ActiveBox::Minions;
            }
            KeyCode::Left => {
                self.active_box = ActiveBox::Cycles;
            }
            KeyCode::Enter => {
                if self.active_box == ActiveBox::Cycles {
                    let cycles = self.get_cycles();
                    if !cycles.is_empty() {
                        self.minions = self.get_minions();
                        self.selected_minion = 0;
                    }
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
    pub fn get_cycles(&self) -> Vec<String> {
        (0..100).map(|x| format!("cycle {}", x)).collect()
    }

    /// Returns a vector of minion names (random IDs).
    pub fn get_minions(&self) -> Vec<String> {
        (0..100).map(|x| format!("minion-{}-{}", x, rand::rng().random_range(0..100))).collect()
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
            Block::default().borders(Borders::ALL).title("Cycles").border_type(BorderType::Double)
        } else {
            Block::default().borders(Borders::ALL).title("Cycles")
        };
        Widget::render(&cycles_block, cycles_a, buf);

        let cycles_inner = cycles_block.inner(cycles_a);
        let cycles = self.get_cycles();
        let items: Vec<ListItem> = cycles.iter().map(|cycle| ListItem::new(cycle.as_str())).collect();
        let mut list_state = ListState::default();
        if !cycles.is_empty() {
            list_state.select(Some(self.selected_cycle));
        }

        let cycles_list = List::new(items)
            .highlight_style(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

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
        let right_block = Block::default().borders(Borders::ALL).title("Right Box");
        let header = Row::new(vec!["Key", "Value"]).style(Style::default().fg(Color::Yellow)).bottom_margin(1);
        let row1 = Row::new(vec!["foo", "bar"]);
        let row2 = Row::new(vec!["baz", "toto"]);

        let table = Table::new(vec![header, row1, row2], &[Constraint::Length(10), Constraint::Length(10)])
            .block(right_block)
            .column_spacing(1);

        Widget::render(table, events_a, buf);
    }
}
