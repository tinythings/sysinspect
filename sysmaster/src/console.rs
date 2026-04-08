//! Console-specific request handling for `SysMaster`.
//!
//! This module owns the encrypted local console listener, request decoding,
//! console-command dispatch, and the typed response builders used by the CLI.
//! It deliberately stops at typed payloads and outbound `MasterMessage`
//! construction; any human-facing formatting remains on the client side.

use super::*;

use crate::hopstart::{HopStartTarget, HopStarter};
use libmodpak::{SysInspectModPak, compare_versions};
use libsysinspect::{
    console::{
        ConsoleEnvelope, ConsoleMinionInfoRow, ConsoleOnlineMinionRow, ConsolePayload, ConsoleQuery, ConsoleResponse, ConsoleSealed,
        ConsoleTransportStatusRow, authorised_console_client, load_master_private_key,
    },
    context::get_context,
    traits::TraitSource,
};
use tokio::net::{TcpStream, tcp::OwnedReadHalf};
use tokio::time;

/// Maximum single-line console request size accepted from the local TCP console.
///
/// Requests are newline-delimited JSON frames, so the limit is applied before
/// parsing and before any cryptographic work is attempted.
const MAX_CONSOLE_FRAME_SIZE: usize = 64 * 1024;

/// Upper bound for reading one console request frame from a connected client.
///
/// This prevents a local client from holding a console socket open forever
/// without completing a request.
const CONSOLE_READ_TIMEOUT: StdDuration = StdDuration::from_secs(5);

/// Result returned by console helpers that both answer the caller and stage
/// follow-up cluster messages that still need to be broadcast.
type ConsoleOutcome = (ConsoleResponse, Vec<MasterMessage>);

/// Parsed selector flags for `cluster/transport/status` console requests.
///
/// The filter is optional because older or minimal clients may omit it, in
/// which case the request defaults to returning every selected minion.
#[derive(Debug, Clone, Deserialize)]
struct TransportStatusConsoleRequest {
    filter: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CmdbStartupConsoleRequest {
    user: String,
    host: String,
    root: String,
    bin: String,
    path: String,
    backend: String,
}

impl CmdbStartupConsoleRequest {
    fn from_context(context: &str) -> Result<Self, SysinspectError> {
        if context.trim().is_empty() {
            return Err(SysinspectError::InvalidQuery("CMDB update requires startup inventory context".to_string()));
        }

        let request = serde_json::from_str::<Self>(context)
            .map_err(|err| SysinspectError::DeserializationError(format!("Failed to parse CMDB request context: {err}")))?;

        for (name, value) in [
            ("user", request.user.as_str()),
            ("host", request.host.as_str()),
            ("root", request.root.as_str()),
            ("bin", request.bin.as_str()),
            ("path", request.path.as_str()),
            ("backend", request.backend.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(SysinspectError::InvalidQuery(format!("CMDB field {name} must not be empty")));
            }
        }

        Ok(request)
    }

    fn into_startup(self) -> crate::registry::rec::MinionCmdbStartup {
        crate::registry::rec::MinionCmdbStartup::new(self.user, self.host, self.root, self.bin, self.path, self.backend)
    }
}

impl TransportStatusConsoleRequest {
    /// Parse the JSON request context for a transport-status console command.
    ///
    /// An empty context is treated as `all` to keep the server-side behavior
    /// predictable for callers that do not send explicit filters.
    fn from_context(context: &str) -> Result<Self, SysinspectError> {
        if context.trim().is_empty() {
            return Ok(Self { filter: Some("all".to_string()) });
        }

        serde_json::from_str(context)
            .map_err(|err| SysinspectError::DeserializationError(format!("Failed to parse transport status request context: {err}")))
    }

    /// Decide whether one transport-status row should be included in the reply.
    ///
    /// The filter is interpreted against the current rotation state already
    /// loaded for the minion. Missing state is only included when the filter is
    /// `all`.
    fn include_row(&self, rotation: Option<&libsysinspect::transport::TransportRotationStatus>) -> bool {
        match self.filter.as_deref().unwrap_or("all") {
            "pending" => rotation.is_some_and(|status| *status != libsysinspect::transport::TransportRotationStatus::Idle),
            "idle" => rotation.is_some_and(|status| *status == libsysinspect::transport::TransportRotationStatus::Idle),
            _ => true,
        }
    }
}

/// Count of immediate versus deferred rotation requests produced by one console
/// rotate operation.
#[derive(Debug, Default)]
struct RotationDispatchSummary {
    online_count: usize,
    queued_count: usize,
}

impl RotationDispatchSummary {
    /// Record that one selected minion received an immediate rotation message.
    fn note_online_dispatch(&mut self) {
        self.online_count += 1;
    }

