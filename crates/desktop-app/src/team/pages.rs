//! The lurq queries of a team project's pages (`docs/architecture/team-authority-plan.md` §6, ORC-717): one task by its
//! key, the workspace's modules and one module, each with its assignee, the claim its work runs under and what the
//! person may do with it, read from `orchester_workspace::team_reads` and kept, as the lists' queries are
//! ([`super::queries`]), by the [`TeamSource`]'s key and their own arguments. A claim or an assignee says whether it is
//! the person signed in to the server; who that is, is read from this computer's servers, never sent anywhere. A claim
//! says too whether this computer holds it, as its own records of the claims it took say
//! (`orchester_workspace::team_work::team_here`): the server names the holder, not the computer.

use orchester_api::{
  Attribution, UserRef,
  v2::{
    Action, Capabilities, ClaimRead, ClosedHow, ExecutionState, LifecycleAction, MAX_PAGE_LIMIT, ModuleListRequest,
    ModuleSummary, PageRequest, PendingAuthorization, SearchText, Subject, TaskFilters, TaskListRequest, TaskRead,
  },
};
use orchester_core::{
  forecast::Forecast,
  id::{ClaimId, ModuleId, RunId, UserId},
  module::{Module, progress::Progress},
  project::TaskKey,
  run::CommitId,
  sharing::TaskRevision,
  task::Task,
  time::Timestamp,
};
use orchester_store::TeamFence;
use orchester_workspace::{
  background,
  sharing::project::convert,
  team_reads::{TeamRead, TeamReadError, listed},
  team_work::{TeamHere, team_here},
};

use super::{
  queries::team_pipelines,
  source::{TeamFailure, TeamSource},
};

/// Someone a team read names: their name on the server, and whether they are the person signed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamPerson {
  pub name: String,
  pub you: bool,
}

/// The claim a task's or a module's work runs under, as the server reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamClaim {
  pub id: ClaimId,
  pub holder: TeamPerson,
  /// When it was taken.
  pub acquired: Timestamp,
  /// When the holder's work last renewed it.
  pub renewed: Timestamp,
  /// When it goes stale without another renewal, by the server's threshold.
  pub stale_at: Timestamp,
  pub stale: bool,
  /// A task's claim taken under its module's, or its module's own.
  pub under_module: bool,
  /// This computer took it and holds it, as far as it knows.
  pub here: bool,
}

/// Whole minutes from `from` to `to`, none below zero.
pub fn minutes(from: Timestamp, to: Timestamp) -> String {
  ((to.unix_millis() - from.unix_millis()) / 60_000).max(0).to_string()
}

/// What the server's capabilities let the person do with a task's lifecycle (decision D4): each is the capability
/// action of that name, which the server tells with its reason, so nothing here is a role kept on this computer. A
/// closed task's *Done* and *Cancel* are not offered, nor an open task's *Reopen*: the state is told before the person.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lifecycle {
  /// The task's capabilities list at least one lifecycle action, allowed or denied: only then does a screen state who
  /// does the rest, since a rule the server never denied is not one to tell.
  pub listed: bool,
  /// Mark as done: the claim's holder's or an Admin's.
  pub complete: bool,
  /// Cancel task: an Admin's.
  pub cancel: bool,
  /// Reopen a Done or Canceled task to Backlog or Todo: an Admin's.
  pub reopen: bool,
  /// Move between Backlog and Todo: an Admin's.
  pub triage: bool,
  /// Mark as blocked and Clear blocked: the claim's holder's or an Admin's.
  pub block: bool,
}

/// What the server's capabilities let the person do with a task or a module: advisory, as every command is checked
/// again there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Abilities {
  pub acquire: bool,
  pub take_over: bool,
  pub release: bool,
  /// Change the module's branch (an Admin's).
  pub branch: bool,
  /// Authorize the module's delivery (an Admin's).
  pub authorize: bool,
  pub lifecycle: Lifecycle,
}

/// A team task as its page shows it: the task, its revision, the slug of a pipeline this computer has none for, its
/// assignee, its claim, the state of the work under it and what the person may do with it; and the team runs of the
/// project the server fenced on this computer, with why and how many of their reports it never took, which the page
/// finds its latest run among.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamTask {
  pub task: Task,
  pub revision: TaskRevision,
  pub missing_pipeline: Option<String>,
  pub assignee: Option<TeamPerson>,
  pub claim: Option<TeamClaim>,
  pub work: Option<ExecutionState>,
  pub abilities: Abilities,
  /// How and when the server says a Done or Canceled task was closed; `None` while it is open, and also when the
  /// server does not say (an older one never does), so the card names no person and no time.
  pub closed: Option<TeamClosed>,
  /// Who set the task's current blocked reason and when; `None` when it has none, and when the server does not say.
  pub blocked: Option<TeamAttribution>,
  pub fences: Vec<(RunId, TeamFence)>,
  pub unsent: Vec<(RunId, usize)>,
}

/// Who made a change to a task, and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamAttribution {
  pub by: TeamPerson,
  pub at: Timestamp,
}

