//! The Orchestrator chat follows the agents of an open project (`orchester_workspace::orchestrator::waits`): whenever
//! the project's runs change state or its conversations change, its new waits are kept in the conversation, and a
//! wait that starts an Orchestrator turn by itself gets its turn as a person's message does: the project's runner
//! attached first, its tools served (`mcp::chat`) and the runner asked to answer it. Mounted with the project's feed,
//! so it follows the project while its tabs are inactive and whether or not the Orchestrator's screen is open. Each
//! follow also reads where the runs' base branches are, and tells the runs a move left behind
//! (`orchester_workspace::base_moved`).

use std::{
  collections::{HashMap, HashSet},
  path::PathBuf,
  sync::LazyLock,
};

use lurq::app::{
  ctx::{Ctx, StreamEmitter},
  i18n::I18n,
};
use orchester_store::StoreError;
use orchester_workspace::{
  background, base_moved,
  messages::{self, Message},
  orchestrator::{
    queue,
    send::{self, SendError},
    tools::ChatScope,
    waits::{self, Followed, MergeChecks, SucceededAt, Switches, WaitTurn},
  },
  runner::Runners,
  tasks::outside::Outside,
  workspace::{OpenProject, OrchestratorConsent},
};

use crate::services::Services;

/// One follower keeps waits at a time, so two that overlap never both open a conversation for the same new wait; the
/// daily report (`super::daily_report`) is kept under it too, so a report and a wait never both open one.
pub(super) static KEEPING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The runs of each project, by its folder, that git found merged outside Orchester or with nothing to merge, each by
/// the event it succeeded with: a run that succeeded so stays so until it works again, and git is not asked about it
/// again.
static QUIET: LazyLock<parking_lot::Mutex<HashMap<PathBuf, HashSet<SucceededAt>>>> = LazyLock::new(Default::default);

/// Follows `open`'s waits again whenever its run `states` or its conversations change. Renders nothing.
pub(super) fn follow(ctx: &mut Ctx, (open, states): (&OpenProject, u64)) {
  let services = Services::of(ctx);
  let chats = chats(ctx, (services.runners.clone(), open.repo_root.clone()));
  let i18n = ctx.i18n().clone();
  let open = open.clone();
  ctx.future((open.repo_root.clone(), states, chats), move |_| {
    let (services, i18n, open) = (services.clone(), i18n.clone(), open.clone());
    async move {
      // A future whose deps change is replaced and aborted, and the turn a follower starts changes them (its
      // conversation thinks, its tools change runs): following runs as a task of its own, so a turn it marked
      // thinking is always asked for or ended.
      let runtime = services.runtime.clone();
      let following = runtime.spawn(async move { follow_once(&services, i18n, &open).await });
      let followed = following.await.unwrap_or_else(|error| {
        tracing::error!(%error, "following the waits of the project's agents did not finish");
        Err(messages::interrupted())
      });
      if let Err(error) = &followed {
        tracing::error!(?error, "cannot follow the waits of the project's agents");
      }
      followed
    }
  });
}

/// Starts the turn of a message the person queued while the Orchestrator answered, once its conversation's turn
/// ended; keeps the project's new waits, and asks the runner for the turn a wait started by itself, if one did. A turn
/// starts only once the runner attached, since one that attaches ends the turns still thinking as interrupted. The
/// conversations are told of a change only when there was one: telling them wakes this follower again.
async fn follow_once(services: &Services, i18n: I18n, open: &OpenProject) -> Result<(), Message> {
  start_queued(services, i18n.clone(), open).await?;
  let switches = {
    let workspace = services.workspace.clone();
    background::run(move || {
      Ok::<_, StoreError>(Switches {
        answers: workspace.orchestrator_allows(OrchestratorConsent::AnswerAgents)?,
        continues: workspace.orchestrator_allows(OrchestratorConsent::ContinueInterrupted)?,
        merges: workspace.orchestrator_allows(OrchestratorConsent::MergeFinished)?
          && workspace.orchestrator_allows(OrchestratorConsent::MergeRuns)?,
      })
    })
    .await?
    .map_err(|error| messages::store_error(&error))?
  };
  let checks = check_merges(services, open).await?;
  observe_bases(services, open).await?;
  let mut followed = keep(services, open, &checks, (switches, false)).await?;
  if followed.pending {
    match services.runners.chat_runner(open).await {
      Ok(()) => {
        let started = keep(services, open, &checks, (switches, true)).await?;
        followed.kept |= started.kept;
        followed.turn = started.turn;
      }
      Err(error) => tracing::info!(%error, "no runner attached for the turn an agent's wait starts; it starts later"),
    }
  }
  if followed.kept || followed.turn.is_some() {
    services.runners.chat_changed(&open.repo_root);
  }
  // The Orchestrator completed a run that committed nothing (ORC-93): its task is Done now.
  if followed.completed {
    services.changes.record(open.project.id(), Outside::Tasks);
  }
  if let Some(turn) = followed.turn {
    answer(services, i18n, open, (turn, true)).await;
  }
  Ok(())
}