    /// Record that one selected minion was offline and had rotation staged for later replay.
    fn note_queued_dispatch(&mut self) {
        self.queued_count += 1;
    }

    /// Convert the accumulated counters into the typed console payload expected by the CLI.
    fn response(&self) -> ConsoleResponse {
        ConsoleResponse::ok(ConsolePayload::RotationSummary { online_count: self.online_count, queued_count: self.queued_count })
    }
}

impl SysMaster {
    /// Serialize a console response into a plain JSON line for direct socket writes.
    ///
    /// This helper is used for pre-encryption validation errors and other cases
    /// where the master must still answer the local client without building a
    /// sealed response frame.
    fn console_response_json(response: &ConsoleResponse) -> Option<String> {
        serde_json::to_string(response).ok()
    }

    /// Build a JSON-encoded error reply for local console failures.
    ///
    /// Returning `Option<String>` keeps the helper symmetric with
    /// `console_response_json` and lets callers propagate a fully formed line or
    /// drop the response if serialization unexpectedly fails.
    fn console_error_json(error: impl Into<String>) -> Option<String> {
        Self::console_response_json(&ConsoleResponse::err(error))
    }

    /// Build raw online-minion rows for the console `network --online` query.
    ///
    /// The returned rows only contain typed data assembled from registry traits
    /// and the current session liveness table. No presentation formatting is
    /// applied here.
    async fn online_minions_data(&mut self, query: &str, traits: &str, mid: &str) -> Result<Vec<ConsoleOnlineMinionRow>, SysinspectError> {
        Ok({
            let repo_versions = SysInspectModPak::new(self.cfg.get_mod_repo_root())
                .ok()
                .map(|repo| {
                    repo.minion_builds().into_iter().fold(std::collections::BTreeMap::new(), |mut rows, row| {
                        rows.insert((row.platform().to_string(), row.arch().to_string()), row.version().to_string());
                        rows
                    })
                })
                .unwrap_or_default();
            let selected = self.selected_minions(query, traits, mid).await?;
            let mut session = self.session.lock().await;
            {
                let mut rows = Vec::with_capacity(selected.len());
                for minion in selected {
                    let cmdb = self.mreg.lock().await.get_cmdb(minion.id()).unwrap_or_default();
                    let (fqdn, hostname, ip) = Self::preferred_host(&minion, cmdb.as_ref());
                    let current_version = minion.get_traits().get("minion.version").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let target_version = repo_versions
                        .get(&(
                            minion.get_traits().get("system.os.distribution").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            minion.get_traits().get("system.arch").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        ))
                        .cloned()
                        .unwrap_or_default();
                    rows.push(ConsoleOnlineMinionRow {
                        fqdn,
                        hostname,
                        ip,
                        minion_id: minion.id().to_string(),
                        alive: session.alive(minion.id()),
                        outdated: !current_version.is_empty()
                            && !target_version.is_empty()
                            && compare_versions(&current_version, &target_version).is_lt(),
                        version: current_version,
                        target_version,
                    });
                }
                rows
            }
        })
    }

    /// Build the full trait-backed info payload for exactly one selected minion.
    ///
    /// The function enforces the single-target requirement server-side so the
    /// console protocol never returns an ambiguous multi-minion info payload.
    /// The response includes synthetic `minion.id` and `minion.online` rows in
    /// addition to the stored traits.
    async fn minion_info_rows(&mut self, query: &str, traits: &str, mid: &str) -> Result<Vec<ConsoleMinionInfoRow>, SysinspectError> {
        let targets = self.selected_minions(query, traits, mid).await?;
        if targets.is_empty() {
            return Err(SysinspectError::InvalidQuery("Minion info requires one matching minion, but none were found".to_string()));
        }
        if targets.len() > 1 {
            return Err(SysinspectError::InvalidQuery(format!(
                "Minion info requires exactly one matching minion, but {} were selected",
                targets.len()
            )));
        }
        let mut session = self.session.lock().await;
        let minion = targets.into_iter().next().expect("validated exactly one selected minion");
        let minion_id = minion.id().to_string();
        let mut rows = vec![
            ConsoleMinionInfoRow { key: "minion.id".to_string(), value: serde_json::Value::String(minion_id.clone()), source: TraitSource::Preset },
            ConsoleMinionInfoRow {
                key: "minion.online".to_string(),
                value: serde_json::Value::Bool(session.alive(&minion_id)),
                source: TraitSource::Preset,
            },
        ];

        rows.extend(minion.get_traits().iter().map(|(name, value)| ConsoleMinionInfoRow {
            key: name.clone(),
            value: value.clone(),
            source: if minion.is_function_trait(name) {
                TraitSource::Function
            } else if minion.is_yaml_trait(name) {
                TraitSource::Static
            } else {
                TraitSource::Preset
            },
        }));

        Ok(rows)
    }

