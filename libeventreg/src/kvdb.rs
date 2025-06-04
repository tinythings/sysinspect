use chrono::{DateTime, Utc};
use colored::Colorize;
use fs_extra::dir::{CopyOptions, copy};
use indexmap::IndexMap;
use libsysinspect::{
    SysinspectError,
    cfg::mmconf::HistoryConfig,
    util::{self},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sled::{Db, Tree};
use std::{collections::HashMap, fmt::Debug, fs, path::PathBuf, sync::Mutex};
use tempfile::Builder;

const TR_SESSIONS: &str = "sessions";

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct EventData {
    data: HashMap<String, Value>,
}
impl EventData {
    fn new(payload: HashMap<String, Value>) -> Self {
        Self { data: payload }
    }

    pub fn get_entity_id(&self) -> String {
        util::dataconv::as_str(self.data.get("eid").cloned())
    }

    pub fn get_action_id(&self) -> String {
        util::dataconv::as_str(self.data.get("aid").cloned())
    }

    pub fn get_status_id(&self) -> String {
        util::dataconv::as_str(self.data.get("sid").cloned())
    }

    pub fn get_event_id(&self) -> String {
        format!("{}/{}/{}", self.get_entity_id(), self.get_status_id(), self.get_action_id())
    }

    pub fn get_cycle_id(&self) -> String {
        util::dataconv::as_str(self.data.get("cid").cloned())
    }

    pub fn get_constraints(&self) -> HashMap<String, Value> {
        serde_json::from_value(self.data.get("constraints").unwrap().clone()).unwrap()
    }

    pub fn get_response(&self) -> HashMap<String, Value> {
        // Should work... :-)
        serde_json::from_value(self.data.get("response").unwrap().clone()).unwrap()
    }

    pub fn get_response_mut(&mut self) -> Result<&mut serde_json::Map<String, Value>, String> {
        self.data
            .get_mut("response")
            .ok_or_else(|| "Key 'response' not found in data".to_string())?
            .as_object_mut()
            .ok_or_else(|| "Value for 'response' is not an object".to_string())
    }

    /// Get the timestamp
    pub fn get_timestamp(&self) -> String {
        util::dataconv::as_str(self.data.get("timestamp").cloned())
    }

    pub fn from_bytes(b: Vec<u8>) -> Result<Self, SysinspectError> {
        match String::from_utf8(b) {
            Ok(data) => Ok(serde_json::from_str::<Self>(&data)?),
            Err(err) => Err(SysinspectError::MasterGeneralError(format!("Unable to recover event minion: {err}"))),
        }
    }

    /// Flattens the entire data into IndexMap<String, String>
    pub fn flatten(&self) -> IndexMap<String, String> {
        let mut out = IndexMap::new();
        Self::_flatten(self.data.get("response").unwrap(), "", &mut out);
        out
    }

    fn _flatten(value: &Value, prefix: &str, result: &mut IndexMap<String, String>) {
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    let new_prefix = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                    Self::_flatten(v, &new_prefix, result);
                }
            }
            Value::Array(arr) => {
                for (i, v) in arr.iter().enumerate() {
                    let new_prefix = format!("{}[{}]", prefix, i);
                    Self::_flatten(v, &new_prefix, result);
                }
            }
            _ => {
                result.insert(prefix.to_string(), value.to_string());
            }
        }
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventMinion {
    mid: String,
    cycles_id: Option<String>, // Is set later
    traits: HashMap<String, Value>,
}

impl EventMinion {
    pub fn new(mid: String) -> Self {
        EventMinion { mid, cycles_id: None, traits: HashMap::new() }
    }

    /// Minion Id
    pub fn id(&self) -> &str {
        &self.mid
    }

    /// Cycles Id
    pub fn cid(&self) -> String {
        self.cycles_id.clone().unwrap_or_default()
    }

    pub fn get_trait(&self, id: &str) -> Option<&Value> {
        self.traits.get(id)
    }

