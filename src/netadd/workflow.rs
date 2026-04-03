use crate::netadd::{
    artifact::{MinionArtifact, MinionCatalogue, PlatformId},
    parser::{parse_request, resolve_plan},
    render::render_outcomes,
    types::{AddOutcome, AddPlan, AddRequest},
};
use crate::sshprobe::detect::{ProbeInfo, SSHPlatformDetector};
use clap::ArgMatches;
use libcommon::SysinspectError;
use std::path::Path;

/// Dedicated host-onboarding workflow entrypoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NetworkAddWorkflow {
    req: AddRequest,
}

impl NetworkAddWorkflow {
    /// Build one workflow from CLI matches.
    pub(crate) fn from_matches(am: &ArgMatches) -> Result<Self, SysinspectError> {
        Ok(Self { req: parse_request(am)? })
    }

    /// Validate and resolve the host batch.
    pub(crate) fn plan(&self) -> Result<AddPlan, SysinspectError> {
        resolve_plan(&self.req)
    }

    /// Produce the current operator-facing console view.
    pub(crate) fn render(&self) -> Result<String, SysinspectError> {
        Ok(render_outcomes(
            &self.plan()?.items.into_iter().map(|host| AddOutcome { detail: "validated".to_string(), host, state: "planned" }).collect::<Vec<_>>(),
        ))
    }

    /// Probe every planned host and render the discovered target details.
    pub(crate) fn probe_render(&self) -> Result<String, SysinspectError> {
        let mut rows = Vec::new();
        for host in self.plan()?.items {
            let mut det = SSHPlatformDetector::new(&host.host).set_user(&host.user).check_writable(true);
            if let Some(path) = &host.path {
                det = det.set_destination(path);
            }
            match det.info() {
                Ok(info) => rows.push(AddOutcome { detail: info.summary(), host, state: "probed" }),
                Err(err) => rows.push(AddOutcome { detail: err.to_string(), host, state: "error" }),
            }
        }
        Ok(render_outcomes(&rows))
    }

    /// Select one local sysminion artefact for a probed target.
    pub(crate) fn select_artifact(&self, repo_root: &Path, info: &ProbeInfo) -> Result<MinionArtifact, SysinspectError> {
        MinionCatalogue::open(repo_root)?.select(&PlatformId::from_probe(info)?)
    }
}
