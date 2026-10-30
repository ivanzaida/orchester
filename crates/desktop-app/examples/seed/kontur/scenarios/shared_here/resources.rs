//! The team's configuration as a member's computer keeps it: the server's resources, with no draft here, and the
//! versions the server confirmed with the references they name — the profiles Builder, Planner, Reviewer, Explorer and
//! Triage, the tools Query staging and Report crash, and the pipeline Feature delivery, whose steps run Explorer and
//! Builder. A version's local body names what its references are bound to here, or the publisher's id while nothing is.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  blob::ContentHash,
  connection::mcp::McpServerSelection,
  id::{ConnectionId, ResourceId, SecretId, UserId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    StepPermissions,
  },
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, ApprovalMode, DeniedBehavior, HttpAuth, HttpMethod, HttpRequest, ModelId,
    Pipeline, PromptText, ProviderSettings, Resource, ResourceBody, Slug, Step, StepKey, StepKind, Tool, ToolAction,
    VersionNumber,
  },
  sharing::{SharedPerson, TaskRevision},
};
use orchester_store::{CachedVersion, ReferenceTarget, VersionReference};

use super::{DAY, Local, Theirs, invalid};
use crate::kontur::{Kontur, SeedError, name};

/// One version the server confirmed: its number, who published it and how long ago, its local body, what it names, and
/// whether it waits for *Use v<N> here*.
struct Published<B> {
  number: u32,
  publisher: &'static str,
  ago: i64,
  body: B,
  references: Vec<VersionReference>,
  waits: bool,
}

/// Keeps the team's resources and caches their versions, oldest first.
pub(super) fn keep(kontur: &mut Kontur, theirs: &Theirs, local: &Local) -> Result<(), SeedError> {
  let work = (theirs.work, local.work.unwrap_or(theirs.work));
  let mut keeper = Keeper::default();
  let builder = keeper.keep(kontur, ("builder", "Builder"), builder(work)?)?;
  keeper.keep(kontur, ("planner", "Planner"), planner(work)?)?;
  keeper.keep(kontur, ("reviewer", "Reviewer"), reviewer(theirs, local.codex)?)?;
  let explorer = keeper.keep(kontur, ("explorer", "Explorer"), explorer(theirs)?)?;
  keeper.keep(kontur, ("triage", "Triage"), triage(theirs, local.codex)?)?;
  let query = query_staging(theirs, local.staging)?;
  keeper.keep(kontur, ("query-staging", "Query staging"), vec![query])?;
  keeper.keep(kontur, ("report-crash", "Report crash"), vec![report_crash(theirs)?])?;
  let delivery = version(
    (1, "dana-k", 12 * DAY),
    feature_delivery(explorer, builder)?,
    Vec::new(),
  );
  keeper.keep(kontur, ("feature-delivery", "Feature delivery"), vec![delivery])?;
  Ok(())
}

/// Counts the versions cached, so each has a hash of its own, as each of the server's has.
#[derive(Default)]
struct Keeper {
  cached: u32,
}

impl Keeper {
  /// Keeps a resource of `B`'s kind as the server's, then caches `versions`; its id.
  fn keep<B: ResourceBody>(
    &mut self,
    kontur: &mut Kontur,
    (slug, named): (&str, &str),
    versions: Vec<Published<B>>,
  ) -> Result<ResourceId, SeedError> {
    let resource = Resource::new(
      ResourceId::generate(),
      B::KIND,
      Slug::new(slug).map_err(invalid)?,
      name(named),
      kontur.ago(30 * DAY),
    );
    kontur.db.keep_shared_resource(&resource, TaskRevision::FIRST)?;
    for version in versions {
      self.cached += 1;
      cache(kontur, resource.id(), version, self.cached)?;
    }
    Ok(resource.id())
  }
}

