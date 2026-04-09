use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub struct AnsiDocument {
    lines: Vec<AnsiLine>,
}

impl AnsiDocument {
    pub fn parse(text: &str) -> Self {
        AnsiNormalizer::new(text).normalize().pipe_ref(|normalized| Self {
            lines: normalized
                .split('\n')
                .map(AnsiLine::parse)
                .collect(),
        })
    }

    pub fn lines(&self) -> Vec<Line<'static>> {
        self.lines.iter().map(AnsiLine::line).collect()
    }
}

struct AnsiLine {
    spans: Vec<AnsiSpan>,
}

impl AnsiLine {
    fn parse(text: &str) -> Self {
        Self {
            spans: AnsiParser::new(text).parse(),
        }
    }

    fn line(&self) -> Line<'static> {
        Line::from(self.spans.iter().map(AnsiSpan::span).collect::<Vec<_>>())
    }
}

struct AnsiSpan {
    text: String,
    style: Style,
}

impl AnsiSpan {
    fn new(text: String, style: Style) -> Self {
        Self { text, style }
    }

    fn span(&self) -> Span<'static> {
        Span::styled(self.text.clone(), self.style)
    }
}

struct AnsiParser<'a> {
    chars: std::str::Chars<'a>,
    state: AnsiState,
    spans: Vec<AnsiSpan>,
    current: String,
}

impl<'a> AnsiParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars(),
            state: AnsiState::default(),
            spans: Vec::new(),
            current: String::new(),
        }
    }

    fn parse(mut self) -> Vec<AnsiSpan> {
        while let Some(ch) = self.chars.next() {
            self.consume_char(ch);
        }

        self.flush_current();
        self.spans
    }

    fn consume_char(&mut self, ch: char) {
        if ch == '\u{1b}' {
            self.consume_escape();
            return;
        }
        if !ch.is_control() {
            self.current.push(ch);
        }
    }

    fn consume_escape(&mut self) {
        self.flush_current();
        self.chars
            .next()
            .filter(|marker| *marker == '[')
            .into_iter()
            .for_each(|_| self.consume_csi());
    }

    fn consume_csi(&mut self) {
        let mut payload = String::new();

        while let Some(ch) = self.chars.next() {
            if Self::is_csi_final(ch) {
                self.apply_csi(&payload, ch);
                return;
            }
            payload.push(ch);
        }
    }

    fn apply_csi(&mut self, payload: &str, final_char: char) {
        if final_char == 'm' {
            self.state = self.state.apply(
                &payload
                    .split(';')
                    .filter(|code| !code.is_empty())
                    .map(|code| code.parse::<u16>().unwrap_or(0))
                    .collect::<Vec<_>>(),
            );
        }
    }

    fn is_csi_final(ch: char) -> bool {
        ('@'..='~').contains(&ch)
    }

    fn flush_current(&mut self) {
        (!self.current.is_empty())
            .then_some(AnsiSpan::new(
                std::mem::take(&mut self.current),
                self.state.style(),
            ))
            .into_iter()
            .for_each(|span| self.spans.push(span));
    }
}

struct AnsiNormalizer<'a> {
    text: &'a str,
}

impl<'a> AnsiNormalizer<'a> {
    fn new(text: &'a str) -> Self {
        Self { text }
    }

    fn normalize(&self) -> String {
        self.text
            .chars()
            .filter_map(Self::normalize_char)
            .collect()
    }

    fn normalize_char(ch: char) -> Option<char> {
        match ch {
            '\r' => None,
            '\n' | '\u{1b}' => Some(ch),
            _ if ch.is_control() => None,
            _ => Some(ch),
        }
    }
}

#[derive(Clone, Copy)]
struct AnsiState {
    fg: Option<Color>,
    bold: bool,
}

impl Default for AnsiState {
    fn default() -> Self {
        Self {
            fg: None,
            bold: false,
        }
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
            22 => Self {
                bold: false,
                ..*self
            },
            30 => Self {
                fg: Some(Color::Black),
                ..*self
            },
            31 => Self {
                fg: Some(Color::Red),
                ..*self
            },
            32 => Self {
                fg: Some(Color::Green),
                ..*self
            },
            33 => Self {
                fg: Some(Color::Yellow),
                ..*self
            },
            34 => Self {
                fg: Some(Color::Blue),
                ..*self
            },
            35 => Self {
                fg: Some(Color::Magenta),
                ..*self
            },
            36 => Self {
                fg: Some(Color::Cyan),
                ..*self
            },
            37 => Self {
                fg: Some(Color::Gray),
                ..*self
            },
            39 => Self { fg: None, ..*self },
            90 => Self {
                fg: Some(Color::DarkGray),
                ..*self
            },
            91 => Self {
                fg: Some(Color::LightRed),
                ..*self
            },
            92 => Self {
                fg: Some(Color::LightGreen),
                ..*self
            },
            93 => Self {
                fg: Some(Color::LightYellow),
                ..*self
            },
            94 => Self {
                fg: Some(Color::LightBlue),
                ..*self
            },
            95 => Self {
                fg: Some(Color::LightMagenta),
                ..*self
            },
            96 => Self {
                fg: Some(Color::LightCyan),
                ..*self
            },
            97 => Self {
                fg: Some(Color::White),
                ..*self
            },
            _ => *self,
        }
    }

    fn style(&self) -> Style {
        self.fg
            .pipe_ref(|fg| {
                fg.map(|color| Style::default().fg(color))
                    .unwrap_or_else(Style::default)
            })
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
