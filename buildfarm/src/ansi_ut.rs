use ratatui::style::{Color, Modifier};

use crate::ansi::AnsiDocument;

#[test]
fn ansi_document_keeps_plain_text() {
    let lines = AnsiDocument::parse("hello").lines();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].content, "hello");
}

#[test]
fn ansi_document_preserves_bright_red_text() {
    let lines = AnsiDocument::parse("\u{1b}[1;91mFAIL\u{1b}[0m").lines();

    assert_eq!(lines[0].spans[0].content, "FAIL");
    assert_eq!(lines[0].spans[0].style.fg, Some(Color::LightRed));
    assert!(lines[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn ansi_document_splits_multiple_colored_spans() {
    let lines = AnsiDocument::parse("A \u{1b}[92mOK\u{1b}[0m B").lines();

    assert_eq!(lines[0].spans.len(), 3);
    assert_eq!(lines[0].spans[1].content, "OK");
    assert_eq!(lines[0].spans[1].style.fg, Some(Color::LightGreen));
}

#[test]
fn ansi_document_keeps_multiple_lines() {
    let lines = AnsiDocument::parse("one\ntwo").lines();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].spans[0].content, "one");
    assert_eq!(lines[1].spans[0].content, "two");
}
