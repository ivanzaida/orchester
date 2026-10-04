//! Work the pull request screen starts off the UI thread: reading what is stored, refreshing the
//! pull request and its diff, and opening the pull request in the browser.
//! A refresh is a quiet operation of the status bar activity, listed like an inbox sync: "read",
//! "not read" when GitHub refused (the screen's notice says why), failed when it could not run,
//! and not at all without a linked repository.

use lurq::{
  app::ctx::{Ctx, FutureAction, FutureHandle},
  core::Signal,
};
use orchester_core::time::Timestamp;

use super::super::actions::{OpenAction, open_action};
use crate::{
  activity::{Activity, operations},
  background,
  messages::Message,
  programs::Programs,
  pull_requests::detail::{Detail, load_detail_in, refresh_detail_in},
  workspace::OpenProject,
};

pub type RefreshAction = FutureAction<(), Detail, Message>;

/// The screen's futures, requested in this order in `create` and `render`.
#[derive(Clone)]
pub struct Actions {
  pub refresh: RefreshAction,
  pub open: OpenAction,
}

impl Actions {
  pub fn of(ctx: &mut Ctx, open: &OpenProject, number: u64) -> Self {
    Self {
      refresh: refresh_action(ctx, open, number),
      open: open_action(ctx, open),
    }
  }

  /// Keeps what the latest refresh returned in `latest`, and why it could not run in `failed`.
  pub fn watch(&self, ctx: &mut Ctx, latest: &Signal<Option<Detail>>, failed: &Signal<Option<Message>>) {
    let (latest, failed) = (latest.clone(), failed.clone());
    ctx.watch(&self.refresh.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if let Some(error) = &state.error {
        failed.set(Some(error.clone()));
      } else if let Some(detail) = &state.data {
        failed.set(None);
        latest.set(Some(detail.clone()));
      }
    });
  }

  /// Whether a refresh runs; subscribes the caller.
  pub fn running(&self) -> bool {
    self.refresh.state().get().is_pending()
  }

  /// Whether a refresh may start at `now`: none runs and no rate limit holds it back.
  pub fn may_refresh(&self, detail: Option<&Detail>, now: Timestamp) -> bool {
    let paused = match detail {
      Some(Detail::Linked(linked)) => linked.latest_failure().is_some_and(|failure| failure.is_paused(now)),
      Some(Detail::Unlinked) | None => false,
    };
    !self.refresh.state().get_untracked().is_pending() && !paused
  }
}

/// The stored pull request and diff, read once when the screen opens.
pub fn load(ctx: &mut Ctx, open: &OpenProject, number: u64) -> FutureHandle<Detail, Message> {
  ctx.future((open.repo_root.clone(), number), |(repo_root, number)| async move {
    background::run(move || load_detail_in(&repo_root, number))
      .await?
      .map_err(|error| {
        tracing::error!(%error, number, "cannot read the stored pull request");
        error.message()
      })
  })
}

fn refresh_action(ctx: &mut Ctx, open: &OpenProject, number: u64) -> RefreshAction {
  let activity = Activity::of(ctx);
  let programs = ctx
    .use_context::<Programs>()
    .expect("the root provides Programs before mounting screens");
  let open = open.clone();
  ctx.future_action(move |()| {
    let (activity, programs, open) = (activity.clone(), programs.clone(), open.clone());
    async move {
      let repo_root = open.repo_root.clone();
      let running = activity.start(operations::pull_request_refresh(&open, number));
      let done = background::run(move || refresh_detail_in(&repo_root, number, &programs))
        .await
        .and_then(|result| {
          result.map_err(|error| {
            tracing::error!(%error, number, "cannot refresh the pull request");
            error.message()
          })
        });
      match &done {
        // Nothing was asked of GitHub, so nothing is listed; the screen says why.
        Ok(Detail::Unlinked) => drop(running),
        Ok(Detail::Linked(linked)) if linked.failure.is_some() => {
          running.finish_as(
            Message::new("statusBar.operations.pullRequestRefresh.notRead").with("name", number.to_string()),
          );
        }
        Ok(_) => running.finish(),
        Err(reason) => running.fail(reason.clone()),
      }
      done
    }
  })
}