    /// Remove a minion from registry and key storage and prepare the matching console reply.
    ///
    /// When a command message can still be constructed for the target minion it
    /// is returned alongside the response so the caller can broadcast the final
    /// remove command over the cluster transport.
    async fn unregister_console_response(&mut self, mid: &str) -> Result<ConsoleOutcome, SysinspectError> {
        if mid.trim().is_empty() {
            return Ok((ConsoleResponse::err("Unregister requires a minion id"), vec![]));
        }

        let targets = self.selected_minions("", "", mid).await?;
        if targets.is_empty() {
            return Err(SysinspectError::MasterGeneralError(format!("Unable to find minion {mid}")));
        }
        if targets.len() > 1 {
            return Err(SysinspectError::MasterGeneralError(format!(
                "Unregister requires exactly one matching minion, but {} were selected",
                targets.len()
            )));
        }
        let target = targets.into_iter().next().expect("validated exactly one selected minion");
        let msg = self.msg_query_data(&format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}"), "", "", target.id(), "").await;

        log::info!("Removing minion {}", target.id());
        if let Err(err) = self.mreg.lock().await.remove(target.id()) {
            return Err(SysinspectError::MasterGeneralError(format!("Unable to remove minion {}: {err}", target.id())));
        }
        if let Err(err) = self.mkr().remove_mn_key(target.id()) {
            return Err(SysinspectError::MasterGeneralError(format!("Unable to unregister minion {}: {err}", target.id())));
        }

        Ok((
            ConsoleResponse::ok(ConsolePayload::Ack {
                action: "remove_minion".to_string(),
                target: target.id().to_string(),
                count: 1,
                items: vec![],
            }),
            msg.into_iter().collect(),
        ))
    }

    async fn upsert_cmdb_console_response(&mut self, mid: &str, context: &str) -> Result<ConsoleResponse, SysinspectError> {
        if mid.trim().is_empty() {
            return Ok(ConsoleResponse::err("CMDB update requires a minion id"));
        }
        if !self.mkr().is_registered(mid) {
            return Err(SysinspectError::MasterGeneralError(format!("Unable to find registered minion {mid} for CMDB update")));
        }

        let startup = CmdbStartupConsoleRequest::from_context(context)?.into_startup();
        self.mreg.lock().await.upsert_cmdb_startup(mid, startup)?;

        Ok(ConsoleResponse::ok(ConsolePayload::Ack { action: "cmdb_upsert".to_string(), target: mid.to_string(), count: 1, items: vec![] }))
    }

    async fn hopstart_console_response(&mut self, query: &str, traits: &str, mid: &str) -> Result<ConsoleResponse, SysinspectError> {
        let mut targets = Vec::new();
        let selected = self.selected_minions(query, traits, mid).await?;
        let mut session = self.session.lock().await;

        for minion in selected {
            if session.alive(minion.id()) {
                continue;
            }
            if let Ok(Some(cmdb)) = self.mreg.lock().await.get_cmdb(minion.id()) {
                if cmdb.backend() != Some("hopstart") {
                    continue;
                }
                if let (Some(host), Some(root), Some(user), Some(bin), Some(config)) =
                    (cmdb.host(), cmdb.root(), cmdb.user(), cmdb.bin(), cmdb.config())
                {
                    targets.push(HopStartTarget::new(host.to_string(), root.to_string(), user.to_string(), bin.to_string(), config.to_string()));
                } else {
                    log::error!("Hop-start skipped for {}: incomplete CMDB startup inventory", minion.id());
                }
            }
        }

        HopStarter::new(self.cfg.hopstart()).issue(targets.clone()).await;

        Ok(ConsoleResponse::ok(ConsolePayload::Ack {
            action: "hopstart_issued".to_string(),
            target: String::new(),
            count: targets.len(),
            items: vec![],
        }))
    }