/// What closed a task, and when (`orchester_api::v2::TaskClosure`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamClosed {
  pub how: TeamClosedHow,
  pub at: Timestamp,
}

/// What closed a Done or Canceled task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamClosedHow {
  /// A person's change: the holder who marked it Done, or the Admin who closed or canceled it.
  Person(TeamPerson),
  /// The accepted delivery of its module marked it Done; the module's name as it was then.
  Delivery { module_name: String },
  /// The success of its work marked it Done.
  Run,
}

/// A team module as its row and its page show it: the module, its revision, its progress and forecast as the server
/// counts them, how many tasks it has (Canceled ones too), its branch, its assignee, its claim, what the person may do
/// with it, and its delivery to base that is requested and not yet delivered or failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamModule {
  pub module: Module,
  pub revision: TaskRevision,
  pub progress: Progress,
  pub tasks: u32,
  /// `None` when the server's forecast names a day that is no day.
  pub forecast: Option<Forecast>,
  pub branch: String,
  pub assignee: Option<TeamPerson>,
  pub claim: Option<TeamClaim>,
  pub abilities: Abilities,
  pub pending: Option<TeamDelivery>,
}

/// A module's open delivery to base as the server reads it: who asked for it and when, the module branch's commit it
/// takes onto the base commit, and how it is authorized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamDelivery {
  pub asked_by: TeamPerson,
  pub asked_at: Timestamp,
  pub source_commit: CommitId,
  pub base_commit: CommitId,
  pub authorization: PendingAuthorization,
}

/// The task `key`, found by the server's search for it; `None` when the project has no such task.
#[lurq::query(stale_time = "30s", gc_time = "5m")]
pub async fn team_task(source: TeamSource, key: TaskKey) -> Result<Option<TeamTask>, TeamFailure> {
  let shown = key.to_string();
  let request = TaskListRequest {
    filters: TaskFilters {
      search: SearchText::new(&shown).ok(),
      ..TaskFilters::default()
    },
    sort: Default::default(),
    page: PageRequest::new(None, Some(MAX_PAGE_LIMIT)).expect("the largest page is a page"),
  };
  let page = source.answered(source.project().task_page(request).await)?.into_value();
  let Some(read) = page.items.into_iter().find(|read| read.task.key == shown) else {
    return Ok(None);
  };
  let (pipelines, who) = (team_pipelines(&source).await?, Who::of(&source).await);
  team_task_of(&read, &pipelines, &who).map(Some)
}

/// The workspace's modules, oldest first.
#[lurq::query(stale_time = "1m", gc_time = "10m")]
pub async fn team_module_list(source: TeamSource) -> Result<Vec<TeamModule>, TeamFailure> {
  let request = ModuleListRequest {
    state: None,
    page: PageRequest::new(None, Some(MAX_PAGE_LIMIT)).expect("the largest page is a page"),
  };
  let page = source
    .answered(source.project().module_page(request).await)?
    .into_value();
  let who = Who::of(&source).await;
  page.items.iter().map(|summary| team_module_of(summary, &who)).collect()
}

/// The module `module`; `None` when the workspace has no such module.
#[lurq::query(stale_time = "1m", gc_time = "10m")]
pub async fn team_module(source: TeamSource, module: ModuleId) -> Result<Option<TeamModule>, TeamFailure> {
  let summary = match source.project().module(module).await {
    Err(TeamReadError::NotFound) => return Ok(None),
    read => source.answered(read).map(TeamRead::into_value)?,
  };
  team_module_of(&summary, &Who::of(&source).await).map(Some)
}

/// Who the person is on the source's server, and what this computer holds of the project's claims.
struct Who {
  you: Option<UserId>,
  here: TeamHere,
}

impl Who {
  async fn of(source: &TeamSource) -> Self {
    let root = source.root().to_owned();
    let here = match background::run(move || team_here(&root)).await {
      Ok(Ok(here)) => here,
      // The page then names no claim as this computer's: the server's reads still say who holds each.
      Ok(Err(error)) => {
        tracing::warn!(%error, "this computer's team claims cannot be read");
        TeamHere::default()
      }
      Err(_) => TeamHere::default(),
    };
    Self {
      you: you(source).await,
      here,
    }
  }
}

/// Who is signed in to the source's server on this computer; `None` when that cannot be read.
async fn you(source: &TeamSource) -> Option<UserId> {
  let (servers, server) = (source.servers().clone(), source.key().server);
  let entries = background::run(move || servers.entries()).await.ok()?.ok()?;
  let entry = entries.into_iter().find(|entry| entry.stored.id == server)?;
  entry.stored.account.map(|account| account.user)
}

fn person(user: &UserRef, you: Option<UserId>) -> TeamPerson {
  TeamPerson {
    name: user.display_name.clone(),
    you: you == Some(user.id),
  }
}

fn attribution(read: &Attribution, you: Option<UserId>) -> TeamAttribution {
  TeamAttribution {
    by: person(&read.by, you),
    at: Timestamp::from_unix_millis(read.at),
  }
}