    pub fn from_bytes(b: Vec<u8>) -> Result<Self, SysinspectError> {
        match String::from_utf8(b) {
            Ok(data) => Ok(serde_json::from_str::<Self>(&data)?),
            Err(err) => Err(SysinspectError::MasterGeneralError(format!("Unable to recover event minion: {err}"))),
        }
    }

    pub fn set_cid(&mut self, cid: String) {
        self.cycles_id = Some(cid);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventSession {
    /// sysinspect query
    query: String,

    /// Timestamp
    ts: DateTime<Utc>,

    /// Session ID
    sid: String,
}

impl EventSession {
    pub fn new(query: String, sid: String, ts: DateTime<Utc>) -> Self {
        EventSession { query, sid, ts }
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>, SysinspectError> {
        Ok(serde_json::to_string(self)?.into_bytes())
    }

    pub fn from_bytes(b: Vec<u8>) -> Result<Self, SysinspectError> {
        match String::from_utf8(b) {
            Ok(data) => Ok(serde_json::from_str::<Self>(&data)?),
            Err(err) => Err(SysinspectError::MasterGeneralError(format!("Unable to recover event session: {err}"))),
        }
    }

    pub fn get_ts_rfc3339(&self) -> String {
        self.ts.to_rfc3339()
    }

    pub fn get_ts_mask(&self, m: Option<&str>) -> String {
        self.ts.format(m.unwrap_or("%Y.%m.%d %H:%M")).to_string()
    }

    pub fn get_ts_unix(&self) -> i64 {
        self.ts.timestamp()
    }

    pub fn sid(&self) -> &str {
        &self.sid
    }

    pub fn query(&self) -> &str {
        &self.query
    }
}

#[derive(Debug)]
pub struct EventsRegistry {
    conn: Db,
    cloned: Option<PathBuf>,
    cfg: HistoryConfig,
    trimlock: Mutex<()>,
}

impl Default for EventsRegistry {
    fn default() -> Self {
        Self { conn: sled::Config::new().temporary(true).open().unwrap(), cloned: None, cfg: HistoryConfig::default(), trimlock: Mutex::new(()) } // open in memory
    }
}

impl EventsRegistry {
    pub fn new(p: PathBuf, cfg: HistoryConfig) -> Result<EventsRegistry, SysinspectError> {
        log::info!("Opening database registry at {}", p.to_str().unwrap_or_default());
        if !p.exists() {
            fs::create_dir_all(&p)?;
        }

        Ok(EventsRegistry {
            conn: match sled::open(p) {
                Ok(db) => db,
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
            },
            cloned: None,
            cfg,
            trimlock: Mutex::new(()),
        })
    }

    /// Should be explicitly called on exit
    pub fn cleanup(&self) -> Result<(), SysinspectError> {
        if let Some(cloned) = self.cloned.clone() {
            return Ok(fs::remove_dir_all(cloned)?);
        }

        Ok(())
    }

    /// This is a brute-force copy of the database, because sled doesn't allow open database
    /// in read-only mode from the other processes if it is already opened.
    pub fn clone(p: PathBuf) -> Result<EventsRegistry, SysinspectError> {
        let mut options = CopyOptions::new();
        options.overwrite = true;
        options.copy_inside = true;

        let prefix = "sysinspect-db-clone-";
        let pattern = format!("/tmp/{}*", prefix);
        for entry in glob::glob(&pattern).unwrap() {
            match entry {
                Ok(path) if path.is_dir() => {
                    log::info!("Cleanup stale clone: {:?}", path);
                    fs::remove_dir_all(path)?;
                }
                Ok(_) => {} // Not a directory, skip it.
                Err(e) => eprintln!("Error matching path: {:?}", e),
            }
        }

        let tmpdir = Builder::new().prefix(prefix).tempdir()?.into_path();
        log::info!("Cloned database to {}", tmpdir.to_str().unwrap_or_default());
        copy(p, &tmpdir, &options).map(|_| ()).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        Ok(EventsRegistry {
            conn: match sled::open(&tmpdir) {
                Ok(db) => db,
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("{err}"))),
            },
            cloned: Some(tmpdir),
            cfg: HistoryConfig::default(),
            trimlock: Mutex::new(()),
        })
    }

    /// Get a tree by a tree Id
    fn get_tree(&self, tid: &str) -> Result<Tree, SysinspectError> {
        match self.conn.open_tree(tid) {
            Ok(tree) => Ok(tree),
            Err(err) => Err(SysinspectError::MasterGeneralError(format!("Unable to open {tid} database: {err}"))),
        }
    }

    /// Return a tree Id out of sid and mid
    fn to_tree_id(sid: &str, mid: &str) -> String {
        format!("{}@{}", sid, mid)
    }

    /// Add an event
    pub fn add_event(&mut self, sid: &EventSession, mid: EventMinion, payload: HashMap<String, Value>) -> Result<(), SysinspectError> {
        let events = self.get_tree(&Self::to_tree_id(sid.sid(), mid.id()))?;
        if let Err(err) = events.insert(
            format!(
                "{}/{}/{}",
                util::dataconv::as_str(payload.get("eid").cloned()),
                util::dataconv::as_str(payload.get("sid").cloned()),
                util::dataconv::as_str(payload.get("aid").cloned())
            ),
            serde_json::to_string(&payload)?.as_bytes(),
        ) {
            Err(SysinspectError::MasterGeneralError(format!("{err}")))
        } else {
            Ok(())
        }
    }

    /// This either creates a new session or returns the existing one
    pub fn open_session(&self, model: String, sid: String, ts: String) -> Result<EventSession, SysinspectError> {
        let sessions = self.get_tree(TR_SESSIONS)?;
        if !sessions.contains_key(&sid)? {
            let es = EventSession::new(model.to_owned(), sid.to_owned(), ts.parse().unwrap_or(Utc::now()));
            if let Err(err) = sessions.insert(&sid, es.as_bytes()?) {
                return Err(SysinspectError::MasterGeneralError(format!("Error opening events session: {err}")));
            }
            log::trace!("Session {} for {} registered", sid.yellow(), model.bright_yellow());
            return Ok(es);
        } else if let Some(sb) = sessions.get(&sid)? {
            log::trace!("Returning an existing session: {sid}");
            return EventSession::from_bytes(sb.to_vec());
        }

        Err(SysinspectError::MasterGeneralError("Session not found".to_string()))
    }

    /// Ensure that the minion data is there.
    pub fn ensure_minion(&mut self, sid: &EventSession, mid: String, traits: HashMap<String, Value>) -> Result<String, SysinspectError> {
        let session_minions = self.get_tree(sid.sid())?;
        if !session_minions.contains_key(&mid)? {
            log::debug!("Adding minion: {mid} at {}", sid.sid().green());
            session_minions.insert(&mid, serde_json::to_string(&traits)?.as_bytes())?;
        } else {
            log::debug!("Minion already in the database {mid} at {}", sid.sid().yellow());
        }

        Ok(mid)
    }

    /// Get the last session from the database.
    ///
    /// > **NOTE:** Not ideal, because we need to get the *entire* list of sessions and sort them.
    /// > On the other hand not entirely critical, because the database meant to be periodically purged
    /// > and the number of sessions is expected to be small enough.
    pub fn get_last_session(&self) -> Result<EventSession, SysinspectError> {
        if let Some(last) = self.get_sessions()?.last() {
            return Ok(last.clone());
        }
        Err(SysinspectError::MasterGeneralError("No sessions found".to_string()))
    }

    /// Return existing recorded sessions
    pub fn get_sessions(&self) -> Result<Vec<EventSession>, SysinspectError> {
        if self.cfg.rotate() {
            if let Ok(_lock) = self.trimlock.try_lock() {
                self.trim_data()?;
            }
        }

        let mut ks = Vec::<EventSession>::new();
        let sessions = self.get_tree(TR_SESSIONS)?;
        for v in sessions.iter().values() {
            let v = match v {
                Ok(v) => String::from_utf8(v.to_vec()).unwrap_or_default(),
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Error getting sessions: {err}"))),
            };
            ks.push(EventSession::from_bytes(v.as_bytes().to_vec())?);
        }

        // Always sort them by date/time, ascending
        ks.sort_by_key(|x| x.get_ts_unix());

        Ok(ks)
    }

    pub fn get_session(&self, sid: &str) -> Result<EventSession, SysinspectError> {
        let sessions = self.get_tree(TR_SESSIONS)?;
        if let Some(v) = sessions.get(sid)? {
            let v = String::from_utf8(v.to_vec()).unwrap_or_default();
            return EventSession::from_bytes(v.as_bytes().to_vec());
        }

        Err(SysinspectError::MasterGeneralError(format!("Session {} not found", sid)))
    }

    /// Return all minions within the session
    pub fn get_minions(&self, sid: &str) -> Result<Vec<EventMinion>, SysinspectError> {
        let mut ms = Vec::<EventMinion>::new();
        let minions = self.get_tree(sid)?;
        for data in minions.iter().values() {
            let traits = match data {
                Ok(m) => serde_json::from_str::<HashMap<String, Value>>(&String::from_utf8(m.to_vec()).unwrap_or_default())?,
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Error getting minions: {err}"))),
            };
            ms.push(EventMinion { mid: util::dataconv::as_str(traits.get("system.id").cloned()), cycles_id: Some(sid.to_string()), traits });
        }
        Ok(ms)
    }

    pub(crate) fn get_events(&self, sid: &str, mid: &str) -> Result<Vec<EventData>, SysinspectError> {
        let tid = Self::to_tree_id(sid, mid);
        let mut es = Vec::<EventData>::new();
        let events = self.get_tree(&tid)?;
        for evt in events.iter().values() {
            let payload = match evt {
                Ok(m) => serde_json::from_str::<HashMap<String, Value>>(&String::from_utf8(m.to_vec()).unwrap_or_default())?,
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Error getting minion events: {err}"))),
            };

            es.push(EventData::new(payload));
        }

        Ok(es)
    }

    /// Trim the database data to the specific amount of records.
    pub fn trim_data(&self) -> Result<(), SysinspectError> {
        log::debug!("Trimming database data. Age: {:?}, Limit: {}", self.cfg.age(), self.cfg.limit());
        let sessions = self.get_sessions()?;
        let age = if let Ok(d) = chrono::Duration::from_std(self.cfg.age()) { Utc::now().checked_sub_signed(d) } else { None };
        let tree = self.get_tree(TR_SESSIONS)?;

        // Delete sessions exceeding the limit
        if sessions.len() > self.cfg.limit() {
            for session in &sessions[..sessions.len() - self.cfg.limit()] {
                log::debug!("Deleting session {} due to exceeding limit", session.sid());
                tree.remove(session.sid())?;
            }
        }

        // Delete sessions older than the age limit
        if let Some(age) = age {
            for session in &sessions {
                if session.ts < age {
                    log::debug!("Deleting session {} due to age limit", session.sid());
                    tree.remove(session.sid())?;
                }
            }
        }

        Ok(())
    }

    /// Delete everything from the database (flush it out completely).
    pub(crate) fn purge_all_data(&self) -> Result<(), SysinspectError> {
        log::info!("Purging all data from the database");
        let mut c = 0;
        for tree in self.conn.tree_names() {
            let name = std::str::from_utf8(tree.as_ref()).unwrap_or("<invalid utf8>");

            // skip internal trees, they aren't purgeable anyway even if hit
            if !name.starts_with("__sled__") {
                if let Err(err) = self.conn.drop_tree(tree.clone()) {
                    log::error!("Error purging tree {}: {}", name, err);
                } else {
                    log::debug!("Purged tree {}", name);
                    c += 1;
                }
            }
        }
        log::info!("Purged {} trees", c);

        Ok(())
    }
}
