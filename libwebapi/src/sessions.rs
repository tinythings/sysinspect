//! Memory sessions
//!
//! This module provides an in-memory session store for user sessions.
//! It is designed for simplicity and ease of use, but is not suitable for production use.

use libsysinspect::SysinspectError;
use once_cell::sync::OnceCell;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sodiumoxide::crypto::secretbox;
use sodiumoxide::crypto::secretbox::Key;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
pub struct Session {
    pub uid: String,
    pub created: Instant,
    pub timeout: Duration,
    pub symkey: Key, // Sodium symmetric key for data encryption
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
    pub fn open(&mut self, uid: String) -> Result<String, SysinspectError> {
        reap_expired!(self);

        if let Some(esid) = self.sessions.iter().find_map(|(sid, s)| if s.uid == uid { Some(sid.clone()) } else { None }) {
            self.sessions.remove(&esid);
        }

        let sid = uuid::Uuid::new_v4().to_string();
        self.sessions.insert(sid.clone(), Session { uid, created: Instant::now(), timeout: self.default_timeout, symkey: secretbox::gen_key() });

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

    /// Returns the symmetric key for the session ID, if it exists and not expired.
    /// The key is used for encrypting/decrypting data associated with the session.
    /// If the session is expired, it will be removed from the store.
    /// If the session does not exist, returns None.
    /// If the session exists but is expired, it will be removed and None will be returned.
    /// If the session exists and is valid, returns Some(Key).
    ///
    /// # Arguments
    /// * `sid` - The session ID for which to retrieve the symmetric key.
    /// # Returns
    /// * `Some(Key)` if the session exists and is valid.
    /// * `None` if the session does not exist or is expired.
    /// # Errors
    /// * If the session ID is not found or is expired, it will be removed from the store and `None` will be returned.
    /// * If the session ID is not valid, it will return `None`.
    ///
    pub(crate) fn key(&mut self, sid: &str) -> Option<Key> {
        reap_expired!(self);
        if let Some(session) = self.sessions.get(sid) {
            if session.created.elapsed() < session.timeout {
                return Some(session.symkey.clone());
            } else {
                self.sessions.remove(sid);
            }
        }
        None
    }

    /// Encrypts a value using the session's symmetric key and returns the nonce and ciphertext.
    /// The nonce is used to ensure that the same plaintext encrypted multiple times will yield different ciphertext.
    /// The ciphertext is the encrypted form of the serialized value.
    /// # Arguments
    /// * `value` - The value to encrypt, which must implement the `Serialize` trait.
    /// * `key` - The symmetric key to use for encryption, which is derived from the session.
    /// # Returns
    /// * `(Vec<u8>, Vec<u8>)` - A tuple containing the nonce and the ciphertext.
    /// # Errors
    /// * If the value cannot be serialized, it will panic.
    /// * If the key is not valid, it will panic.
    ///
    pub fn encrypt<T: Serialize>(&mut self, sid: &str, value: &T) -> Result<(Vec<u8>, Vec<u8>), SysinspectError> {
        let key = self.key(sid).ok_or(SysinspectError::ObjectNotFound("Session key not found".to_string()))?;
        let nonce = secretbox::gen_nonce();
        let data = serde_json::to_vec(value).map_err(|e| SysinspectError::SerializationError(e.to_string()))?;
        Ok((nonce.0.to_vec(), secretbox::seal(&data, &nonce, &key)))
    }

    /// Decrypts the ciphertext using the session's symmetric key and returns the deserialized value.
    /// The nonce is used to ensure that the ciphertext can be decrypted correctly.
    /// # Arguments
    /// * `sid` - The session ID to use for retrieving the symmetric key.
    /// * `nonce` - The nonce used during encryption, which is required for decryption.
    /// * `ct` - The ciphertext to decrypt, which must be the result of a previous encryption operation.
    /// # Returns
    /// * `T` - The deserialized value of type `T`, which must implement the `DeserializeOwned` trait.
    /// # Errors
    /// * If the session key is not found, it will return an error.
    /// * If the ciphertext cannot be decrypted, it will panic.
    /// * If the deserialization fails, it will panic.
    /// # Panics
    /// * If the ciphertext is invalid or cannot be decrypted, it will panic.
    /// * If the deserialization fails, it will panic.
    ///
    pub fn decrypt<T: DeserializeOwned>(&mut self, sid: &str, nonce: &[u8], ct: &[u8]) -> Result<T, SysinspectError> {
        let nonce = secretbox::Nonce::from_slice(nonce).unwrap();
        let key = self.key(sid).ok_or(SysinspectError::ObjectNotFound("Session key not found".to_string()))?;
        let pt = match secretbox::open(ct, &nonce, &key) {
            Ok(pt) => pt,
            Err(_) => return Err(SysinspectError::ObjectNotFound("Failed to decrypt data for whatever reasons".to_string())),
        };
        serde_json::from_slice(&pt).map_err(|e| SysinspectError::DeserializationError(e.to_string()))
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
