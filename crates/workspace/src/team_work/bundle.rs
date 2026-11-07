//! The execution bundle a start's receipt names, read from the server by its hash with every file it pins, each checked
//! against the bundle, and made into the run's inputs with what this computer filled in.

use std::{path::Path, time::Duration};

use orchester_api::{
  client::{Authority, BundleReadError},
  v2::{AuthorityRef, ExecutionBundle, Subject},
};
use orchester_core::{
  blob::ContentHash,
  connection::{Connection, ConnectionKind},
  resource::TeamMcpLaunch,
  run::{
    CommitId,
    inputs::{
      BundleContent, InputsError, InputsOrigin, LocalBindings, LocalConsent, ProfileBinding, RunInputs, TeamAcceptance,
      TeamMcpBinding, team_mcp_id,
    },
  },
  secret::SecretVault,
  store::ConnectionStore,
  task::TaskLimits,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use super::{Answer, TeamWorkError, answer};
use crate::readiness::{self, McpPrograms};

/// What the bundle must say to be the start's: the authority and the base commit it was asked for.
pub(super) struct Expected<'a> {
  pub(super) sha256: &'a ContentHash,
  pub(super) authority: AuthorityRef,
  pub(super) base_commit: &'a CommitId,
}

/// The bundle and the bytes of every file it pins, in its files' order.
pub(super) struct Fetched {
  pub(super) bundle: ExecutionBundle,
  pub(super) files: Vec<Vec<u8>>,
}

/// Reads the bundle `expected` names and every file it pins, each checked; the server's answer when it did not give
/// them. A bundle this computer cannot read is an error, not an answer: reading it again gives the same bundle.
pub(super) async fn fetch(
  authority: &Authority<'_>,
  expected: &Expected<'_>,
) -> Result<Result<Fetched, Answer>, TeamWorkError> {
  let bundle = match authority.bundle(expected.sha256).await {
    Ok(bundle) => bundle,
    Err(BundleReadError::Call(error)) => return Ok(Err(answer(error))),
    Err(BundleReadError::Schema { built }) => return Err(TeamWorkError::BundleSchema { built }),
    Err(BundleReadError::Unreadable { .. }) => return Err(TeamWorkError::BundleUnreadable),
  };
  check(&bundle, expected)?;
  let mut files = Vec::new();
  for file in bundle.inputs().files() {
    let bytes = match authority.bundle_file(expected.sha256, &file.sha256).await {
      Ok(bytes) => bytes,
      Err(error) => return Ok(Err(answer(error))),
    };
    bundle.check_file(&file.sha256, &bytes)?;
    files.push(bytes);
  }
  Ok(Ok(Fetched { bundle, files }))
}

/// Whether `bundle` is the one `expected` names, built for its claim, task and base commit.
fn check(bundle: &ExecutionBundle, expected: &Expected<'_>) -> Result<(), TeamWorkError> {
  let mismatch = |reason| TeamWorkError::BundleMismatch {
    expected: expected.sha256.clone(),
    reason,
  };
  let hash = bundle.sha256().map_err(|_| mismatch("it has no canonical hash"))?;
  let start = bundle.start();
  let checks = [
    (hash == *expected.sha256, "its hash"),
    (start.claim_id == expected.authority.claim_id, "its claim"),
    (start.subject == expected.authority.subject, "its task"),
    (start.base_commit == *expected.base_commit, "its base commit"),
  ];
  match checks.into_iter().find(|(matches, _)| !matches) {
    Some((_, reason)) => Err(mismatch(reason)),
    None => Ok(()),
  }
}

/// The run's inputs of `bundle`, accepted as `acceptance`, with what this computer has filled in for the versions the
/// bundle pins: the connection each profile runs on, and the program path of each team MCP server (`programs`). It is
/// refused when the pinned versions need something this computer lacks ([`TeamWorkError::NotReadyHere`]): the versions
/// the team server pins can be newer than the ones readiness was read from before the start. `vault` is asked whether it
/// has each team secret's value, never for the value.
pub(super) fn inputs(
  (db, vault): (&ProjectDb, &dyn SecretVault),
  bundle: &ExecutionBundle,
  acceptance: TeamAcceptance,
  programs: &McpPrograms,
) -> Result<RunInputs, TeamWorkError> {
  let missing = readiness::bundle_missing(db, vault, programs, bundle)?;
  if !missing.is_empty() {
    return Err(TeamWorkError::NotReadyHere {
      pipeline: bundle.inputs().pipeline().label().name().as_str().to_owned(),
      missing,
    });
  }
  let bindings = bindings(db, bundle, programs)?;
  let start = bundle.start();
  let budget = start
    .limits
    .budget_tokens
    .map(TokenBudget::new)
    .transpose()
    .map_err(|_| InputsError::Unrepresentable("token budget"))?;
  let limits =
    TaskLimits::new(budget, start.limits.max_duration_ms.map(Duration::from_millis)).map_err(InputsError::from)?;
  let content = BundleContent::new(
    InputsOrigin::Team(acceptance),
    (start.task_number, start.task.clone()),
    (limits, start.delivery),
    bundle.inputs().clone(),
  )?;
  Ok(RunInputs::new(content, bindings, LocalConsent::default())?)
}

/// How this computer binds `bundle`: each pinned profile to the connection it picked for the profile, else the only one
/// of the CLI the bundle binds the profile to, and each pinned team MCP server to its program path.
fn bindings(db: &ProjectDb, bundle: &ExecutionBundle, programs: &McpPrograms) -> Result<LocalBindings, TeamWorkError> {
  let connections = db.connections()?;
  let inputs = bundle.inputs();
  let mut profiles = Vec::new();
  for binding in bundle.bindings() {
    let runs_cli = |connection: &&Connection| *connection.kind() == ConnectionKind::Agent(binding.cli);
    let pick = db.connection_pick(binding.resource_id)?.filter(|pick| {
      connections
        .iter()
        .filter(runs_cli)
        .any(|found| found.id() == pick.connection)
    });
    let (connection, account) = match pick {
      Some(pick) => (pick.connection, pick.account),
      None => match connections.iter().filter(runs_cli).collect::<Vec<_>>().as_slice() {
        [only] => (only.id(), None),
        // Readiness said one is there, and one was removed or added since.
        _ => return Err(InputsError::MissingBinding(binding.resource_id).into()),
      },
    };
    let mcp_servers = inputs
      .profiles()
      .iter()
      .find(|pinned| pinned.pinned().resource == binding.resource_id)
      .map_or_else(Vec::new, |pinned| {
        pinned.body().mcp_servers().iter().copied().map(team_mcp_id).collect()
      });
    profiles.push(ProfileBinding {
      resource: binding.resource_id,
      connection,
      account,
      required_cli: Some(binding.cli),
      mcp_servers,
    });
  }
  let team_mcp = inputs
    .mcp_servers()
    .iter()
    .map(|pinned| {
      let server = pinned.pinned().resource;
      let runs_program = matches!(pinned.body().launch(), TeamMcpLaunch::Program(_));
      TeamMcpBinding {
        server,
        program: programs.path(server).filter(|_| runs_program).map(Path::to_path_buf),
      }
    })
    .collect();
  Ok(LocalBindings {
    profiles,
    references: Vec::new(),
    team_mcp,
  })
}

/// The subject a start's authority names, which must be a task's.
pub(super) const fn task_subject(subject: Subject) -> Option<orchester_core::id::TaskId> {
  match subject {
    Subject::Task { id } => Some(id),
    Subject::Module { .. } => None,
  }
}
