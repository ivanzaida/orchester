//! Under the row, where a change of the run's permissions stands (`LJmTH`, `D8qjLz`): kept for the
//! next invocation (`A3h7B4`); pending with *Keep for next invocation instead*; restarting, or not
//! restarted, after the checkpoint (`v0wd4R`, `R5mCao`); applied. The saved and pending notices
//! name the change ("pnpm build added") when what the agent launched with is known.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Row,
  layout::layout_kind::Justify,
  node::{Element, dimension::Dimension},
};
use orchester_core::{revision::Revision, time::Timestamp};

use super::{KEEP_BUTTON_HEIGHT, KeepAction, RunPermissionsProps, policy::change_words};
use crate::{
  display,
  screens::project::task::heading::agent_of,
  tasks::{detail::ChangeState, permissions::InvocationPolicy},
  ui::{button::ActionButton, field::field_error, icon::Icon, message_text, notice::Notice},
};

/// The notices for the row's change state, if any.
pub(super) fn state_notice(ctx: &Ctx, props: &RunPermissionsProps, keep: &KeepAction) -> Vec<Element> {
  match &props.permissions.state {
    Some(ChangeState::NextInvocation) => vec![saved_notice(ctx, props)],
    Some(ChangeState::Pending) => pending_notice(ctx, props, keep),
    // The agent has already stopped, so the change can no longer be kept for the next invocation
    // (the runner refuses it): the notice says what happens instead and offers no action.
    Some(ChangeState::Restarting { checkpoint }) => vec![restart_notice(ctx, props, checkpoint.as_ref(), false)],
    Some(ChangeState::RestartFailed { checkpoint }) => vec![restart_notice(ctx, props, checkpoint.as_ref(), true)],
    Some(ChangeState::Applied {
      at,
      checkpoint,
      invocation,
    }) => vec![applied_notice(ctx, props, *at, checkpoint.as_ref(), *invocation)],
    None => Vec::new(),
  }
}

/// What changes for the next invocation, against what the agent launched with.
fn change(ctx: &Ctx, props: &RunPermissionsProps) -> Option<Arc<str>> {
  let launched = match &props.permissions.invocation_policy {
    InvocationPolicy::Active(launched) | InvocationPolicy::Last(launched) => launched,
    InvocationPolicy::Starting
    | InvocationPolicy::Unknown
    | InvocationPolicy::NotStarted
    | InvocationPolicy::NoInvocation => return None,
  };
  change_words(ctx, launched, props.permissions.next_launch.as_ref()?)
}

/// "Saved for the next invocation · pnpm build added" (`A3h7B4`).
fn saved_notice(ctx: &Ctx, props: &RunPermissionsProps) -> Element {
  let title = match change(ctx, props) {
    Some(change) => ctx.t_args("runs.permissions.nextTitleChange", [("change", &*change)]),
    None => ctx.t("runs.permissions.nextTitle"),
  };
  let run = props.run_number.to_string();
  Notice::new(
    Icon::BookmarkCheck,
    title,
    ctx.t_args(
      "runs.permissions.nextDetail",
      [("agent", agent_of(&props.step)), ("run", run.as_str())],
    ),
  )
  .id("task-permissions-next")
  .into()
}

/// "Applying pnpm build added at the next checkpoint…" with *Keep for next invocation instead*.
fn pending_notice(ctx: &Ctx, props: &RunPermissionsProps, keep: &KeepAction) -> Vec<Element> {
  let (agent, step) = (agent_of(&props.step), &*props.step.name);
  let state = keep.state().get();
  let pending = state.is_pending();
  let (keep, run, profile) = (keep.clone(), props.run, props.permissions.profile_id);
  let button = ActionButton::new(ctx.t("runs.permissions.keepForNext"))
    .id("task-permissions-keep")
    .icon(Icon::Undo2)
    .compact()
    .height(Dimension::Px(KEEP_BUTTON_HEIGHT));
  let button = if pending {
    button.disabled()
  } else {
    button.on_click(move || keep.run((run, profile)))
  };
  let title = match change(ctx, props) {
    Some(change) => ctx.t_args("runs.permissions.pendingTitleChange", [("change", &*change)]),
    None => ctx.t("runs.permissions.pendingTitle"),
  };
  let mut elements: Vec<Element> = vec![
    Notice::new(
      Icon::Loader,
      title,
      ctx.t_args("runs.permissions.pendingDetail", [("agent", agent), ("step", step)]),
    )
    .id("task-permissions-pending")
    .into(),
  ];
  elements.extend(
    state
      .error
      .filter(|_| !pending)
      .map(|error| field_error(&message_text(ctx, &error))),
  );
  elements.push(
    Row::new()
      .width(Dimension::Pct(100.0))
      .justify(Justify::End)
      .child(button.render(ctx))
      .into(),
  );
  elements
}

/// "Restarting with the new permissions…" (`v0wd4R`), or, when the restart did not launch the
/// agent, "New permissions not active: Builder did not restart" (`R5mCao`); neither offers an action.
fn restart_notice(ctx: &Ctx, props: &RunPermissionsProps, checkpoint: Option<&Revision>, failed: bool) -> Element {
  let (agent, step) = (agent_of(&props.step), &*props.step.name);
  let detail = match (checkpoint, failed) {
    (Some(revision), false) => ctx.t_args(
      "runs.permissions.restartingDetail",
      [
        ("agent", agent),
        ("step", step),
        ("checkpoint", display::short_revision(revision.as_str())),
      ],
    ),
    (None, false) => ctx.t_args(
      "runs.permissions.restartingDetailNoCheckpoint",
      [("agent", agent), ("step", step)],
    ),
    (Some(revision), true) => ctx.t_args(
      "runs.permissions.restartFailedDetail",
      [
        ("agent", agent),
        ("step", step),
        ("checkpoint", display::short_revision(revision.as_str())),
      ],
    ),
    (None, true) => ctx.t_args(
      "runs.permissions.restartFailedDetailNoCheckpoint",
      [("agent", agent), ("step", step)],
    ),
  };
  let notice = if failed {
    Notice::new(
      Icon::TriangleAlert,
      ctx.t_args("runs.permissions.restartFailedTitle", [("agent", agent)]),
      detail,
    )
    .id("task-permissions-restart-failed")
  } else {
    Notice::new(Icon::RefreshCw, ctx.t("runs.permissions.restartingTitle"), detail).id("task-permissions-restarting")
  };
  notice.into()
}

/// "New permissions active since 12:09", with the checkpoint (when there was one) and the invocation.
fn applied_notice(
  ctx: &Ctx,
  props: &RunPermissionsProps,
  at: Timestamp,
  checkpoint: Option<&Revision>,
  invocation: u32,
) -> Element {
  let (agent, step) = (agent_of(&props.step), &*props.step.name);
  let invocation = invocation.to_string();
  let detail = match checkpoint {
    Some(revision) => ctx.t_args(
      "runs.permissions.appliedDetail",
      [
        ("agent", agent),
        ("step", step),
        ("checkpoint", display::short_revision(revision.as_str())),
        ("invocation", invocation.as_str()),
      ],
    ),
    None => ctx.t_args(
      "runs.permissions.appliedDetailNoCheckpoint",
      [("agent", agent), ("step", step), ("invocation", invocation.as_str())],
    ),
  };
  Notice::new(
    Icon::Check,
    ctx.t_args("runs.permissions.appliedTitle", [("time", display::clock(at).as_str())]),
    detail,
  )
  .id("task-permissions-applied")
  .into()
}
