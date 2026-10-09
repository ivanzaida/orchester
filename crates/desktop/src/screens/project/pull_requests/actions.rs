//! Work the Pull requests screen starts off the UI thread: reading the stored list, syncing,
//! *Use this account* followed by a sync, and opening a pull request in the browser. Syncs are
//! quiet operations of the status bar activity: a sync that read the list is listed as "synced";
//! one without a linked repository asked GitHub nothing and is not listed at all;
//! one GitHub refused is listed as "not synced" (a finished event, not a failure to dismiss, since
//! the minute timer would pin one every minute; the screen's notice says why); one that could not
//! run at all (storage) is a failure.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureHandle},
  core::Signal,
};
use orchester_core::time::Timestamp;

use crate::{
  activity::operations,
  background, external,
  messages::Message,
  programs::Programs,
  pull_requests::{Inbox, InboxError, load_in, sync_in, use_active_account_in},
  workspace::OpenProject,
};

pub type InboxAction = FutureAction<PathBuf, Inbox, Message>;
/// A pull request's number and address.
pub type OpenAction = FutureAction<(u64, Arc<str>), (), Message>;

type Work = fn(&std::path::Path, &Programs) -> Result<Inbox, InboxError>;

/// The screen's futures, requested in this order in `create` and `render`.
#[derive(Clone)]
pub struct Actions {
  pub sync: InboxAction,
  pub account: InboxAction,
  pub open: OpenAction,
}

impl Actions {
  pub fn of(ctx: &mut Ctx, open: &OpenProject) -> Self {
    Self {
      sync: inbox_action(ctx, open, sync_in),
      account: inbox_action(ctx, open, use_active_account_in),
      open: open_action(ctx, open),
    }
  }

  /// Keeps what the latest sync returned in `latest`, and why it could not run in `failed`.
  pub fn watch(&self, ctx: &mut Ctx, latest: &Signal<Option<Inbox>>, failed: &Signal<Option<Message>>) {
    for action in [&self.sync, &self.account] {
      let (latest, failed) = (latest.clone(), failed.clone());
      ctx.watch(&action.state(), move |state| {
        if state.is_pending() {
          return;
        }
        if let Some(error) = &state.error {
          failed.set(Some(error.clone()));
        } else if let Some(inbox) = &state.data {
          failed.set(None);
          latest.set(Some(inbox.clone()));
        }
      });
    }
  }

  /// Whether a sync or *Use this account* runs; subscribes the caller.
  pub fn running(&self) -> bool {
    self.sync.state().get().is_pending() || self.account.state().get().is_pending()
  }

  /// Whether a sync may start at `now`: nothing runs and no rate limit holds syncs back.
  pub fn may_sync(&self, inbox: Option<&Inbox>, now: Timestamp) -> bool {
    let running = self.sync.state().get_untracked().is_pending() || self.account.state().get_untracked().is_pending();
    let paused = match inbox {
      Some(Inbox::Linked(linked)) => linked.failure.as_ref().is_some_and(|failure| failure.is_paused(now)),
      Some(Inbox::Unlinked) | None => false,
    };
    !running && !paused
  }
}

/// The stored list, read once when the screen opens.
pub fn load(ctx: &mut Ctx, open: &OpenProject) -> FutureHandle<Inbox, Message> {
  ctx.future(open.repo_root.clone(), |repo_root| async move {
    background::run(move || load_in(&repo_root)).await?.map_err(|error| {
      tracing::error!(%error, "cannot read the project's pull requests");
      error.message()
    })
  })
}

fn inbox_action(ctx: &mut Ctx, open: &OpenProject, work: Work) -> InboxAction {
  let activity = crate::activity::of(ctx);
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let open = open.clone();
  ctx.future_action(move |repo_root: PathBuf| {
    let (activity, programs, open) = (activity.clone(), programs.clone(), open.clone());
    async move {
      let running = activity.start(operations::pull_request_sync(&open));
      let done = background::run(move || work(&repo_root, &programs))
        .await
        .and_then(|result| {
          result.map_err(|error| {
            tracing::error!(%error, "cannot sync the project's pull requests");
            error.message()
          })
        });
      match &done {
        // Nothing was asked of GitHub, so nothing is listed; the screen says why (08.D).
        Ok(Inbox::Unlinked) => drop(running),
        // Replaces an earlier "synced" event, so the status bar never keeps claiming a sync
        // that the latest attempt did not repeat; the screen's notice says why.
        Ok(Inbox::Linked(linked)) if linked.failure.is_some() => {
          running.finish_as(Message::new("statusBar.operations.pullRequestSync.notSynced"));
        }
        Ok(_) => running.finish(),
        Err(reason) => running.fail(reason.clone()),
      }
      done
    }
  })
}

pub fn open_action(ctx: &mut Ctx, open: &OpenProject) -> OpenAction {
  let activity = crate::activity::of(ctx);
  let open = open.clone();
  ctx.future_action(move |(number, url): (u64, Arc<str>)| {
    let (activity, open) = (activity.clone(), open.clone());
    async move {
      let work = async move {
        background::run(move || external::open_url(&url))
          .await?
          .map_err(|error| {
            tracing::error!(%error, number, "cannot open the pull request in the browser");
            Message::new("errors.openInBrowser")
          })
      };
      activity.track(operations::pull_request_open(&open, number), work).await
    }
  })
}