/// What git tells of `open`'s runs that succeeded, asked before the follower's lock and remembered for the runs it
/// found merged outside Orchester or with nothing to merge.
async fn check_merges(services: &Services, open: &OpenProject) -> Result<MergeChecks, Message> {
  let (root, programs) = (open.repo_root.clone(), services.programs.clone());
  let quiet = QUIET.lock().get(&root).cloned().unwrap_or_default();
  let mut checks = {
    let quiet = quiet.clone();
    background::run(move || waits::check_merges_in((&root, &programs), &quiet)).await??
  };
  QUIET
    .lock()
    .entry(open.repo_root.clone())
    .or_default()
    .extend(checks.quiet.iter().copied());
  checks.quiet.extend(quiet);
  Ok(checks)
}

/// Reads where the base branches of `open`'s unfinished runs are, before the follower's lock, and queues a notice for
/// each run a move left behind (`orchester_workspace::base_moved`); their timelines read them again when it did.
async fn observe_bases(services: &Services, open: &OpenProject) -> Result<(), Message> {
  let (root, programs) = (open.repo_root.clone(), services.programs.clone());
  let queued = background::run(move || base_moved::observe_in(&root, &programs)).await??;
  if queued > 0 {
    services.runners.base_notices_queued(&open.repo_root);
  }
  Ok(())
}

/// Follows `open`'s waits and run ends once, with what git told of its runs that succeeded (`checks`; `services`'
/// programs ask it for a run they do not tell), under the person's `switches`, and starting a turn only when the
/// runner is `attached`.
async fn keep(
  services: &Services,
  open: &OpenProject,
  checks: &MergeChecks,
  (switches, attached): (Switches, bool),
) -> Result<Followed, Message> {
  let (root, project, programs) = (open.repo_root.clone(), open.project.id(), services.programs.clone());
  let checks = checks.clone();
  let _keeping = KEEPING.lock().await;
  background::run(move || waits::follow_in((&root, project, &programs), &checks, (switches, attached))).await?
}

/// Starts the turn of the next message the person queued while the Orchestrator answered, once its conversation no
/// longer answers (`orchester_workspace::orchestrator::queue`); before any wait's turn, and only once the runner
/// attached.
async fn start_queued(services: &Services, i18n: I18n, open: &OpenProject) -> Result<(), Message> {
  let root = open.repo_root.clone();
  if !background::run(move || queue::next_ready_in(&root)).await?? {
    return Ok(());
  }
  if let Err(error) = services.runners.chat_runner(open).await {
    tracing::info!(%error, "no runner attached for a queued message's turn; it starts later");
    return Ok(());
  }
  let root = open.repo_root.clone();
  let started = {
    let _keeping = KEEPING.lock().await;
    background::run(move || queue::start_next_in(&root)).await??
  };
  if let Some(turn) = started {
    services.runners.chat_changed(&open.repo_root);
    answer(services, i18n, open, (turn, false)).await;
  }
  Ok(())
}

/// Serves the tools of `turn`, one Orchester started by itself (`on_its_own`) or a queued message's, and asks `open`'s
/// runner to answer it; a turn neither takes ends failed.
async fn answer(services: &Services, i18n: I18n, open: &OpenProject, (turn, on_its_own): (WaitTurn, bool)) {
  let scope = ChatScope {
    project: open.project.id(),
    name: open.project.name().as_str().to_owned(),
    root: open.repo_root.clone(),
    chat: turn.chat,
    on_its_own,
  };
  let tools = match services.chat_tools.serve_turn(services, i18n, (scope, turn.message)) {
    Ok(tools) => tools,
    Err(error) => {
      tracing::error!(%error, "the Orchestrator's tools could not be served for a wait's turn");
      send::end_wait_turn(&services.runners, open, turn.message, error.to_string()).await;
      return;
    }
  };
  match send::answer_wait(&services.runners, open, turn.message, tools).await {
    Err(SendError::NotSent(_)) => services.chat_tools.end(turn.message),
    Ok(()) | Err(SendError::Unconfirmed(_)) => {
      services
        .chat_tools
        .end_with_turn(services, open.repo_root.clone(), turn.message);
    }
  }
}

/// How often the project's conversations changed; changes of other projects wake the stream without a render.
fn chats(ctx: &mut Ctx, deps: (Runners, PathBuf)) -> u64 {
  let count = ctx.stream(deps, |(runners, root), emitter: StreamEmitter<u64, ()>| async move {
    let mut updates = runners.chats_of(&root);
    loop {
      let revision = *updates.borrow_and_update();
      if !emitter.emit(revision) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  count.state().get().data.unwrap_or(0)
}
