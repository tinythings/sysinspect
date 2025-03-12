use chrono::{DateTime, Utc};
use colored::Colorize;
use fs_extra::dir::{CopyOptions, copy};
use libsysinspect::{
    SysinspectError,
    util::{self},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sled::{Db, Tree};
use std::{collections::HashMap, fs, path::PathBuf};
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

    pub fn from_bytes(b: Vec<u8>) -> Result<Self, SysinspectError> {
        match String::from_utf8(b) {
            Ok(data) => Ok(serde_json::from_str::<Self>(&data)?),
            Err(err) => Err(SysinspectError::MasterGeneralError(format!("Unable to recover event minion: {err}"))),
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
}

impl Default for EventsRegistry {
    fn default() -> Self {
        Self { conn: sled::Config::new().temporary(true).open().unwrap(), cloned: None } // open in memory
    }
}

impl EventsRegistry {
    pub fn new(p: PathBuf) -> Result<EventsRegistry, SysinspectError> {
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
    pub fn add_event(
        &mut self, sid: &EventSession, mid: EventMinion, payload: HashMap<String, Value>,
    ) -> Result<(), SysinspectError> {
        let events = self.get_tree(&Self::to_tree_id(sid.sid(), &mid.id()))?;
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
            log::info!("Session {} for {} registered", sid.yellow(), model.bright_yellow());
            return Ok(es);
        } else if let Some(sb) = sessions.get(&sid)? {
            log::debug!("Returning an existing session: {sid}");
            return EventSession::from_bytes(sb.to_vec());
        }

        Err(SysinspectError::MasterGeneralError("Session not found".to_string()))
    }

    /// Ensure that the minion data is there.
    pub fn ensure_minion(
        &mut self, sid: &EventSession, mid: String, traits: HashMap<String, Value>,
    ) -> Result<String, SysinspectError> {
        let session_minions = self.get_tree(sid.sid())?;
        if !session_minions.contains_key(&mid)? {
            log::debug!("Ensuring minion: {mid}");
            session_minions.insert(&mid, serde_json::to_string(&traits)?.as_bytes())?;
        } else {
            log::debug!("Minion already in the database {mid}");
        }

        Ok(mid)
    }

    /// Return existing recorded sessions
    pub fn get_sessions(&self) -> Result<Vec<EventSession>, SysinspectError> {
        let mut ks = Vec::<EventSession>::new();
        let sessions = self.get_tree(TR_SESSIONS)?;
        for v in sessions.iter().values() {
            let v = match v {
                Ok(v) => String::from_utf8(v.to_vec()).unwrap_or_default(),
                Err(err) => return Err(SysinspectError::MasterGeneralError(format!("Error getting sessions: {err}"))),
            };
            ks.push(EventSession::from_bytes(v.as_bytes().to_vec())?);
        }

        Ok(ks)
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
            ms.push(EventMinion {
                mid: util::dataconv::as_str(traits.get("system.id").cloned()),
                cycles_id: Some(sid.to_string()),
                traits,
            });
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
}
