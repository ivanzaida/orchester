//! *Replace draft vN?*, asked in place under a viewed version's banner while a draft exists (`05b` `eCrOM`).
use lurq::{app::ctx::Ctx, node::Element};

use super::state::State;
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  notice::{Notice, NoticeTone},
};

pub fn question(ctx: &Ctx, state: &State, writing: bool) -> Element {
  let editing = state.get();
  let version = orchester_workspace::pipelines::write::next_version(&editing.opened)
    .map_or_else(String::new, |v| v.get().to_string());
  let (replace, keep) = (state.clone(), state.clone());
  let confirm = ActionButton::new(
    "pipeline-restore-confirm",
    ButtonKind::Primary,
    ctx.t_args("pipelines.replaceDraft", [("version", version.clone())]),
  )
  .icon(Icon::Undo2)
  .on_click(move || {
    if !writing {
      replace.update(|s| s.restore());
      replace.get_untracked().views.to_draft();
    }
  });
  Notice::new(
    "pipeline-restore-question",
    NoticeTone::Warning,
    ctx.t_args("pipelines.replaceDraftTitle", [("version", version)]),
  )
  .stacked()
  .flat()
  .body(ctx.t("pipelines.replaceDraftDetail"))
  .action(if writing { confirm.disabled() } else { confirm })
  .action(
    ActionButton::new("pipeline-restore-keep", ButtonKind::Ghost, ctx.t("pipelines.keepDraft"))
      .on_click(move || keep.update(|s| s.restore_requested = false)),
  )
  .into()
}
