//! Memory sessions
//!
//! This module provides an in-memory session store for user sessions.
//! It is designed for simplicity and ease of use, but is not suitable for production use.

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
pub struct Session {
    pub uid: String,
    pub created: Instant,
    pub timeout: Duration,
}
macro_rules! reap_expired {
    ($self:ident) => {
        $self.sessions.retain(|_, session| session.created.elapsed() < session.timeout);
    };
}

pub struct SessionStore {
    sessions: HashMap<String, Session>,
    default_timeout: Duration,
}

impl SessionStore {
    pub fn new() -> Self {
        SessionStore {
            sessions: HashMap::new(),
            default_timeout: Duration::from_secs(3600), // 1 hour default
        }
    }

    /// Opens a new session for the given user ID and returns the session ID.
    /// If a session for the given uid exists, remove it first before creating a new one.
    pub fn open(&mut self, uid: String) -> String {
        reap_expired!(self);

        if let Some(esid) = self.sessions.iter().find_map(|(sid, s)| if s.uid == uid { Some(sid.clone()) } else { None }) {
            self.sessions.remove(&esid);
        }

        let sid = uuid::Uuid::new_v4().to_string();
        self.sessions.insert(sid.clone(), Session { uid, created: Instant::now(), timeout: self.default_timeout });

        sid
    }

    /// Returns the user ID associated with the session ID, if it exists and not expired.
    pub fn uid(&mut self, sid: &str) -> Option<String> {
        reap_expired!(self);
        if let Some(session) = self.sessions.get(sid) {
            if session.created.elapsed() < session.timeout {
                return Some(session.uid.clone());
            } else {
                // Session expired, remove it
                self.sessions.remove(sid);
            }
        }
        None
    }

    /// Updates the session's last activity time to prevent it from expiring.
    pub fn ping(&mut self, sid: &str) {
        reap_expired!(self);
        if let Some(session) = self.sessions.get_mut(sid) {
            session.created = Instant::now();
        }
    }

    /// Close the session by removing it from the store.
    pub fn close(&mut self, sid: &str) {
        reap_expired!(self);
        self.sessions.remove(sid);
    }

    /// Optionally, allow setting a custom timeout for new sessions
    pub fn set_default_timeout(&mut self, timeout: Duration) {
        reap_expired!(self);
        self.default_timeout = timeout;
    }
}

/// Global session store instance
static _SESSIONS: OnceCell<Mutex<SessionStore>> = OnceCell::new();

/// Returns a reference to the global session store.
pub fn get_session_store() -> &'static Mutex<SessionStore> {
    _SESSIONS.get_or_init(|| Mutex::new(SessionStore::new()))
}