fn claim(read: &ClaimRead, who: &Who) -> TeamClaim {
  let holder = person(&read.holder, who.you);
  TeamClaim {
    id: read.id,
    here: holder.you && who.here.holds(read.id),
    holder,
    acquired: Timestamp::from_unix_millis(read.acquired_at),
    renewed: Timestamp::from_unix_millis(read.last_execution_heartbeat_at),
    stale_at: Timestamp::from_unix_millis(read.stale_at),
    stale: read.stale,
    under_module: read.parent_module_claim_id.is_some() || matches!(read.subject, Subject::Module { .. }),
  }
}

/// What `capabilities` allow on `subject`; an action listed for no subject is the workspace's, and holds for it too. A
/// command's kind is named by its dotted words (`claim.acquire` is `["claim", "acquire"]`).
fn abilities(capabilities: &Capabilities, subject: Subject) -> Abilities {
  let allowed = |words: &[&str]| {
    capabilities.actions.iter().any(|action| {
      matches!(action.action(), Action::Command(command) if command.as_str().split('.').eq(words.iter().copied()))
        && action.subject().is_none_or(|named| named == subject)
        && action.is_allowed()
    })
  };
  Abilities {
    acquire: allowed(&["claim", "acquire"]),
    take_over: allowed(&["claim", "take_over"]),
    release: allowed(&["claim", "release"]),
    branch: allowed(&["module", "branch", "set"]),
    authorize: allowed(&["delivery", "authorize"]),
    lifecycle: lifecycle(capabilities, subject),
  }
}

fn team_task_of(read: &TaskRead, pipelines: &listed::TeamPipelines, who: &Who) -> Result<TeamTask, TeamFailure> {
  let named = read.task.content.pipeline.as_deref();
  let pipeline = named.and_then(|named| pipelines.id(named));
  let task = listed::task_of(read, pipeline).map_err(|error| {
    tracing::warn!(%error, "a team task cannot be shown");
    TeamFailure::Refused
  })?;
  Ok(TeamTask {
    task,
    revision: read.task.revision,
    missing_pipeline: named.filter(|_| pipeline.is_none()).map(str::to_owned),
    assignee: read.assignee.as_ref().map(|user| person(user, who.you)),
    claim: read.effective_claim.as_ref().map(|read| claim(read, who)),
    work: read.work.as_ref().map(|work| work.state),
    abilities: abilities(&read.capabilities, Subject::Task { id: read.task.id }),
    closed: read.closed.as_ref().map(|closure| TeamClosed {
      how: match &closure.how {
        ClosedHow::Person { by } => TeamClosedHow::Person(person(by, who.you)),
        ClosedHow::Delivery { module_name, .. } => TeamClosedHow::Delivery {
          module_name: module_name.clone(),
        },
        ClosedHow::Run { .. } => TeamClosedHow::Run,
      },
      at: Timestamp::from_unix_millis(closure.at),
    }),
    blocked: read.blocked.as_ref().map(|blocked| attribution(blocked, who.you)),
    fences: who.here.fences.clone(),
    unsent: who.here.unsent.clone(),
  })
}

fn team_module_of(summary: &ModuleSummary, who: &Who) -> Result<TeamModule, TeamFailure> {
  let read = &summary.module;
  let module = convert::cached_module(&read.module).map_err(|error| {
    tracing::warn!(%error, "a team module cannot be shown");
    TeamFailure::Refused
  })?;
  Ok(TeamModule {
    module: module.module,
    revision: read.module.revision,
    progress: summary.progress.to_core(),
    tasks: summary.tasks,
    forecast: summary.forecast.to_core(),
    branch: read.branch.clone(),
    assignee: read.assignee.as_ref().map(|user| person(user, who.you)),
    claim: read.effective_claim.as_ref().map(|read| claim(read, who)),
    abilities: abilities(&read.capabilities, Subject::Module { id: read.module.id }),
    pending: read.pending_delivery.as_ref().map(|pending| TeamDelivery {
      asked_by: person(pending.requested_by(), who.you),
      asked_at: Timestamp::from_unix_millis(pending.requested_at()),
      source_commit: pending.source_commit().clone(),
      base_commit: pending.base_commit().clone(),
      authorization: pending.authorization(),
    }),
  })
}

/// What `capabilities` allow of the lifecycle of `subject`: the actions the server lists for it and allows.
pub fn lifecycle(capabilities: &Capabilities, subject: Subject) -> Lifecycle {
  let allowed = |wanted: LifecycleAction| {
    capabilities.actions.iter().any(|action| {
      *action.action() == Action::Task(wanted)
        && action.subject().is_none_or(|named| named == subject)
        && action.is_allowed()
    })
  };
  let listed = capabilities.actions.iter().any(|action| {
    LifecycleAction::ALL
      .iter()
      .any(|wanted| *action.action() == Action::Task(*wanted))
      && action.subject().is_none_or(|named| named == subject)
  });
  Lifecycle {
    listed,
    complete: allowed(LifecycleAction::Complete),
    cancel: allowed(LifecycleAction::Cancel),
    reopen: allowed(LifecycleAction::Reopen),
    triage: allowed(LifecycleAction::Triage),
    block: allowed(LifecycleAction::Block),
  }
}
