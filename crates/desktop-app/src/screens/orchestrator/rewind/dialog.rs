//! The question before going back (09c `z7sgJ`, `Modal layer` `YVDpJ`): History's delete dialog, 480 wide, in the
//! warning tone with `message-square-x`. For an edit, *Send the edit and drop 3 messages?* over what goes (its edit
//! replaces the message, so many after it leave, so many the person's and so many the Orchestrator's) and what stays
//! (what the Orchestrator did after it, which it is told exists), then *Cancel* and *Drop 3 and send*; for *Rewind to
//! here*, *Rewind to before 16:04 and drop 4 messages?* with *Rewind*. Enter confirms and Esc cancels. A rewind that
//! failed says why under the lines and the dialog stays; one that went back closes it.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::orchestrator::{ChatAction, RewindPoint};
use orchester_workspace::display;

use super::{Asked, Failed};
use crate::{
  project::ShownProject,
  screens::orchestrator::{View, action, draft::Conversation},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone, line, lines},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

/// The dialog's width (`YVDpJ`).
const WIDTH: f32 = 480.0;

/// The question the screen asks before going back, while it asks one.
pub(in crate::screens::orchestrator) fn dialog(ctx: &mut Ctx, view: &View, project: &ShownProject) -> Option<Element> {
  let state = &view.rewind;
  let asked = state.asked.get().filter(|_| state.open.get())?;
  let on = view.plan_only.on_now(Some(Conversation::Chat(asked.chat)));
  let mode = (view.plan_only.mode(on), on);
  let pending = state.action.state().get().is_pending();
  let time = display::clock(asked.at);
  let edit = asked.edit.is_some();
  let count = if edit {
    asked.dropped.count().saturating_sub(1)
  } else {
    asked.dropped.count()
  };
  let confirm = {
    let (state, project, asked) = (state.clone(), project.open.clone(), asked.clone());
    move || state.confirm(&project, &asked, mode)
  };
  let (glyph, label) = if edit {
    let keys = (
      "orchestrator.edit.confirm.send.one",
      "orchestrator.edit.confirm.send.other",
    );
    (Icon::Send, plural(ctx, keys, count, &[]))
  } else {
    (Icon::History, ctx.t("orchestrator.rewind.confirm.go"))
  };
  let button = ActionButton::new("orchestrator-rewind-confirm", ButtonKind::Primary, label).icon(glyph);
  let button = if pending {
    button.loading()
  } else {
    button.on_click(confirm.clone())
  };
  let failure = match state.action.state().get().error {
    Some(Failed::Unchanged(why)) if !pending => {
      let reason = message_text(ctx, &why);
      Some(
        field_error(
          "orchestrator-rewind-error",
          &ctx.t_args("orchestrator.rewind.failed", [("reason", &*reason)]),
        )
        .into(),
      )
    }
    _ => None,
  };
  let enter = if edit {
    ctx.t("orchestrator.edit.confirm.enterSends")
  } else {
    ctx.t("orchestrator.rewind.confirm.enterRewinds")
  };
  let dialog = Dialog::new(
    "orchestrator-rewind",
    Icon::MessageSquareX,
    title(ctx, &asked, &time, count),
    ctx.t("orchestrator.edit.cancel"),
  )
  .tone(Tone::Warning)
  .width(WIDTH)
  .part(what(ctx, &asked, (&time, count)));
  Some(
    dialog
      .on_enter(enter, confirm)
      .action(button)
      .under(failure)
      .render(ctx, &state.open),
  )
}

/// The question: sending the edit, or going back before or after the message of `time`, and how many messages that
/// drops.
fn title(ctx: &Ctx, asked: &Asked, time: &str, count: u32) -> Arc<str> {
  let keys = match (asked.edit.is_some(), asked.point) {
    (true, _) => (
      "orchestrator.edit.confirm.title.one",
      "orchestrator.edit.confirm.title.other",
    ),
    (false, RewindPoint::Before(_)) => (
      "orchestrator.rewind.confirm.before.one",
      "orchestrator.rewind.confirm.before.other",
    ),
    (false, RewindPoint::After { .. }) => (
      "orchestrator.rewind.confirm.after.one",
      "orchestrator.rewind.confirm.after.other",
    ),
  };
  plural(ctx, keys, count, &[("time", time)])
}

/// What goes and what stays.
fn what(ctx: &Ctx, asked: &Asked, (time, count): (&str, u32)) -> lurq::components::Column {
  let dropped = &asked.dropped;
  // An edit replaces the message: it is not one of those that leave.
  let mine = if asked.edit.is_some() {
    dropped.mine.saturating_sub(1)
  } else {
    dropped.mine
  };
  let args = [
    ("time", time.to_owned()),
    ("count", count.to_string()),
    ("mine", mine.to_string()),
    ("theirs", dropped.theirs.to_string()),
  ];
  let goes = match (asked.edit.is_some(), asked.point, count) {
    (true, _, 1) => ctx.t_args("orchestrator.edit.confirm.whatOne", args.clone()),
    (true, ..) => ctx.t_args("orchestrator.edit.confirm.what", args.clone()),
    (false, RewindPoint::Before(_), _) => ctx.t_args("orchestrator.rewind.confirm.whatBefore", args.clone()),
    (false, RewindPoint::After { .. }, _) => ctx.t_args("orchestrator.rewind.confirm.whatAfter", args.clone()),
  };
  let stays = if dropped.done.is_empty() {
    ctx.t_args("orchestrator.rewind.staysNone", [("time", time)])
  } else {
    ctx.t_args(
      "orchestrator.rewind.stays",
      [("time", time.to_owned()), ("list", done_list(ctx, &dropped.done))],
    )
  };
  lines(
    "orchestrator-rewind-does",
    [
      line("orchestrator-rewind-goes", Icon::MessagesSquare, &goes),
      line("orchestrator-rewind-stays", Icon::ListChecks, &stays),
    ],
  )
}

/// What `done` did, each as its line says it, after one another: *Created task KON-31, Started run KON-31*.
pub(super) fn done_list(ctx: &Ctx, done: &[ChatAction]) -> String {
  let items: Vec<String> = done
    .iter()
    .map(|action| {
      let task = action.task.as_deref().unwrap_or_default();
      let what = ctx.t_args(
        action::words(&action.tool).0,
        [("task", task), ("tool", action.tool.as_str())],
      );
      ctx
        .t_args("orchestrator.rewind.staysItem", [("what", &*what), ("task", task)])
        .trim()
        .to_owned()
    })
    .collect();
  items.join(", ")
}

/// The words of the key for one or for `count` other than one, with `args`.
pub(super) fn plural(ctx: &Ctx, (one, other): (&str, &str), count: u32, args: &[(&str, &str)]) -> Arc<str> {
  let count_words = count.to_string();
  let mut all: Vec<(&str, &str)> = args.to_vec();
  all.push(("count", &count_words));
  ctx.t_args(if count == 1 { one } else { other }, all)
}
