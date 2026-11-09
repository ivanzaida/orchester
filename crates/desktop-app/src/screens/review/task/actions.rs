//! Every change the review page makes, one at a time, off the UI thread (`tasks::review_threads`): a comment kept
//! unsent, a question kept asked and then handed to the project's runner at once, *Ask again*, *Reply*, a comment's
//! new words or its deletion, *Discard*, *Resolve*, *Viewed*, *Send to Implement* (the comments kept sending, then
//! handed to the runner), *Stop answering*, *Approve and merge* (which first deletes the comments not sent that the
//! person gives up; also *Try merge again* on a merge that conflicted) and *Ask the agent to resolve* (the run goes
//! back to Implement). The review is read again once the change is kept, and again once the runner took what was
//! handed to it. A question or comments the runner did not take are kept *Not asked* or *Not sent* with why, which the
//! thread and the tray show; any other refusal shows over the diff.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  conversation::MessageBody,
  id::{ResourceId, ReviewMessageId, ReviewThreadId, RunId},
  pull_request::feedback::LineAnchor,
  revision::Revision,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  messages::Message,
  runner::Runners,
  tasks::{
    resolve::RunWorktrees,
    review_threads::{self, ReviewRefusal, approve::approve_and_merge, ask, conflicts, send},
    worktree_removal::WorktreeRemover,
  },
  workspace::OpenProject,
};
use parking_lot::Mutex;

use super::state::{Closes, ReviewState};
use crate::{
  project::{ShownProject, hold::ReadHold},
  services::Services,
};

/// A change of the run's review.
#[derive(Clone, Debug)]
pub(super) enum Write {
  /// A change kept in the project alone.
  Keep(Kept),
  Ask(LineAnchor, Revision, MessageBody),
  AskAgain(ReviewThreadId, MessageBody),
  /// *Send to Implement*, the step running with the agent profile, or the run's own when there is none.
  Send(Option<ResourceId>),
  Stop(ReviewThreadId),
  /// *Approve and merge* at the head reviewed, the comments not sent given here discarded first; on a merge that
  /// conflicted, *Try merge again* (with none to discard).
  Approve(Revision, Vec<ReviewMessageId>),
  /// *Ask the agent to resolve* the conflicts of the approved merge: the run goes back to Implement, the step
  /// running with the agent profile, or the run's own when there is none.
  Resolve(Option<ResourceId>),
}

/// A change that does not go to the runner.
#[derive(Clone, Debug)]
pub(super) enum Kept {
  Comment(LineAnchor, Revision, MessageBody),
  Reply(ReviewThreadId, MessageBody),
  Edit(ReviewMessageId, MessageBody),
  /// Deletes these comments not sent yet: *Delete* one, *Discard* all.
  Delete(Vec<ReviewMessageId>),
  Resolve(ReviewThreadId, bool),
  Viewed(Revision, String, bool),
}

/// How a change went on: kept, or a merge that ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Landed {
  Kept,
  Approved,
  Resolving,
}

impl lurq::app::component::DevtoolsInspectable for Landed {}

/// What a change runs with.
#[derive(Clone)]
struct Reach {
  root: PathBuf,
  programs: Programs,
  runners: Runners,
  open: OpenProject,
  run: RunId,
  state: ReviewState,
  /// The box the change under way was written in, until it is kept.
  closes: Arc<Mutex<Option<Closes>>>,
  /// Holds a hand-off to the runner while QA captures it (`hold_hand_offs`).
  hand_offs: ReadHold,
}

/// The page's changes, one at a time: one asked for while another runs is refused, so none is dropped half-way.
#[derive(Clone)]
pub(super) struct Writes {
  action: FutureAction<Write, Landed, Message>,
  /// The box the change under way was written in, until it is kept.
  closes: Arc<Mutex<Option<Closes>>>,
}

impl PartialEq for Writes {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl Writes {
  pub fn new(ctx: &mut Ctx, project: &ShownProject, run: RunId, state: &ReviewState) -> Self {
    let services = Services::of(ctx);
    let closes: Arc<Mutex<Option<Closes>>> = Arc::default();
    let reach = Reach {
      root: project.open.repo_root.clone(),
      programs: services.programs,
      runners: services.runners,
      open: project.open.clone(),
      run,
      state: state.clone(),
      closes: closes.clone(),
      hand_offs: services.hand_offs,
    };
    let action = ctx.future_action(move |write: Write| {
      let reach = reach.clone();
      async move { apply(&reach, write).await.map_err(|refusal| refusal.message()) }
    });
    let (state, feed, closing) = (state.clone(), project.feed.clone(), closes.clone());
    ctx.watch(&action.state(), move |landed| {
      if landed.is_pending() {
        return;
      }
      let closes = closing.lock().take();
      if let Some(error) = &landed.error {
        state.written.update(|count| *count += 1);
        state.failure.set(Some(error.clone()));
        return;
      }
      state.failure.set(None);
      state.kept(closes);
      match landed.data {
        Some(Landed::Approved) => {
          // A merge ran: the project's tasks and merges are read again.
          feed.saved();
        }
        Some(Landed::Resolving) => {
          // The run's work went back to a step that resolves the merge, so what that merge found no longer stands.
          feed.jobs.of(run).forget_merge_failure();
          feed.saved();
        }
        _ => {}
      }
    });
    Self { action, closes }
  }

