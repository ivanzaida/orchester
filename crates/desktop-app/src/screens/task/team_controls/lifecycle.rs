//! A team task's lifecycle asks on its page (decision D4, `Task - v2` `jDN0r` rows 14–17,
//! `docs/design/team-authority.md`): what each ask changes ([`carry_out`]), Mark as blocked's reason in a dialog of its
//! own ([`blocking_dialog`]; the design draws no frame for it) and Reopen's two choices, Back to Todo and Back to
//! Backlog, with their foot line ([`reopen_menu`], T4 `V26A3`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::Text,
  core::ElementRef,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::task::organization::WorkflowStatus;

use super::{TeamAsk, TeamControls, TeamTaskShown};
use crate::{
  screens::task::TaskContext,
  team::{
    act::{Acted, Acting},
    lifecycle::Change,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::Dialog,
    field::FieldInput,
    icon::Icon,
    menu::{MenuItem, menu_at_end, menu_panel, separator},
  },
};

/// The lifecycle change `ask` makes, with the blocked reason typed; `None` for an ask that is none.
fn change(ask: TeamAsk, reason: &str) -> Option<Change> {
  match ask {
    TeamAsk::Done => Some(Change::Done),
    TeamAsk::Cancel => Some(Change::Cancel),
    TeamAsk::BackToTodo => Some(Change::MoveTo(WorkflowStatus::Todo)),
    TeamAsk::BackToBacklog => Some(Change::MoveTo(WorkflowStatus::Backlog)),
    TeamAsk::Block => Some(Change::Block(reason.to_owned())),
    TeamAsk::ClearBlocked => Some(Change::ClearBlocked),
    TeamAsk::Claim | TeamAsk::TakeOver | TeamAsk::Release | TeamAsk::Run | TeamAsk::Dismiss => None,
  }
}

/// Carries out the lifecycle ask on the task as the page shows it, on the revision it read; the team server decides it
/// by the person's claim and role.
pub(super) async fn carry_out(ask: TeamAsk, (shown, reason): (&TeamTaskShown, String), acting: &Acting) -> Acted {
  let Some(change) = change(ask, &reason) else {
    return Acted::Failed;
  };
  match change.action(&shown.task.task, shown.task.revision) {
    Ok(action) => acting.act(&shown.source, action).await.0,
    // A blank reason never reaches the server: the dialog does not offer it.
    Err(error) => {
      tracing::warn!(%error, "a lifecycle change was refused before it was sent");
      Acted::Failed
    }
  }
}

const BLOCK: &str = "task-block-confirm";

/// Mark as blocked's question: the reason, which is not blank. Open while the person asked; closes with the ask.
pub fn blocking_dialog(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let controls = TeamControls::of(ctx)?;
  if !controls.blocking.get() {
    return None;
  }
  let key = task.key.to_string();
  let typed = controls.reason.get();
  let reason = FieldInput::new(
    "task-block-reason",
    ctx.t("summary.team.lifecycle.block.reason"),
    controls.reason.clone(),
  )
  .placeholder(ctx.t("summary.team.lifecycle.block.placeholder"))
  .text_area()
  .render(ctx);
  let (open, ask) = (controls.blocking.clone(), controls.clone());
  let confirm = ActionButton::new(
    BLOCK,
    ButtonKind::Primary,
    ctx.t("summary.team.lifecycle.block.confirm"),
  )
  .icon(Icon::Ban);
  let confirm = if typed.trim().is_empty() {
    confirm.disabled()
  } else {
    confirm.on_click(move || {
      open.set(false);
      ask.run(TeamAsk::Block);
    })
  };
  Some(
    Dialog::new(
      "task-block",
      Icon::Ban,
      ctx.t_args("summary.team.lifecycle.block.title", [("task", key.as_str())]),
      ctx.t("summary.team.lifecycle.block.cancel"),
    )
    .part(reason)
    .action(confirm)
    .render(ctx, &controls.blocking),
  )
}

/// *Reopen* with its two choices under it while they are open: Back to Todo (*ready to claim*) and Back to Backlog
/// (*not planned yet*), over the line saying a reopened task is free. Only for an Admin, as the task's capabilities
/// say.
pub fn reopen_menu(ctx: &mut Ctx, anchor: &ElementRef) -> Option<Element> {
  let controls = TeamControls::of(ctx)?;
  let open = controls.reopening.clone();
  let choose = |id: &str, label: &str, detail: &str, glyph: Icon, ask: TeamAsk| {
    let (open, controls) = (open.clone(), controls.clone());
    MenuItem::new(id.to_owned(), ctx.t(label), move || {
      open.set(false);
      controls.run(ask);
    })
    .icon(glyph)
    .detail(ctx.t(detail))
    .inline_detail()
  };
  let rows = vec![
    choose(
      "task-reopen-todo",
      "summary.team.lifecycle.reopen.todo",
      "summary.team.lifecycle.reopen.todoDetail",
      Icon::Circle,
      TeamAsk::BackToTodo,
    ),
    choose(
      "task-reopen-backlog",
      "summary.team.lifecycle.reopen.backlog",
      "summary.team.lifecycle.reopen.backlogDetail",
      Icon::CircleDashed,
      TeamAsk::BackToBacklog,
    ),
  ];
  let rows: Vec<Element> = rows.into_iter().map(|row| row.render(ctx, &open)).collect();
  let foot = Text::new(&ctx.t("summary.team.lifecycle.reopen.free"))
    .id("task-reopen-free")
    .variant(TypographyStyle::Caption)
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm));
  let panel = menu_panel("task-reopen-menu")
    .width(Dimension::Px(REOPEN_WIDTH))
    .with_children(rows)
    .child(separator(ctx))
    .child(foot);
  Some(menu_at_end(anchor.clone(), open, panel).into())
}

/// The reopen menu's width (T4 `V26A3`).
const REOPEN_WIDTH: f32 = 250.0;