    /// Register the concrete minion ids targeted by one outbound console message.
    ///
    /// This keeps task tracking aligned with console-initiated broadcasts so the
    /// rest of the master can observe completion state the same way it does for
    /// normal queued work.
    async fn register_broadcast_targets(master: Arc<Mutex<Self>>, msg: &MasterMessage) {
        let guard = master.lock().await;
        let ids = guard.mreg.lock().await.get_targeted_minions(msg.target(), false).await;
        guard.taskreg.lock().await.register(msg.cycle(), ids);
    }

    /// Broadcast the `MasterMessage`s produced by one console request.
    ///
    /// Some console commands, such as unregister, should not populate task
    /// tracking, so the caller controls whether target registration is performed
    /// for the dispatched messages.
    async fn broadcast_console_messages(
        master: Arc<Mutex<Self>>, bcast: &broadcast::Sender<MasterMessage>, cfg: &MasterConfig, msgs: Vec<MasterMessage>, register_targets: bool,
    ) {
        for msg in msgs {
            Self::bcast_master_msg(bcast, cfg.telemetry_enabled(), Arc::clone(&master), Some(msg.clone())).await;
            if register_targets {
                Self::register_broadcast_targets(Arc::clone(&master), &msg).await;
            }
        }
    }

    /// Execute one decrypted console query and convert it into a typed console response.
    ///
    /// This is the central router for console-only commands. It handles typed
    /// data requests directly and delegates cluster-affecting operations to the
    /// existing `SysMaster` helpers that already know how to stage, persist, and
    /// build outbound master messages.
    async fn dispatch_console_query(
        master: Arc<Mutex<Self>>, bcast: &broadcast::Sender<MasterMessage>, cfg: &MasterConfig, query: ConsoleQuery,
    ) -> ConsoleResponse {
        if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_ONLINE_MINIONS}")) {
            return match master.lock().await.online_minions_data(&query.query, &query.traits, &query.mid).await {
                Ok(rows) => ConsoleResponse::ok(ConsolePayload::OnlineMinions { rows }),
                Err(err) => ConsoleResponse::err(format!("Unable to get online minions: {err}")),
            };
        }

