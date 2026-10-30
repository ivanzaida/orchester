//! Work the Pull requests page starts off the UI thread: reading the stored list, syncing it, *Use this
//! account* followed by a sync, and handing a pull request's address to the browser. A sync is a quiet
//! operation of the status bar: one that read the list is listed as synced; one without a linked repository
//! asked GitHub nothing and is not listed; one GitHub refused is listed as not synced, a finished event
//! rather than a failure to dismiss, since the minute's sync would pin one every minute (the page's notice
//! says why); one that could not run at all (storage) is a failure.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureHandle},
  core::Signal,
};
use orchester_exec::{external, programs::Programs};
use orchester_workspace::{
  activity::operations,
  background,
  messages::Message,
  pull_requests::{
    InboxError,
    list::{PullRequestList, load_list_in, sync_list_in, use_active_account_list_in},
  },
  workspace::OpenProject,
};

use crate::services::Services;

pub(super) type ListAction = FutureAction<PathBuf, PullRequestList, Message>;
/// A pull request's number and address.
pub(super) type OpenAction = FutureAction<(u64, Arc<str>), (), Message>;

type Work = fn(&std::path::Path, &Programs) -> Result<PullRequestList, InboxError>;

/// The page's work, made once when the page opens.
#[derive(Clone)]
pub(super) struct Actions {
  pub sync: ListAction,
  pub account: ListAction,
  pub open: OpenAction,
}

impl Actions {
  pub fn of(ctx: &mut Ctx, open: &OpenProject) -> Self {
    Self {
      sync: list_action(ctx, open, sync_list_in),
      account: list_action(ctx, open, use_active_account_list_in),
      open: open_action(ctx, open),
    }
  }

  /// Keeps what the latest sync returned in `latest`, and why it could not run in `failed`.
  pub fn watch(&self, ctx: &mut Ctx, latest: &Signal<Option<PullRequestList>>, failed: &Signal<Option<Message>>) {
    for action in [&self.sync, &self.account] {
      let (latest, failed) = (latest.clone(), failed.clone());
      ctx.watch(&action.state(), move |state| {
        if state.is_pending() {
          return;
        }
        if let Some(error) = &state.error {
          failed.set(Some(error.clone()));
        } else if let Some(list) = &state.data {
          failed.set(None);
          latest.set(Some(list.clone()));
        }
      });
    }
  }

  /// Whether a sync or *Use this account* runs; subscribes the caller.
  pub fn running(&self) -> bool {
    self.sync.state().get().is_pending() || self.account.state().get().is_pending()
  }

  /// Whether a sync or *Use this account* runs, without subscribing.
  pub fn running_now(&self) -> bool {
    self.sync.state().get_untracked().is_pending() || self.account.state().get_untracked().is_pending()
  }
}

/// The stored list, read when the page opens and again for each `reread`, after reading it failed.
pub(super) fn load(ctx: &mut Ctx, open: &OpenProject, reread: u64) -> FutureHandle<PullRequestList, Message> {
  let hold = Services::of(ctx).hold;
  ctx.future((open.repo_root.clone(), reread), move |(repo_root, _)| {
    let hold = hold.clone();
    async move {
      hold.released().await;
      background::run(move || load_list_in(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's pull requests");
          error.message()
        })
    }
  })
}

fn list_action(ctx: &mut Ctx, open: &OpenProject, work: Work) -> ListAction {
  let Services { activity, programs, .. } = Services::of(ctx);
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
        // Nothing was asked of GitHub, so nothing is listed; the page says why.
        Ok(PullRequestList::Unlinked) => drop(running),
        // Replaces an earlier "synced" event, so the status bar never keeps claiming a sync the latest
        // attempt did not repeat; the page's notice says why.
        Ok(PullRequestList::Linked(linked)) if linked.failure.is_some() => {
          running.finish_as(orchester_workspace::pull_requests::not_synced());
        }
        Ok(PullRequestList::Linked(_)) => running.finish(),
        Err(reason) => running.fail(reason.clone()),
      }
      done
    }
  })
}

fn open_action(ctx: &mut Ctx, open: &OpenProject) -> OpenAction {
  let activity = Services::of(ctx).activity;
  let open = open.clone();
  ctx.future_action(move |(number, url): (u64, Arc<str>)| {
    let (activity, open) = (activity.clone(), open.clone());
    async move {
      let work = async move {
        background::run(move || external::open_url(&url))
          .await?
          .map_err(|error| {
            tracing::warn!(%error, number, "cannot open the pull request in the browser");
            orchester_workspace::pull_requests::open_failed()
          })
      };
      activity.track(operations::pull_request_open(&open, number), work).await
    }
  })
}
