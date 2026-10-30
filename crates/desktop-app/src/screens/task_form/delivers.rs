//! The Delivers chip (`Delivers` `giv2K`) and its menu (`Delivers menu` `o7M1ed` in `05 · New task · what it delivers`
//! `Tu7Jm`): what the task's runs deliver (ORC-93), a part of its organization. *As the pipeline says* leaves it to the
//! chosen pipeline, and its help says what that pipeline says; the four others say otherwise for this task's runs:
//! code changes, a report, work outside the repository, or changes or a report. The chip is empty while the pipeline
//! decides and reads the task's own choice once there is one. A shared project's server does not keep it and the task
//! writer refuses it there: a shared project's chip shows what the pipeline delivers with a lock, and its popover says
//! why it cannot be changed here, so a save never sends it.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::resource::Deliverable;
use orchester_store::StoreError;
use orchester_workspace::messages;

use super::{
  chips::{ChipState, HelpPlace, Look, MenuOption, PropertyChip, menu_head},
  state::View,
};
use crate::ui::{
  filter::filter_menu_panel, icon::Icon, menu::separator, message::message_text, popover::popover_panel,
  unavailable::reason_line,
};

/// The menu's width (`Delivers menu` `o7M1ed`), and the width of a shared project's popover, which says why.
const MENU_WIDTH: f32 = 400.0;
const UNAVAILABLE_WIDTH: f32 = 320.0;

/// What the task's runs deliver; `None`: as its pipeline says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeliversPick(pub Option<Deliverable>);

impl lurq::app::component::DevtoolsInspectable for DeliversPick {}

/// The glyph of `delivers` and the i18n keys of its words: the chip's value, the option's title and help, and what a
/// pipeline that delivers it says.
const fn words(delivers: Deliverable) -> (Icon, &'static str, &'static str, &'static str, &'static str) {
  match delivers {
    Deliverable::Changes => (
      Icon::GitPullRequest,
      "task_form.delivers.chip.changes",
      "task_form.delivers.changes",
      "task_form.delivers.changesHelp",
      "task_form.delivers.says.changes",
    ),
    Deliverable::Report => (
      Icon::FileText,
      "task_form.delivers.chip.report",
      "task_form.delivers.report",
      "task_form.delivers.reportHelp",
      "task_form.delivers.says.report",
    ),
    Deliverable::Outside => (
      Icon::Plug,
      "task_form.delivers.chip.outside",
      "task_form.delivers.outside",
      "task_form.delivers.outsideHelp",
      "task_form.delivers.says.outside",
    ),
    Deliverable::ChangesOrReport => (
      Icon::GitCompare,
      "task_form.delivers.chip.changesOrReport",
      "task_form.delivers.changesOrReport",
      "task_form.delivers.changesOrReportHelp",
      "task_form.delivers.says.changesOrReport",
    ),
  }
}

pub fn delivers_chip(
  ctx: &mut Ctx,
  view: &View,
  (shared, state): (bool, &ChipState),
  pick: &Signal<DeliversPick>,
) -> Element {
  if shared {
    return shared_chip(ctx, view, state);
  }
  let (look, glyph, value) = match view.values.delivers {
    Some(delivers) => {
      let (glyph, chip, ..) = words(delivers);
      (Look::Set, glyph, ctx.t(chip))
    }
    None => (Look::Empty, Icon::PackageCheck, ctx.t("task_form.delivers.asPipeline")),
  };
  let panel = state.is_open().then(|| menu(ctx, view, state, pick));
  PropertyChip::new(
    "task-form-delivers",
    glyph,
    (ctx.t("task_form.delivers.label"), value),
    look,
  )
  .locked(view.locked())
  .render(state, panel)
}

/// A shared project's chip: what the pipeline delivers, with a lock; its popover says why it cannot be changed here.
fn shared_chip(ctx: &mut Ctx, view: &View, state: &ChipState) -> Element {
  let value = view.pipeline().map_or_else(
    || ctx.t("task_form.delivers.asPipeline"),
    |pipeline| {
      let (_, chip, ..) = words(pipeline.delivers);
      ctx.t(chip)
    },
  );
  let reason = message_text(ctx, &messages::store_error(&StoreError::SharedProject));
  let panel = state.is_open().then(|| {
    popover_panel("task-form-delivers-unavailable", UNAVAILABLE_WIDTH)
      .child(reason_line("task-form-delivers-reason", &reason))
      .into()
  });
  PropertyChip::new(
    "task-form-delivers",
    Icon::PackageCheck,
    (ctx.t("task_form.delivers.label"), value),
    Look::Empty,
  )
  .unavailable(reason)
  .render(state, panel)
}

fn menu(ctx: &mut Ctx, view: &View, state: &ChipState, pick: &Signal<DeliversPick>) -> Element {
  let chosen = view.values.delivers;
  let option = |ctx: &Ctx, delivers: Option<Deliverable>| {
    let (icon, title, help) = match delivers {
      Some(delivers) => {
        let (icon, _, title, help, _) = words(delivers);
        (icon, ctx.t(title), ctx.t(help))
      }
      None => (
        Icon::Workflow,
        ctx.t("task_form.delivers.asPipeline"),
        pipeline_says(ctx, view),
      ),
    };
    let id = delivers.map_or("pipeline", Deliverable::as_str);
    let pick = pick.clone();
    MenuOption {
      id: format!("task-form-delivers-{id}"),
      icon,
      title,
      help: Some(help),
      help_place: HelpPlace::BesideIcon,
      chosen: delivers == chosen,
      mono: false,
    }
    .render(&state.open, move || pick.set(DeliversPick(delivers)))
  };
  let choices: Vec<Element> = Deliverable::ALL
    .into_iter()
    .map(|delivers| option(ctx, Some(delivers)))
    .collect();
  filter_menu_panel("task-form-delivers-menu", MENU_WIDTH)
    .child(menu_head(&ctx.t("task_form.delivers.head")))
    .child(option(ctx, None))
    .child(separator(ctx))
    .with_children(choices)
    .into()
}

/// What the chosen pipeline says its runs deliver, or that a pipeline decides once one is chosen.
fn pipeline_says(ctx: &Ctx, view: &View) -> Arc<str> {
  let Some(pipeline) = view.pipeline() else {
    return ctx.t("task_form.delivers.asPipelineHelp");
  };
  let (.., says) = words(pipeline.delivers);
  ctx.t_args(
    "task_form.delivers.pipelineSays",
    [
      ("pipeline", pipeline.name.to_string()),
      ("what", ctx.t(says).to_string()),
    ],
  )
}

/// What the task's runs deliver: its own choice, else the chosen pipeline's; `None` without either.
pub fn delivered(view: &View) -> Option<Deliverable> {
  view
    .values
    .delivers
    .or_else(|| view.pipeline().map(|pipeline| pipeline.delivers))
}