        if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_MINION_INFO}")) {
            return match master.lock().await.minion_info_rows(&query.query, &query.traits, &query.mid).await {
                Ok(rows) => ConsoleResponse::ok(ConsolePayload::MinionInfo { rows }),
                Err(err) => ConsoleResponse::err(format!("Unable to get minion info: {err}")),
            };
        }

        if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_TRANSPORT_STATUS}")) {
            return match TransportStatusConsoleRequest::from_context(&query.context) {
                Ok(request) => match master.lock().await.transport_status_data(&request, &query.query, &query.traits, &query.mid).await {
                    Ok(rows) => ConsoleResponse::ok(ConsolePayload::TransportStatus { rows }),
                    Err(err) => ConsoleResponse::err(format!("Unable to get transport status: {err}")),
                },
                Err(err) => ConsoleResponse::err(format!("Failed to parse transport status request: {err}")),
            };
        }

        if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_ROTATE}")) {
            let (response, msgs) = match RotationConsoleRequest::from_context(&query.context) {
                Ok(request) => {
                    let mut guard = master.lock().await;
                    match guard.rotate_console_response(&request, &query.query, &query.traits, &query.mid).await {
                        Ok(data) => data,
                        Err(err) => (ConsoleResponse::err(err.to_string()), vec![]),
                    }
                }
                Err(err) => (ConsoleResponse::err(format!("Failed to parse rotate request: {err}")), vec![]),
            };
            Self::broadcast_console_messages(Arc::clone(&master), bcast, cfg, msgs, true).await;
            return response;
        }

        if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_HOPSTART}")) {
            return match master.lock().await.hopstart_console_response(&query.query, "", &query.mid).await {
                Ok(response) => response,
                Err(err) => ConsoleResponse::err(err.to_string()),
            };
        }

        if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_REMOVE_MINION}")) {
            let (response, msgs) = {
                let mut guard = master.lock().await;
                match guard.unregister_console_response(&query.mid).await {
                    Ok(data) => data,
                    Err(err) => (ConsoleResponse::err(err.to_string()), vec![]),
                }
            };
            Self::broadcast_console_messages(Arc::clone(&master), bcast, cfg, msgs, false).await;
            return response;
        }

        if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_CMDB_UPSERT}")) {
            return match master.lock().await.upsert_cmdb_console_response(&query.mid, &query.context).await {
                Ok(response) => response,
                Err(err) => ConsoleResponse::err(format!("Unable to upsert CMDB: {err}")),
            };
        }

        if query.model.eq(&format!("{SCHEME_COMMAND}{CLUSTER_PROFILE}")) {
            let (response, msgs) = match ProfileConsoleRequest::from_context(&query.context) {
                Ok(request) => {
                    let mut guard = master.lock().await;
                    match guard.do_profile_console(&request, &query.query, &query.traits, &query.mid).await {
                        Ok(data) => data,
                        Err(err) => (ConsoleResponse::err(err.to_string()), vec![]),
                    }
                }
                Err(err) => (ConsoleResponse::err(format!("Failed to parse profile request: {err}")), vec![]),
            };
            Self::broadcast_console_messages(Arc::clone(&master), bcast, cfg, msgs, true).await;
            return response;
        }

        let msg = {
            let mut guard = master.lock().await;
            guard.msg_query_data(&query.model, &query.query, &query.traits, &query.mid, &query.context).await
        };
        if let Some(msg) = msg {
            Self::broadcast_console_messages(Arc::clone(&master), bcast, cfg, vec![msg], true).await;
            return ConsoleResponse {
                ok: true,
                error: String::new(),
                payload: ConsolePayload::Ack { action: "accepted_console_command".to_string(), target: query.model, count: 0, items: vec![] },
            };
        }

        ConsoleResponse::err("No message constructed for the console query")
    }

    /// Read exactly one newline-terminated request frame from a console socket.
    ///
    /// The function applies both a time limit and a size limit before any JSON
    /// parsing occurs, and converts transport-level failures into plain JSON
    /// error replies that can be sent back on the same connection.
    async fn read_console_request(read_half: OwnedReadHalf) -> Option<String> {
        let reader = TokioBufReader::new(read_half);
        let mut frame = Vec::new();
        let mut reader = reader.take((MAX_CONSOLE_FRAME_SIZE + 1) as u64);
        match time::timeout(CONSOLE_READ_TIMEOUT, reader.read_until(b'\n', &mut frame)).await {
            Err(_) => Self::console_error_json(format!("Console request timed out after {} seconds", CONSOLE_READ_TIMEOUT.as_secs())),
            Ok(Ok(0)) => Self::console_error_json("Empty console request"),
            Ok(Ok(_)) if frame.len() > MAX_CONSOLE_FRAME_SIZE || !frame.ends_with(b"\n") => {
                Self::console_error_json(format!("Console request exceeds {} bytes", MAX_CONSOLE_FRAME_SIZE))
            }
            Ok(Ok(_)) => String::from_utf8(frame)
                .map(|line| line.trim().to_string())
                .map_err(|err| format!("Console request is not valid UTF-8: {err}"))
                .map_or_else(Self::console_error_json, Some),
            Ok(Err(err)) => Self::console_error_json(format!("Failed to read console request: {err}")),
        }
    }

    /// Validate, decrypt, dispatch, and reseal one console request envelope.
    ///
    /// The caller supplies the already-loaded master private key and broadcast
    /// handle so this function can stay focused on the request lifecycle:
    /// deserialize envelope, verify authorisation, derive the session key,
    /// decrypt the query, dispatch it, and seal the response.
    async fn process_console_envelope(
        master: Arc<Mutex<Self>>, cfg: &MasterConfig, bcast: &broadcast::Sender<MasterMessage>, master_prk: &rsa::RsaPrivateKey, line: &str,
    ) -> Option<String> {
        let envelope = match serde_json::from_str::<ConsoleEnvelope>(line) {
            Ok(envelope) => envelope,
            Err(err) => return Self::console_error_json(format!("Failed to parse console request: {err}")),
        };

        if !authorised_console_client(cfg, &envelope.bootstrap.client_pubkey).unwrap_or(false) {
            return Self::console_error_json("Console client key is not authorised");
        }

        let (key, _client_pkey) = match envelope.bootstrap.session_key(master_prk) {
            Ok(data) => data,
            Err(err) => return Self::console_error_json(format!("Console bootstrap failed: {err}")),
        };

        let query = match envelope.sealed.open::<ConsoleQuery>(&key) {
            Ok(query) => query,
            Err(err) => return Self::console_error_json(format!("Failed to open console query: {err}")),
        };

        let response = Self::dispatch_console_query(master, bcast, cfg, query).await;
        match ConsoleSealed::seal(&response, &key)
            .and_then(|sealed| serde_json::to_string(&sealed).map_err(|e| SysinspectError::SerializationError(e.to_string())))
        {
            Ok(reply) => Some(reply),
            Err(err) => {
                log::error!("Failed to seal console response: {err}");
                Self::console_error_json(format!("Failed to seal console response: {err}"))
            }
        }
    }

    /// Serve one accepted TCP console connection from initial read to final reply write.
    ///
    /// Non-JSON input is treated as a prebuilt plain response payload. JSON
    /// input is handled as an encrypted console envelope and routed through the
    /// authenticated console flow.
    async fn handle_console_stream(
        master: Arc<Mutex<Self>>, cfg: MasterConfig, bcast: broadcast::Sender<MasterMessage>, master_prk: rsa::RsaPrivateKey, stream: TcpStream,
    ) {
        let (read_half, mut write_half) = stream.into_split();
        let reply = match Self::read_console_request(read_half).await {
            Some(line) if !line.trim_start().starts_with('{') => Some(line),
            Some(line) => Self::process_console_envelope(master, &cfg, &bcast, &master_prk, &line).await,
            None => None,
        };

        if let Some(reply) = reply
            && let Err(err) = write_half.write_all(format!("{reply}\n").as_bytes()).await
        {
            log::error!("Failed to write console response: {err}");
        }
    }

    /// Resolve the minions targeted by a console command.
    ///
    /// Selection priority is explicit minion id, then hostname or IP fallback,
    /// then general query lookup. If no explicit id is given, trait selectors
    /// take precedence over the free-form query string. The resulting records are
    /// always sorted by minion id to keep console output stable.
    async fn selected_minions(&mut self, query: &str, traits: &str, mid: &str) -> Result<Vec<crate::registry::rec::MinionRecord>, SysinspectError> {
        let mut records = if !mid.is_empty() {
            let mut registry = self.mreg.lock().await;
            let mut records = registry.get(mid)?.into_iter().collect::<Vec<_>>();
            if records.is_empty() {
                records = registry.get_by_hostname_or_ip(mid)?;
            }
            if records.is_empty() {
                records = registry.get_by_query(mid)?;
            }
            if records.is_empty() {
                records = registry
                    .get_registered_ids()?
                    .into_iter()
                    .filter(|id| id.starts_with(mid))
                    .filter_map(|id| registry.get(&id).ok().flatten())
                    .collect();
            }
            records
        } else if !traits.trim().is_empty() {
            let traits = get_context(traits)
                .ok_or_else(|| SysinspectError::InvalidQuery("Traits selector must be in key:value format".to_string()))?
                .into_iter()
                .collect::<HashMap<_, _>>();
            self.mreg.lock().await.get_by_traits(traits)?
        } else {
            self.mreg.lock().await.get_by_query(if query.trim().is_empty() { "*" } else { query })?
        };
        records.sort_by(|a, b| a.id().cmp(b.id()));
        Ok(records)
    }

    /// Require a profile name for profile operations that operate on a named profile object.
    fn require_profile_name(request: &ProfileConsoleRequest) -> Result<(), SysinspectError> {
        if !request.name().trim().is_empty() {
            return Ok(());
        }

        Err(SysinspectError::InvalidQuery("Profile name cannot be empty".to_string()))
    }

    /// Extract the currently assigned non-default profiles from one minion record.
    ///
    /// Profile metadata may be stored as either a scalar string or an array of
    /// strings in traits. The default profile is intentionally filtered out so
    /// console tag operations only manage explicit operator assignments.
    fn current_profiles(minion: &crate::registry::rec::MinionRecord) -> Vec<String> {
        let mut profiles = match minion.get_traits().get("minion.profile") {
            Some(serde_json::Value::String(name)) if !name.trim().is_empty() => vec![name.to_string()],
            Some(serde_json::Value::Array(names)) => names.iter().filter_map(|name| name.as_str().map(str::to_string)).collect::<Vec<_>>(),
            _ => vec![],
        };
        profiles.retain(|profile| profile != "default");
        profiles
    }

    /// Apply or remove profile tags on the selected minions and build the reply.
    ///
    /// Before emitting any update messages, the function validates that all
    /// requested profiles exist in the module repository so partial application
    /// cannot occur.
    async fn profile_tag_console_response(
        &mut self, request: &ProfileConsoleRequest, query: &str, traits: &str, mid: &str,
    ) -> Result<ConsoleOutcome, SysinspectError> {
        let repo = SysInspectModPak::new(self.cfg.get_mod_repo_root())?;
        let known_profiles = repo.list_profiles(None)?;
        let missing = request.profiles().iter().filter(|name| !known_profiles.contains(name)).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Ok((
                ConsoleResponse {
                    ok: false,
                    error: format!("Unknown profile{}: {}", if missing.len() == 1 { "" } else { "s" }, missing.join(", ")),
                    payload: ConsolePayload::Empty,
                },
                vec![],
            ));
        }

        let mut msgs = Vec::new();
        for minion in self.selected_minions(query, traits, mid).await? {
            let mut profiles = Self::current_profiles(&minion);
            if request.op() == "tag" {
                for profile in request.profiles() {
                    if !profiles.contains(profile) {
                        profiles.push(profile.to_string());
                    }
                }
            } else {
                profiles.retain(|profile| !request.profiles().contains(profile));
            }

            let context = if profiles.is_empty() {
                json!({"op": "unset", "traits": {"minion.profile": null}})
            } else {
                json!({"op": "set", "traits": {"minion.profile": profiles}})
            }
            .to_string();

            if let Some(msg) = self.msg_query_data(&format!("{SCHEME_COMMAND}{CLUSTER_TRAITS_UPDATE}"), "", "", minion.id(), &context).await {
                msgs.push(msg);
            }
        }

        Ok((
            ConsoleResponse::ok(ConsolePayload::Ack {
                action: if request.op() == "tag" { "apply_profiles".to_string() } else { "remove_profiles".to_string() },
                target: String::new(),
                count: msgs.len(),
                items: request.profiles().to_vec(),
            }),
            msgs,
        ))
    }

    /// Execute one profile-related console command.
    ///
    /// Pure repository operations return only a typed response, while tag and
    /// untag operations also return the broadcast messages that will push trait
    /// changes to the selected minions.
    async fn do_profile_console(
        &mut self, request: &ProfileConsoleRequest, query: &str, traits: &str, mid: &str,
    ) -> Result<ConsoleOutcome, SysinspectError> {
        let repo = SysInspectModPak::new(self.cfg.get_mod_repo_root())?;

        match request.op() {
            "new" => Ok((
                {
                    Self::require_profile_name(request)?;
                    repo.new_profile(request.name())?;
                    ConsoleResponse::ok(ConsolePayload::Ack {
                        action: "create_profile".to_string(),
                        target: request.name().to_string(),
                        count: 0,
                        items: vec![],
                    })
                },
                vec![],
            )),
            "delete" => Ok((
                {
                    Self::require_profile_name(request)?;
                    repo.delete_profile(request.name())?;
                    ConsoleResponse::ok(ConsolePayload::Ack {
                        action: "delete_profile".to_string(),
                        target: request.name().to_string(),
                        count: 0,
                        items: vec![],
                    })
                },
                vec![],
            )),
            "list" => Ok((
                ConsoleResponse::ok(ConsolePayload::StringList {
                    items: if request.name().is_empty() {
                        repo.list_profiles(None)?
                    } else {
                        repo.list_profile_matches(Some(request.name()), request.library())?
                    },
                }),
                vec![],
            )),
            "show" => Ok((
                {
                    Self::require_profile_name(request)?;
                    ConsoleResponse::ok(ConsolePayload::Text { value: repo.show_profile(request.name())? })
                },
                vec![],
            )),
            "add" => Ok((
                {
                    Self::require_profile_name(request)?;
                    repo.add_profile_matches(request.name(), request.matches().to_vec(), request.library())?;
                    ConsoleResponse::ok(ConsolePayload::Ack {
                        action: "update_profile".to_string(),
                        target: request.name().to_string(),
                        count: 0,
                        items: vec![],
                    })
                },
                vec![],
            )),
            "remove" => Ok((
                {
                    Self::require_profile_name(request)?;
                    repo.remove_profile_matches(request.name(), request.matches().to_vec(), request.library())?;
                    ConsoleResponse::ok(ConsolePayload::Ack {
                        action: "update_profile".to_string(),
                        target: request.name().to_string(),
                        count: 0,
                        items: vec![],
                    })
                },
                vec![],
            )),
            "tag" | "untag" => self.profile_tag_console_response(request, query, traits, mid).await,
            _ => Ok((ConsoleResponse::err(format!("Unsupported profile operation {}", request.op())), vec![])),
        }
    }

    /// Execute a console-driven transport rotation request.
    ///
    /// Online minions receive immediate messages. Offline minions have the exact
    /// serialized request persisted into transport state so it can be replayed on
    /// reconnect without the CLI needing to resubmit the operation.
    async fn rotate_console_response(
        &mut self, request: &RotationConsoleRequest, query: &str, traits: &str, mid: &str,
    ) -> Result<ConsoleOutcome, SysinspectError> {
        if request.op() != "rotate" {
            return Ok((ConsoleResponse::err(format!("Unsupported rotate operation {}", request.op())), vec![]));
        }

        let mut online_msgs = Vec::new();
        let mut summary = RotationDispatchSummary::default();

        let targets = self.selected_minions(query, traits, mid).await?;
        for minion in targets {
            let minion_id = minion.id().to_string();
            let online = self.session.lock().await.alive(&minion_id);
            if online {
                if let Some(msg) = self.build_rotation_message(&minion_id, request, None).await? {
                    online_msgs.push(msg);
                    summary.note_online_dispatch();
                }
            } else {
                let context = self.stage_rotation_context(&minion_id, request)?;
                self.persist_pending_rotation_context(&minion_id, Some(context))?;
                summary.note_queued_dispatch();
            }
        }

        Ok((summary.response(), online_msgs))
    }

    /// Build raw transport-status rows for the selected minions.
    ///
    /// Each row captures host identity plus the currently persisted transport
    /// state: active key id, handshake timestamp, derived last rotation time,
    /// and the current rotation status. Consumers decide how to render or sort
    /// the data.
    async fn transport_status_data(
        &mut self, request: &TransportStatusConsoleRequest, query: &str, traits: &str, mid: &str,
    ) -> Result<Vec<ConsoleTransportStatusRow>, SysinspectError> {
        let targets = self.selected_minions(query, traits, mid).await?;
        let mut rows = Vec::with_capacity(targets.len());

        for minion in targets {
            let minion_id = minion.id().to_string();
            let cmdb = self.mreg.lock().await.get_cmdb(minion.id()).unwrap_or_default();
            let (fqdn, hostname, _ip) = Self::preferred_host(&minion, cmdb.as_ref());
            let state = TransportStore::for_master_minion(&self.cfg, &minion_id)?.load()?;
            if let Some(state) = state {
                let last_rotated_at = state.active_key_id.as_ref().and_then(|active_key| {
                    state.keys.iter().find(|key| key.key_id == *active_key).map(|record| record.activated_at.unwrap_or(record.created_at))
                });
                let row = ConsoleTransportStatusRow {
                    fqdn,
                    hostname,
                    minion_id,
                    active_key_id: state.active_key_id.clone(),
                    last_handshake_at: state.last_handshake_at,
                    last_rotated_at,
                    rotation: Some(state.rotation),
                };
                if request.include_row(row.rotation.as_ref()) {
                    rows.push(row);
                }
            } else {
                let row = ConsoleTransportStatusRow {
                    fqdn,
                    hostname,
                    minion_id,
                    active_key_id: None,
                    last_handshake_at: None,
                    last_rotated_at: None,
                    rotation: None,
                };
                if request.include_row(row.rotation.as_ref()) {
                    rows.push(row);
                }
            }
        }

        Ok(rows)
    }

    /// Start the local encrypted console listener used by the `sysinspect` CLI.
    ///
    /// The listener task owns the bound socket and accepts new local TCP
    /// connections forever. Each accepted connection is handled in its own task
    /// so slow or failing clients do not block later console operations.
    pub async fn do_console(master: Arc<Mutex<Self>>) {
        log::trace!("Init local console channel");
        tokio::spawn({
            let master = Arc::clone(&master);
            async move {
                let (cfg, bcast) = {
                    let guard = master.lock().await;
                    (guard.cfg(), guard.broadcast().clone())
                };
                let master_prk = match load_master_private_key(&cfg) {
                    Ok(prk) => prk,
                    Err(err) => {
                        log::error!("Failed to load console private key: {err}");
                        return;
                    }
                };
                let listener = match TcpListener::bind(cfg.console_listen_addr()).await {
                    Ok(listener) => listener,
                    Err(err) => {
                        log::error!("Failed to bind console listener: {err}");
                        return;
                    }
                };
                loop {
                    match listener.accept().await {
                        Ok((stream, _peer)) => {
                            let master = Arc::clone(&master);
                            let cfg = cfg.clone();
                            let bcast = bcast.clone();
                            let master_prk = master_prk.clone();
                            tokio::spawn(async move {
                                SysMaster::handle_console_stream(master, cfg, bcast, master_prk, stream).await;
                            });
                        }
                        Err(err) => {
                            log::error!("Console listener accept error: {err}");
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        });
    }
}
