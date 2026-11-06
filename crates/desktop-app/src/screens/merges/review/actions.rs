//! Every change the review page makes, one at a time, off the UI thread (`tasks::review_threads`): a comment kept
//! unsent, a question kept asked and then handed to the project's runner at once, *Ask again*, *Reply*, a comment's
//! new words or its deletion, *Discard*, *Resolve*, *Viewed*, *Send to Implement* (the comments kept sending, then
//! handed to the runner), *Stop answering* and *Approve and merge*. The review is read again once the change is kept,
//! and again once the runner took what was handed to it. A question or comments the runner did not take are kept *Not
//! asked* or *Not sent* with why, which the thread and the tray show; any other refusal shows over the diff.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{
  conversation::MessageBody,
  id::{ReviewMessageId, ReviewThreadId, RunId},
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
    review_threads::{self, ReviewRefusal, approve::approve_and_merge, ask, send},
    worktree_removal::WorktreeRemover,
  },
  workspace::OpenProject,
};
use parking_lot::Mutex;

use super::state::ReviewState;
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
  Send,
  Stop(ReviewThreadId),
  /// *Approve and merge* at the head reviewed.
  Approve(Revision),
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
  written: Signal<u64>,
  /// Holds a hand-off to the runner while QA captures it (`hold_hand_offs`).
  hand_offs: ReadHold,
}

/// The box a change's words were written in, which closes once the change was kept; it stays open, its words with
/// it, when the change was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Closes {
  /// The comment box under the chosen lines.
  Box,
  /// The box inside a thread.
  InThread,
}

/// The page's changes, one at a time: one asked for while another runs is refused, so none is dropped half-way.
#[derive(Clone)]
pub(super) struct Writes {
  action: FutureAction<Write, Landed, Message>,
  /// The box the change under way was written in.
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
    let reach = Reach {
      root: project.open.repo_root.clone(),
      programs: services.programs,
      runners: services.runners,
      open: project.open.clone(),
      run,
      written: state.written.clone(),
      hand_offs: services.hand_offs,
    };
    let action = ctx.future_action(move |write: Write| {
      let reach = reach.clone();
      async move { apply(&reach, write).await.map_err(|refusal| refusal.message()) }
    });
    let closes: Arc<Mutex<Option<Closes>>> = Arc::default();
    let (state, feed, closing) = (state.clone(), project.feed.clone(), closes.clone());
    ctx.watch(&action.state(), move |landed| {
      if landed.is_pending() {
        return;
      }
      state.written.update(|count| *count += 1);
      let closes = closing.lock().take();
      if let Some(error) = &landed.error {
        state.failure.set(Some(error.clone()));
        return;
      }
      state.failure.set(None);
      match closes {
        Some(Closes::Box) => state.close(true),
        Some(Closes::InThread) => state.close_in_thread(),
        None => {}
      }
      if landed.data == Some(Landed::Approved) {
        // A merge ran: the project's tasks and merges are read again.
        feed.saved();
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
    Write::Send => {
      let worktrees = RunWorktrees::through_runner(&reach.runners, (&reach.root, reach.open.project.name().as_str()));
      let sending = blocking(move || send::begin(&root, &worktrees, run)).await?;
      shown(reach).await;
      handed(send::send(&reach.runners, &reach.open, &sending).await)
    }
    Write::Stop(thread) => {
      ask::stop(&reach.runners, &reach.open, (run, thread))
        .await
        .map_err(ReviewRefusal::Store)?;
      Ok(Landed::Kept)
    }
    Write::Approve(reviewed) => {
      let programs = reach.programs.clone();
      let worktrees =
        WorktreeRemover::through_runner(&reach.runners, (&reach.root, reach.open.project.name().as_str()));
      blocking(move || approve_and_merge(&worktrees, &programs, (run, &reviewed), now)).await?;
      Ok(Landed::Approved)
    }
  }
}

/// What was kept before its hand-off to the runner is read again; the hand-off goes once QA does not hold it.
async fn shown(reach: &Reach) {
  reach.written.update(|count| *count += 1);
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
