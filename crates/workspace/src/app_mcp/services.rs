//! What the tools work with: the same workspace, runners, servers and programs the screens use, so a
//! tool goes through the paths a person's action goes through, with the same rules and records.

use std::sync::Arc;

use orchester_core::{id::ResourceId, resource::ResourceBody, store::ResourceStore, time::Timestamp};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::error::{Missing, ToolError};
use crate::{
  activity::Activity,
  messages::Message,
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::{OpenProject, Workspace},
};

#[derive(Clone)]
pub struct Services {
  pub(super) workspace: Workspace,
  pub(super) runners: Runners,
  pub(super) servers: Servers,
  pub(super) programs: Programs,
  pub(super) vault: Vault,
  /// Told what each tool changed in which project, so only the screens showing it read it again.
  pub(super) changes: OutsideChanges,
  /// The status bar's operations, which list a tool's git work as they list a person's.
  pub(super) activity: Activity,
  /// The projects' merge and removal operations, shared with the task pages.
  pub(super) jobs: ProjectJobs,
  /// A person decided the publication the tools are called for (they applied a proposal of the Orchestrator chat):
  /// what it widens is theirs to decide, as in the window, not *Let automation change agent permissions*, but only
  /// for the draft they decided on and what they were shown it widens.
  pub(super) person: Option<Arc<PersonDecision>>,
}

/// What a person decided to publish: the draft saved at `saved`, as version `version`, widening what agents may do
/// by `widens` (what their card showed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonDecision {
  pub saved: Timestamp,
  pub version: u32,
  pub widens: Vec<Message>,
}

impl Services {
  pub fn new(
    workspace: Workspace,
    runners: Runners,
    servers: Servers,
    programs: Programs,
    (changes, activity, jobs): (OutsideChanges, Activity, ProjectJobs),
  ) -> Self {
    Self {
      workspace,
      runners,
      servers,
      programs,
      vault: Vault::keychain(),
      changes,
      activity,
      jobs,
      person: None,
    }
  }

  /// These services for a publication a person decided ([`Self::person`]). Only the window may ask for it, for a
  /// click it took from a person and refused from synthetic input.
  #[must_use]
  pub fn decided_by_person(mut self, decision: PersonDecision) -> Self {
    self.person = Some(Arc::new(decision));
    self
  }

  /// Refused unless no person decided the call, or the draft of `resource` is the one they decided on: saved when
  /// they saw it, over the version before theirs. A tool reads it after the body it publishes, which it publishes
  /// only while that body is unchanged, so a draft saved in between is not published as the person's decision.
  pub(super) fn draft_decided<B: ResourceBody>(&self, db: &ProjectDb, resource: ResourceId) -> Result<(), ToolError> {
    let Some(person) = &self.person else {
      return Ok(());
    };
    let saved = db.draft::<B>(resource)?.map(|draft| draft.updated_at());
    let latest = db
      .latest_version::<B>(resource)?
      .map_or(0, |version| version.number().get());
    if saved == Some(person.saved) && latest + 1 == person.version {
      return Ok(());
    }
    Err(changed_since_decided())
  }

  /// Refused unless no person decided the call, or `widens` is what they were shown it widens.
  pub(super) fn widening_decided(&self, widens: &[Message]) -> Option<Result<(), ToolError>> {
    let person = self.person.as_ref()?;
    Some(if person.widens == widens {
      Ok(())
    } else {
      Err(changed_since_decided())
    })
  }

  pub fn with_vault(mut self, vault: Vault) -> Self {
    self.vault = vault;
    self
  }

  /// The session the tools work in, for the Orchestrator chat's own rules (`crate::orchestrator::tools`).
  pub(crate) const fn workspace(&self) -> &Workspace {
    &self.workspace
  }

  /// The project `named` by its id or, ignoring case, its name, opened as the app opens it. Blocks on
  /// SQLite. It is not recorded as the most recently used project: only a person opening it is.
  pub(super) fn project(&self, named: &str) -> Result<OpenProject, ToolError> {
    let projects = self.workspace.projects()?;
    let by_id = projects.iter().find(|project| project.id.to_string() == named);
    let lowered = named.to_lowercase();
    let by_name: Vec<_> = projects
      .iter()
      .filter(|project| project.name.as_str().to_lowercase() == lowered)
      .collect();
    let registered = match (by_id, by_name.as_slice()) {
      (Some(project), _) => project,
      (None, [project]) => *project,
      (None, []) => return Err(ToolError::not_found(Missing::Project, named)),
      (None, _) => return Err(ToolError::AmbiguousProject(named.to_owned())),
    };
    if registered.folder_missing() {
      return Err(ToolError::refused(
        "project_folder_missing",
        vec![Message::new("errors.projectMissing")],
      ));
    }
    let db = ProjectDb::open(&registered.repo_root)?;
    Ok(OpenProject {
      project: db.project().clone(),
      repo_root: registered.repo_root.clone(),
    })
  }
}

/// The draft, or what it widens, is not what the person decided on.
fn changed_since_decided() -> ToolError {
  ToolError::refused(
    "changed_since_proposed",
    vec![Message::new("orchestrator.proposal.draftChangedPlain")],
  )
}

/// Runs `work`, which blocks on SQLite or git, on the blocking pool.
pub(crate) async fn blocking<T: Send + 'static>(
  work: impl FnOnce() -> Result<T, ToolError> + Send + 'static,
) -> Result<T, ToolError> {
  tokio::task::spawn_blocking(work).await.map_err(|error| {
    tracing::error!(%error, "an MCP tool's work did not finish");
    ToolError::interrupted()
  })?
}
