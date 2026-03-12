use crate::prt::{PyLoggerState, clear_log_buffer, take_log_buffer};
use std::sync::{Arc, Mutex};

#[test]
fn take_log_buffer_drains_state() {
    let state = Arc::new(Mutex::new(PyLoggerState::new(vec!["one".to_string(), "two".to_string()], "demo".to_string())));

    assert_eq!(take_log_buffer(&state), vec!["one".to_string(), "two".to_string()]);
    assert_eq!(take_log_buffer(&state), Vec::<String>::new());
}

#[test]
fn clear_log_buffer_removes_pending_logs() {
    let state = Arc::new(Mutex::new(PyLoggerState::new(vec!["one".to_string()], "demo".to_string())));

    clear_log_buffer(&state);
    assert_eq!(take_log_buffer(&state), Vec::<String>::new());
}
