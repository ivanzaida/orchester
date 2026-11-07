//! `picker-readiness` and `picker-readiness-team`: New task and Edit task whose Pipeline menu says whether each
//! pipeline is ready on this computer (`New task · pipeline picker shows readiness`, `P6aAoZ` for a team project and
//! `PMi6Q` for a local one in `Agents · v2` `acFkz`, ORC-841).
//!
//! In the local project the catalogue's Feature delivery and Quick fix are ready; Design pass runs a profile that
//! chooses two MCP servers whose programs this computer does not have, Review pass one of them. The local project's
//! KON-1 is a draft with every detail on Design pass: its Run is refused `not_ready_here`, and the task's Summary lists
//! the two programs to set (`Task · Run · not ready`, L9 `f5CEX`, ORC-840). In the team project
//! (`team-hub`) Builder needs the program of `pencil-design`, `github`'s secret and a connection, Reviewer a
//! connection, so Team delivery, which runs both, lacks more than Team quick fix, which runs Reviewer.

use std::collections::BTreeSet;

use orchester_core::{
  connection::mcp::{McpServerSelection, McpTransport, StdioServer},
  id::{ConnectionId, IdempotencyKey, ResourceId},
  permission::{RepositoryAccess, StepPermissions},
  resource::{
    AgentProfile, CommandProgram, ModelId, Pipeline, PromptText, ProviderSettings, ResourceKind, Slug, Step, StepKey,
    StepKind,
  },
  store::{McpServerStore, ResourceStore, TaskStore},
  task::TaskDetails,
};

use super::{connections, team_hub};
use crate::kontur::{Kontur, SeedError, catalogue, name};

/// The programs the local project's servers start; no PATH has them.
const MISSING_PROGRAMS: [&str; 2] = ["orchester-qa-missing-docs", "orchester-qa-missing-search"];

/// A local project: two ready pipelines, and two that lack one or two MCP server programs.
pub fn local(kontur: &mut Kontur) -> Result<(), SeedError> {
  let seeded = connections::seeded(kontur)?;
  let at = kontur.ago(4);
  let mut servers = Vec::new();
  for program in MISSING_PROGRAMS {
    let transport = McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(program).map_err(invalid)?,
      Vec::new(),
      None,
    ));
    let server = connections::server((program, transport), Default::default(), at)?;
    kontur.db.create_mcp_server(&server)?;
    servers.push(server.id());
  }
  let design = local_profile(kontur, "Design pass", seeded.codex, &servers)?;
  let review = local_profile(kontur, "Review pass", seeded.codex, &servers[..1])?;
  let mut published = Vec::new();
  for (title, profile) in [("Design pass", design), ("Review pass", review)] {
    let id = kontur
      .db
      .create_resource(slug(title)?, name(title), &pipeline(&[profile])?, at)?
      .id();
    kontur.db.publish_draft::<Pipeline>(id, at)?;
    published.push(id);
  }
  // Design pass, the first one published.
  let design_pass = published
    .first()
    .copied()
    .ok_or_else(|| SeedError::Invalid("no pipeline was published".to_owned()))?;
  draft_ready_to_run(kontur, design_pass)
}

/// KON-1, a draft with every detail on `pipeline`: Run is pressable, and the pipeline is not ready here.
fn draft_ready_to_run(kontur: &mut Kontur, pipeline: ResourceId) -> Result<(), SeedError> {
  let title = "Add retry to webhook sender";
  let details = TaskDetails {
    title: name(title),
    brief: "Webhook deliveries fail for good after a single network error. Retry failed deliveries with exponential \
            backoff and jitter, keep every attempt visible in the endpoint's delivery log, and let each endpoint \
            set how many attempts it gets."
      .to_owned(),
    acceptance_criteria: [
      "Failed deliveries retry with exponential backoff and full jitter.",
      "429 and 503 responses honor Retry-After, capped at 60 s.",
      "Each endpoint has a retry budget; the default is 5 attempts.",
      "Every attempt is recorded with its status code and delay.",
      "Tests cover the backoff schedule and the jitter bound.",
    ]
    .map(str::to_owned)
    .to_vec(),
    pipeline: Some(pipeline),
    limits: crate::kontur::limits(),
  };
  let at = kontur.ago(30);
  let task = kontur
    .db
    .create_tasks(IdempotencyKey::generate(), vec![details], at)?
    .remove(0);
  kontur.seeded(&task, "draft on a pipeline that is not ready here");
  Ok(())
}

/// The team project of `team-hub` with the team's pipelines Team delivery (Builder, then Reviewer) and Team quick fix
/// (Reviewer), beside the catalogue's, which this computer has no team form of.
pub fn team(kontur: &mut Kontur) -> Result<(), SeedError> {
  team_hub::seed(kontur)?;
  let profile = |role: &str| -> Result<ResourceId, SeedError> {
    kontur
      .db
      .resources(ResourceKind::AgentProfile)?
      .into_iter()
      .find(|resource| resource.name().as_str() == role)
      .map(|resource| resource.id())
      .ok_or_else(|| SeedError::Invalid(format!("the team has no profile {role}")))
  };
  let (builder, reviewer) = (profile("Builder")?, profile("Reviewer")?);
  team_hub::team_resource(kontur, "Team delivery", &pipeline(&[builder, reviewer])?)?;
  team_hub::team_resource(kontur, "Team quick fix", &pipeline(&[reviewer])?)
}

/// A profile `title` on `connection` whose agents get `servers`, published.
fn local_profile(
  kontur: &mut Kontur,
  title: &str,
  connection: ConnectionId,
  servers: &[ConnectionId],
) -> Result<ResourceId, SeedError> {
  let body = AgentProfile::new(
    name(title),
    String::new(),
    PromptText::new("Do the work the step asks.").map_err(invalid)?,
    connection,
    ModelId::new(catalogue::MODEL).map_err(invalid)?,
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(servers.to_vec()).map_err(invalid)?);
  let at = kontur.ago(4);
  let id = kontur.db.create_resource(slug(title)?, name(title), &body, at)?.id();
  kontur.db.publish_draft::<AgentProfile>(id, at)?;
  Ok(id)
}

/// A pipeline whose steps run `profiles` one after the other.
fn pipeline(profiles: &[ResourceId]) -> Result<Pipeline, SeedError> {
  let mut steps: Vec<Step> = Vec::new();
  for (index, profile) in profiles.iter().enumerate() {
    let key = format!("step-{}", index + 1);
    let title = format!("Step {}", index + 1);
    steps.push(Step {
      key: StepKey::new(&key).map_err(invalid)?,
      name: name(&title),
      kind: StepKind::Agent {
        profile: Some(*profile),
        repository: RepositoryAccess::Write,
        model: None,
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new(&title).map_err(invalid)?,
      depends_on: steps
        .last()
        .map(|before| BTreeSet::from([before.key.clone()]))
        .unwrap_or_default(),
      approval_gate: false,
      on_failure: None,
      budget: None,
    });
  }
  Pipeline::new(steps).map_err(invalid)
}

fn slug(title: &str) -> Result<Slug, SeedError> {
  Slug::from_name(title).ok_or_else(|| SeedError::Invalid(format!("{title} yields no slug")))
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