/// Caches `version` of resource `id`, with the hash `sequence` gives it.
fn cache<B: ResourceBody>(
  kontur: &mut Kontur,
  id: ResourceId,
  version: Published<B>,
  sequence: u32,
) -> Result<(), SeedError> {
  let number = std::num::NonZeroU32::new(version.number).ok_or_else(|| invalid("version 0"))?;
  let hash = format!("{sequence:064x}");
  let cached = CachedVersion {
    number: VersionNumber::new(number),
    published_at: kontur.ago(version.ago),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: version.publisher.to_owned(),
    },
    server_body: serde_json::to_value(&version.body).map_err(invalid)?,
    hash: ContentHash::new(&hash).map_err(invalid)?,
    references: version.references,
    local_body: version.body,
    waits_here: version.waits,
  };
  kontur.db.cache_shared_version(id, &cached)?;
  Ok(())
}

fn reference(target: ReferenceTarget, named: &str, role: Option<&str>) -> VersionReference {
  VersionReference {
    target,
    name: named.to_owned(),
    role: role.map(str::to_owned),
  }
}

/// A profile on `connection` and `model`, with `instructions`.
fn profile(
  role: &str,
  (responsibility, instructions): (&str, &str),
  (connection, model): (ConnectionId, &str),
) -> Result<AgentProfile, SeedError> {
  Ok(AgentProfile::new(
    name(role),
    responsibility.to_owned(),
    PromptText::new(instructions).map_err(invalid)?,
    connection,
    ModelId::new(model).map_err(invalid)?,
    ProviderSettings::default(),
  ))
}

fn version<B>(
  (number, publisher, ago): (u32, &'static str, i64),
  body: B,
  references: Vec<VersionReference>,
) -> Published<B> {
  Published {
    number,
    publisher,
    ago,
    body,
    references,
    waits: false,
  }
}

/// The team's Claude Code `Work` by the publisher's id, and the connection the local body names for it.
type Work = (ConnectionId, ConnectionId);

/// Builder v4 on the team's Claude Code `Work`.
fn builder((theirs, here): Work) -> Result<Vec<Published<AgentProfile>>, SeedError> {
  let body = profile(
    "Builder",
    (
      "Implements the task in its worktree",
      "# Builder\nWork only inside the task worktree. Keep each commit small and\nrun cargo test before you hand off.",
    ),
    (here, "claude-opus-5-5"),
  )?;
  let work = reference(ReferenceTarget::Connection(theirs), "Work", Some("claude_code_cli"));
  Ok(vec![version((4, "dana-k", 3 * DAY), body, vec![work])])
}

/// Planner v2 on `Work` too.
fn planner((theirs, here): Work) -> Result<Vec<Published<AgentProfile>>, SeedError> {
  let body = profile(
    "Planner",
    ("Plans the task", "# Planner\nWrite the plan; never edit files."),
    (here, "claude-opus-5-5"),
  )?;
  let work = reference(ReferenceTarget::Connection(theirs), "Work", Some("claude_code_cli"));
  Ok(vec![version((2, "dana-k", 9 * DAY), body, vec![work])])
}

/// Reviewer v4 on the team's Codex `Default`, and olha's v5, which adds the MCP server `github` and network (02d): it
/// widens, so it waits.
fn reviewer(theirs: &Theirs, codex: ConnectionId) -> Result<Vec<Published<AgentProfile>>, SeedError> {
  let commands = |listed: &[&str]| -> Result<CommandPermission, SeedError> {
    let prefixes = listed
      .iter()
      .map(|command| CommandPrefix::new(command).map_err(invalid))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(CommandPermission::Allowed(
      AllowedCommands::new(prefixes).map_err(invalid)?,
    ))
  };
  let v4 = profile(
    "Reviewer",
    (
      "Reviews the change, comments on the PR",
      "# Reviewer\nRead the diff and the task's criteria. Comment on the\npull request; never push to the branch.",
    ),
    (codex, "gpt-5.5"),
  )?
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    commands(&["gh pr view"])?,
    NetworkAccess::Off,
  ));
  let v5 = v4
    .clone()
    .with_permissions(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      commands(&["gh pr view"])?,
      NetworkAccess::On,
    ))
    .with_mcp_servers(McpServerSelection::new(vec![theirs.github]).map_err(invalid)?);
  let default = || {
    reference(
      ReferenceTarget::Connection(theirs.default),
      "Default",
      Some("codex_cli"),
    )
  };
  let github = reference(ReferenceTarget::McpServer(theirs.github), "github", None);
  Ok(vec![
    version((4, "dana-k", 3 * DAY), v4, vec![default()]),
    Published {
      waits: true,
      ..version((5, "olha", 60), v5, vec![default(), github])
    },
  ])
}

