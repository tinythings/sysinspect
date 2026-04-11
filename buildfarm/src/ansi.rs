use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub struct AnsiDocument {
    lines: Vec<AnsiLine>,
}

impl AnsiDocument {
    pub fn parse(text: &str) -> Self {
        Self::from_buffer(TerminalBuffer::from_text(text))
    }

    pub fn from_buffer(buffer: TerminalBuffer) -> Self {
        Self {
            lines: buffer
                .text_lines()
                .iter()
                .map(|line| AnsiLine::parse(line.as_str()))
                .collect(),
        }
    }

    pub fn lines(&self) -> Vec<Line<'static>> {
        self.lines.iter().map(AnsiLine::line).collect()
    }
}

#[derive(Clone)]
pub struct TerminalBuffer {
    lines: Vec<String>,
    row: usize,
    col: usize,
}

impl TerminalBuffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            row: 0,
            col: 0,
        }
    }

    pub fn from_text(text: &str) -> Self {
        let mut buffer = Self::new();
        buffer.push_text(text);
        buffer
    }

    pub fn push_text(&mut self, text: &str) {
        let mut chars = text.chars();

        while let Some(ch) = chars.next() {
            self.consume_char(ch, &mut chars);
        }
    }

    pub fn lines(&self) -> Vec<Line<'static>> {
        self.text_lines()
            .iter()
            .map(|line| AnsiLine::parse(line.as_str()).line())
            .collect()
    }

    fn text_lines(&self) -> Vec<String> {
        self.lines.clone()
    }

    fn consume_char(&mut self, ch: char, chars: &mut std::str::Chars<'_>) {
        match ch {
            '\r' => self.col = 0,
            '\n' => self.line_feed(),
            '\u{1b}' => self.consume_escape(chars),
            _ if ch.is_control() => {}
            _ => self.write_char(ch),
        }
    }

    fn consume_escape(&mut self, chars: &mut std::str::Chars<'_>) {
        if chars.next() == Some('[') {
            self.consume_csi(chars);
        }
    }

    fn consume_csi(&mut self, chars: &mut std::str::Chars<'_>) {
        let mut payload = String::new();

        while let Some(ch) = chars.next() {
            if Self::is_csi_final(ch) {
                self.apply_csi(&payload, ch);
                return;
            }
            payload.push(ch);
        }
    }

    fn apply_csi(&mut self, payload: &str, final_char: char) {
        match final_char {
            'A' => self.move_up(Self::count(payload)),
            'B' => self.move_down(Self::count(payload)),
            'E' => self.next_line(Self::count(payload)),
            'F' => self.previous_line(Self::count(payload)),
            'G' => self.move_column(Self::count(payload)),
            'K' => self.clear_line_from_cursor(),
            'm' => self.write_sgr(payload),
            _ => {}
        }
    }

    fn is_csi_final(ch: char) -> bool {
        ('@'..='~').contains(&ch)
    }

    fn count(payload: &str) -> usize {
        payload
            .split(';')
            .next()
            .filter(|part| !part.is_empty())
            .and_then(|part| part.parse::<usize>().ok())
            .unwrap_or(1)
    }

    fn move_up(&mut self, count: usize) {
        self.row = self.row.saturating_sub(count);
        self.ensure_row();
    }

    fn move_down(&mut self, count: usize) {
        self.row = self.row.saturating_add(count);
        self.ensure_row();
    }

    fn move_column(&mut self, column: usize) {
        self.col = column.saturating_sub(1);
    }

    fn next_line(&mut self, count: usize) {
        self.row = self.row.saturating_add(count);
        self.col = 0;
        self.ensure_row();
    }

    fn previous_line(&mut self, count: usize) {
        self.row = self.row.saturating_sub(count);
        self.col = 0;
        self.ensure_row();
    }

    fn clear_line_from_cursor(&mut self) {
        self.ensure_row();
        if self.col == 0 {
            self.lines[self.row].clear();
            return;
        }
        self.visible_byte_index(self.row, self.col)
            .into_iter()
            .for_each(|index| self.lines[self.row].truncate(index));
    }

    fn write_sgr(&mut self, payload: &str) {
        self.ensure_row();
        self.lines[self.row].push('\u{1b}');
        self.lines[self.row].push('[');
        self.lines[self.row].push_str(payload);
        self.lines[self.row].push('m');
    }

    fn write_char(&mut self, ch: char) {
        self.ensure_row();
        self.ensure_col();
        self.replace_visible_char(self.row, self.col, ch);
        self.col += 1;
    }

    fn ensure_row(&mut self) {
        while self.lines.len() <= self.row {
            self.lines.push(String::new());
        }
    }

    fn ensure_col(&mut self) {
        while self.visible_len(self.row) <= self.col {
            self.lines[self.row].push(' ');
        }
    }

    fn line_feed(&mut self) {
        self.row += 1;
        self.col = 0;
        self.ensure_row();
    }

    fn visible_len(&self, row: usize) -> usize {
        self.visible_ranges(row).len()
    }

    fn visible_byte_index(&self, row: usize, col: usize) -> Option<usize> {
        self.visible_ranges(row)
            .get(col)
            .map(|(start, _)| *start)
            .or_else(|| (col == self.visible_len(row)).then_some(self.lines[row].len()))
    }

    fn replace_visible_char(&mut self, row: usize, col: usize, ch: char) {
        self.visible_ranges(row)
            .get(col)
            .cloned()
            .into_iter()
            .for_each(|(start, end)| self.lines[row].replace_range(start..end, &ch.to_string()));
    }

    fn visible_ranges(&self, row: usize) -> Vec<(usize, usize)> {
        let line = &self.lines[row];
        let mut ranges = Vec::new();
        let mut chars = line.char_indices().peekable();

        while let Some((start, ch)) = chars.next() {
            if ch == '\u{1b}' {
                self.skip_csi(&mut chars);
                continue;
            }

            let end = chars.peek().map(|(index, _)| *index).unwrap_or(line.len());
            ranges.push((start, end));
        }

        ranges
    }

    fn skip_csi(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    ) {
        while let Some((_, marker)) = chars.next() {
            if marker == '[' {
                break;
            }
        }
        while let Some((_, marker)) = chars.next() {
            if Self::is_csi_final(marker) {
                break;
            }
        }
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
            if TerminalBuffer::is_csi_final(ch) {
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
            .map(|fg| Style::default().fg(fg))
            .unwrap_or_default()
            .add_modifier(
                self.bold
                    .then_some(Modifier::BOLD)
                    .unwrap_or(Modifier::empty()),
            )
    }
}
