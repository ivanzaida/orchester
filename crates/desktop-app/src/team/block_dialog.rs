//! Mark as blocked's question, one dialog from every place that asks it: the task page and the Tasks list's row menu
//! (`Task - v2` `jDN0r` T13 `DWfta`, T14 `uKVK2`, rules T15 `ahGVl`). It names the task and asks for the reason, which
//! is required: while the field is empty or holds only spaces the primary action is disabled and the footer says why;
//! once text is typed the footer says *Esc cancels*. *Cancel*, Esc and a press on the dimmed page cancel and change
//! nothing.

use lurq::{
  app::ctx::Ctx,
  components::Text,
  core::Signal,
  node::{Element, dimension::Dimension},
};

use crate::{
  theme::DialogText,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, FooterNote},
    field::{FieldInput, field},
    icon::Icon,
  },
};

/// The dialog is 520 wide (`Dialog` `y47tS8` in T13 `DWfta`) and its reason field 88 high (`Field · multi-line`
/// `vRqiM`).
const WIDTH: f32 = 520.0;
const REASON_HEIGHT: f32 = 88.0;

/// What the dialog is made from: the element ids it starts with, the task's key, the reason typed so far and whether it
/// is open.
pub struct Blocking<'a> {
  pub id: &'a str,
  pub key: &'a str,
  pub reason: &'a Signal<String>,
  pub open: &'a Signal<bool>,
}

/// The dialog while `blocking.open` holds; `confirm` runs with the reason, trimmed, when the person marks the task as
/// blocked, and the dialog closes.
pub fn dialog(ctx: &mut Ctx, blocking: &Blocking<'_>, confirm: impl Fn(String) + Send + Sync + 'static) -> Element {
  let id = blocking.id;
  let typed = blocking.reason.get();
  let words = typed.trim().to_owned();
  let input = FieldInput::new(
    format!("{id}-reason"),
    ctx.t("summary.team.lifecycle.block.reason"),
    blocking.reason.clone(),
  )
  .placeholder(ctx.t("summary.team.lifecycle.block.placeholder"))
  .text_area()
  .area_height(REASON_HEIGHT)
  .render(ctx);
  let reason = field(
    id,
    Some(&ctx.t("summary.team.lifecycle.block.reason")),
    input,
    Some(&ctx.t("summary.team.lifecycle.block.hint")),
  );
  let explain = Text::new(&ctx.t("summary.team.lifecycle.block.explain"))
    .id(format!("{id}-explain"))
    .variant(DialogText::Line.role())
    .width(Dimension::Pct(100.0));
  let button = ActionButton::new(
    format!("{id}-confirm"),
    ButtonKind::Primary,
    ctx.t("summary.team.lifecycle.block.confirm"),
  )
  .icon(Icon::Ban);
  let (button, note) = if words.is_empty() {
    (
      button.disabled(),
      FooterNote::Unavailable(ctx.t("summary.team.lifecycle.block.unavailable")),
    )
  } else {
    let open = blocking.open.clone();
    (
      button.on_click(move || {
        open.set(false);
        confirm(words.clone());
      }),
      FooterNote::Keys(ctx.t("summary.team.lifecycle.block.escCancels")),
    )
  };
  Dialog::new(
    id,
    Icon::Ban,
    ctx.t_args("summary.team.lifecycle.block.title", [("task", blocking.key)]),
    ctx.t("summary.team.lifecycle.block.cancel"),
  )
  .width(WIDTH)
  .part(explain)
  .part(reason)
  .footer_note(note)
  .action(button)
  .render(ctx, blocking.open)
}
