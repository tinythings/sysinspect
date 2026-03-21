//! In-memory Web API sessions.

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

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
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
    ///
    /// # Arguments
    /// * `uid` - The user ID for which to create a session.
    /// # Returns
    /// * `Result<String, SysinspectError>` - The session ID if successful, or an error if the session could not be created.
    ///
    pub fn open(&mut self, uid: String) -> Result<String, libcommon::SysinspectError> {
        reap_expired!(self);

        if let Some(esid) = self.sessions.iter().find_map(|(sid, s)| if s.uid == uid { Some(sid.clone()) } else { None }) {
            self.sessions.remove(&esid);
        }

        let sid = uuid::Uuid::new_v4().to_string();
        self.sessions.insert(sid.clone(), Session { uid, created: Instant::now(), timeout: self.default_timeout });

        Ok(sid)
    }

    pub fn open_with_sid(&mut self, uid: String, sid: String) -> Result<String, libcommon::SysinspectError> {
        reap_expired!(self);

        if let Some(esid) = self.sessions.iter().find_map(|(existing_sid, s)| if s.uid == uid { Some(existing_sid.clone()) } else { None }) {
            self.sessions.remove(&esid);
        }

        self.sessions.insert(sid.clone(), Session { uid, created: Instant::now(), timeout: self.default_timeout });

        Ok(sid)
    }

    /// Returns the user ID associated with the session ID, if it exists and not expired.
    /// If the session is expired, it will be removed from the store.
    /// Returns `None` if the session does not exist or is expired.
    ///
    /// # Arguments
    /// * `sid` - The session ID to look up.
    /// # Returns
    /// * `Option<String>` - The user ID if the session is valid, otherwise `None`.
    ///
    pub fn uid(&mut self, sid: &str) -> Option<String> {
        reap_expired!(self);
        if let Some(session) = self.sessions.get(sid) {
            if session.created.elapsed() < session.timeout {
                return Some(session.uid.clone());
            } else {
                self.sessions.remove(sid);
            }
        }
        None
    }

    /// Updates the session's last activity time to prevent it from expiring.
    /// This method should be called periodically to keep the session alive.
    /// It resets the session's `created` time to the current time.
    /// # Arguments
    /// * `sid` - The session ID to ping.
    /// # Returns
    /// * None
    /// # Errors
    /// * If the session ID does not exist, it will do nothing.
    ///
    pub fn ping(&mut self, sid: &str) {
        reap_expired!(self);
        if let Some(session) = self.sessions.get_mut(sid) {
            session.created = Instant::now();
        }
    }

    /// Close the session by removing it from the store.
    /// This method will also reap expired sessions before closing the specified session.
    /// # Arguments
    /// * `sid` - The session ID to close.
    /// # Returns
    /// * None
    /// # Errors
    /// * If the session ID does not exist, it will do nothing.
    ///
    pub fn close(&mut self, sid: &str) {
        reap_expired!(self);
        self.sessions.remove(sid);
    }

    /// Optionally, allow setting a custom timeout for new sessions.
    /// This method will reap expired sessions before setting the new default timeout.
    /// # Arguments
    /// * `timeout` - The new default timeout duration for sessions.
    /// # Returns
    /// * None
    /// # Errors
    /// * None
    ///
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