  /// Runs `write`, unless another change runs.
  pub fn run(&self, write: Write) {
    if !self.busy_untracked() {
      self.action.run(write);
    }
  }

  /// Runs `write`, written in the box `closes`, which closes once it was kept; unless another change runs.
  pub fn run_closing(&self, write: Write, closes: Closes) {
    if !self.busy_untracked() {
      *self.closes.lock() = Some(closes);
      self.action.run(write);
    }
  }

  /// Whether a change runs; reading it renders again when that changes.
  pub fn busy(&self) -> bool {
    self.action.state().get().is_pending()
  }

  /// Whether a change runs, without rendering again when that changes.
  pub fn busy_untracked(&self) -> bool {
    self.action.state().get_untracked().is_pending()
  }
}

/// Applies `write`, handing what goes to Implement to the runner once it is kept: the page reads the review again in
/// between, so a question shows *Sent to Implement* and comments *Sending…* while the runner takes them.
async fn apply(reach: &Reach, write: Write) -> Result<Landed, ReviewRefusal> {
  let root = reach.root.clone();
  let (now, run) = (Timestamp::now(), reach.run);
  match write {
    Write::Keep(kept) => {
      blocking(move || keep(&root, run, kept, now)).await?;
      Ok(Landed::Kept)
    }
    Write::Ask(anchor, head, body) => {
      let asking = blocking(move || ask::start(&root, run, (anchor, head), &body, now)).await?;
      shown(reach).await;
      handed(ask::ask(&reach.runners, &reach.open, &asking).await)
    }
    Write::AskAgain(thread, body) => {
      let asking = blocking(move || ask::again(&root, thread, &body, now)).await?;
      shown(reach).await;
      handed(ask::ask(&reach.runners, &reach.open, &asking).await)
    }
    Write::Send(profile) => {
      let worktrees = RunWorktrees::through_runner(&reach.runners, (&reach.root, reach.open.project.name().as_str()));
      let sending = blocking(move || send::begin(&root, &worktrees, run, profile)).await?;
      shown(reach).await;
      handed(send::send(&reach.runners, &reach.open, &sending).await)
    }
    Write::Stop(thread) => {
      ask::stop(&reach.runners, &reach.open, (run, thread))
        .await
        .map_err(ReviewRefusal::Store)?;
      Ok(Landed::Kept)
    }
    Write::Approve(reviewed, unsent) => {
      if !unsent.is_empty() {
        blocking(move || keep(&root, run, Kept::Delete(unsent), now)).await?;
      }
      let programs = reach.programs.clone();
      let worktrees =
        WorktreeRemover::through_runner(&reach.runners, (&reach.root, reach.open.project.name().as_str()));
      blocking(move || approve_and_merge(&worktrees, &programs, (run, &reviewed), now)).await?;
      Ok(Landed::Approved)
    }
    Write::Resolve(profile) => {
      let worktrees = RunWorktrees::through_runner(&reach.runners, (&reach.root, reach.open.project.name().as_str()));
      let command = blocking(move || conflicts::resolve::begin(&root, &worktrees, run, profile)).await?;
      reach.hand_offs.released().await;
      conflicts::resolve::send(&reach.runners, &reach.open, command).await?;
      Ok(Landed::Resolving)
    }
  }
}

/// What was kept before its hand-off to the runner is read again, the box it was written in closing once that read
/// shows it; the hand-off goes once QA does not hold it.
async fn shown(reach: &Reach) {
  reach.state.kept(reach.closes.lock().take());
  reach.hand_offs.released().await;
}

/// How handing a question or comments to the runner went: one the runner did not take was kept *Not asked* or *Not
/// sent*, which the thread and the tray tell.
fn handed(result: Result<(), ReviewRefusal>) -> Result<Landed, ReviewRefusal> {
  match result {
    Ok(()) | Err(ReviewRefusal::NotAsked(_) | ReviewRefusal::NotSent(_)) => Ok(Landed::Kept),
    Err(refusal) => Err(refusal),
  }
}

/// Keeps a change that does not go to the runner.
fn keep(root: &std::path::Path, run: RunId, kept: Kept, now: Timestamp) -> Result<(), ReviewRefusal> {
  match kept {
    Kept::Comment(anchor, head, body) => review_threads::comment(root, run, (anchor, head), &body, now).map(drop),
    Kept::Reply(thread, body) => review_threads::reply(root, thread, &body, now),
    Kept::Edit(message, body) => review_threads::edit(root, message, &body),
    Kept::Delete(messages) => messages
      .into_iter()
      .try_for_each(|message| review_threads::delete(root, message)),
    Kept::Resolve(thread, resolved) => review_threads::resolve(root, thread, resolved.then_some(now)),
    Kept::Viewed(head, path, viewed) => review_threads::viewed(root, (run, &head, &path), viewed.then_some(now)),
  }
}

/// `work` off the UI thread.
async fn blocking<T: Send + 'static>(
  work: impl FnOnce() -> Result<T, ReviewRefusal> + Send + 'static,
) -> Result<T, ReviewRefusal> {
  background::run(work).await.map_err(ReviewRefusal::Store)?
}
