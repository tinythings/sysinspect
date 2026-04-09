use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub struct AnsiDocument<'a> {
    lines: Vec<AnsiLine<'a>>,
}

impl<'a> AnsiDocument<'a> {
    pub fn parse(text: &'a str) -> Self {
        Self {
            lines: text.split('\n').map(AnsiLine::parse).collect(),
        }
    }

    pub fn lines(&self) -> Vec<Line<'a>> {
        self.lines.iter().map(AnsiLine::line).collect()
    }
}

struct AnsiLine<'a> {
    spans: Vec<AnsiSpan<'a>>,
}

impl<'a> AnsiLine<'a> {
    fn parse(text: &'a str) -> Self {
        Self {
            spans: AnsiParser::new(text).parse(),
        }
    }

    fn line(&self) -> Line<'a> {
        Line::from(self.spans.iter().map(AnsiSpan::span).collect::<Vec<_>>())
    }
}

struct AnsiSpan<'a> {
    text: &'a str,
    style: Style,
}

impl<'a> AnsiSpan<'a> {
    fn new(text: &'a str, style: Style) -> Self {
        Self { text, style }
    }

    fn span(&self) -> Span<'a> {
        Span::styled(self.text, self.style)
    }
}

struct AnsiParser<'a> {
    text: &'a str,
    cursor: usize,
    start: usize,
    state: AnsiState,
    spans: Vec<AnsiSpan<'a>>,
}

impl<'a> AnsiParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            cursor: 0,
            start: 0,
            state: AnsiState::default(),
            spans: Vec::new(),
        }
    }

    fn parse(mut self) -> Vec<AnsiSpan<'a>> {
        while self.cursor < self.text.len() {
            if self.at_escape() {
                self.consume_escape();
            } else {
                self.advance();
            }
        }

        self.push_text(self.text.len());
        self.spans
    }

    fn at_escape(&self) -> bool {
        self.text[self.cursor..].starts_with("\u{1b}[")
    }

    fn consume_escape(&mut self) {
        self.push_text(self.cursor);
        self.cursor += 2;
        self.cursor
            .pipe_ref(|cursor| self.text[*cursor..].find('m').map(|offset| self.cursor + offset))
            .and_then(|end| {
                self.state = self.text[self.cursor..end]
                    .split(';')
                    .filter(|code| !code.is_empty())
                    .map(|code| code.parse::<u16>().unwrap_or(0))
                    .collect::<Vec<_>>()
                    .pipe_ref(|codes| self.state.apply(codes));
                Some(end + 1)
            })
            .unwrap_or_else(|| self.text.len())
            .pipe_ref(|cursor| {
                self.cursor = *cursor;
                self.start = *cursor;
            });
    }

    fn advance(&mut self) {
        self.cursor += 1;
    }

    fn push_text(&mut self, end: usize) {
        (self.start < end)
            .then_some(AnsiSpan::new(&self.text[self.start..end], self.state.style()))
            .into_iter()
            .for_each(|span| self.spans.push(span));
        self.start = end;
    }
}

#[derive(Clone, Copy)]
struct AnsiState {
    fg: Option<Color>,
    bold: bool,
}

impl Default for AnsiState {
    fn default() -> Self {
        Self { fg: None, bold: false }
    }
}

impl AnsiState {
    fn apply(&self, codes: &[u16]) -> Self {
        codes.iter().fold(*self, |state, code| state.apply_code(*code))
    }

    fn apply_code(&self, code: u16) -> Self {
        match code {
            0 => Self::default(),
            1 => Self { bold: true, ..*self },
            22 => Self { bold: false, ..*self },
            30 => Self { fg: Some(Color::Black), ..*self },
            31 => Self { fg: Some(Color::Red), ..*self },
            32 => Self { fg: Some(Color::Green), ..*self },
            33 => Self { fg: Some(Color::Yellow), ..*self },
            34 => Self { fg: Some(Color::Blue), ..*self },
            35 => Self { fg: Some(Color::Magenta), ..*self },
            36 => Self { fg: Some(Color::Cyan), ..*self },
            37 => Self { fg: Some(Color::Gray), ..*self },
            39 => Self { fg: None, ..*self },
            90 => Self { fg: Some(Color::DarkGray), ..*self },
            91 => Self { fg: Some(Color::LightRed), ..*self },
            92 => Self { fg: Some(Color::LightGreen), ..*self },
            93 => Self { fg: Some(Color::LightYellow), ..*self },
            94 => Self { fg: Some(Color::LightBlue), ..*self },
            95 => Self { fg: Some(Color::LightMagenta), ..*self },
            96 => Self { fg: Some(Color::LightCyan), ..*self },
            97 => Self { fg: Some(Color::White), ..*self },
            _ => *self,
        }
    }

    fn style(&self) -> Style {
        self.fg
            .pipe_ref(|fg| fg.map(|color| Style::default().fg(color)).unwrap_or_else(Style::default))
            .pipe_ref(|style| {
                self.bold
                    .then_some(style.add_modifier(Modifier::BOLD))
                    .unwrap_or(*style)
            })
    }
}

trait PipeRef: Sized {
    fn pipe_ref<T>(self, f: impl FnOnce(&Self) -> T) -> T {
        f(&self)
    }
}

impl<T> PipeRef for T {}