/// Explorer v1 on the team's API connection `DeepSeek`, which is missing here.
fn explorer(theirs: &Theirs) -> Result<Vec<Published<AgentProfile>>, SeedError> {
  let body = profile(
    "Explorer",
    (
      "Explores the code and answers questions",
      "# Explorer\nRead the code the task names and answer in short notes.\nNever edit files.",
    ),
    (theirs.deepseek, "deepseek-chat"),
  )?;
  let deepseek = reference(
    ReferenceTarget::Connection(theirs.deepseek),
    "DeepSeek",
    Some("openai_api"),
  );
  Ok(vec![version((1, "dana-k", 12 * DAY), body, vec![deepseek])])
}

/// Triage v2, whose agents get the team's MCP server `linear`, which is missing here.
fn triage(theirs: &Theirs, codex: ConnectionId) -> Result<Vec<Published<AgentProfile>>, SeedError> {
  let body = profile(
    "Triage",
    ("Sorts new issues", "# Triage\nLabel new issues and link duplicates."),
    (codex, "gpt-5.5"),
  )?
  .with_mcp_servers(McpServerSelection::new(vec![theirs.linear]).map_err(invalid)?);
  let linear = reference(ReferenceTarget::McpServer(theirs.linear), "linear", None);
  Ok(vec![version((2, "dana-k", 6 * DAY), body, vec![linear])])
}

/// An HTTP tool posting to `url` with the bearer secret `key`, which runs in Development on each call's approval.
fn http_tool(url: &str, host: &str, key: SecretId) -> Result<Tool, SeedError> {
  let request = HttpRequest::new(
    HttpMethod::Post,
    url.into(),
    host.into(),
    HttpAuth::Bearer { secret: key },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .map_err(invalid)?;
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    name("Development"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .map_err(invalid)?;
  Tool::new("", ToolAction::Http(request), Some(policy)).map_err(invalid)
}

/// Query staging v1, with the team's `STAGING_DB_URL`, bound here to `staging`.
fn query_staging(theirs: &Theirs, staging: SecretId) -> Result<Published<Tool>, SeedError> {
  let body = http_tool("https://staging.example.test/query", "staging.example.test", staging)?;
  let key = reference(
    ReferenceTarget::Secret(theirs.staging),
    "STAGING_DB_URL",
    Some("database_connection"),
  );
  Ok(version((1, "dana-k", 8 * DAY), body, vec![key]))
}

/// Report crash v1, which sends the team's `SENTRY_TOKEN`, missing here, to sentry.io.
fn report_crash(theirs: &Theirs) -> Result<Published<Tool>, SeedError> {
  let body = http_tool("https://sentry.io/api/0/envelope/", "sentry.io", theirs.sentry)?;
  let key = reference(
    ReferenceTarget::Secret(theirs.sentry),
    "SENTRY_TOKEN",
    Some("access_token"),
  );
  Ok(version((1, "dana-k", 8 * DAY), body, vec![key]))
}

/// Feature delivery, a draft whose steps run Explorer, then Builder.
fn feature_delivery(explorer: ResourceId, builder: ResourceId) -> Result<Pipeline, SeedError> {
  let step = |key: &str, named: &str, profile: ResourceId, after: &[&str]| -> Result<Step, SeedError> {
    Ok(Step {
      key: StepKey::new(key).map_err(invalid)?,
      name: name(named),
      kind: StepKind::Agent {
        profile: Some(profile),
        repository: RepositoryAccess::Write,
        model: None,
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new(named).map_err(invalid)?,
      depends_on: after
        .iter()
        .map(|key| StepKey::new(key).map_err(invalid))
        .collect::<Result<BTreeSet<_>, _>>()?,
      approval_gate: false,
      on_failure: None,
      budget: None,
    })
  };
  Pipeline::new(vec![
    step("explore", "Explore", explorer, &[])?,
    step("build", "Build", builder, &["explore"])?,
  ])
  .map_err(invalid)
}
