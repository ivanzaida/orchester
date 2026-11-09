//! The Instructions tab with no instruction yet (06b `VcE5a`): one quiet row in its box with New instruction.
use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};

use super::state::{Selection, Tab, View};
use crate::{
  project::ShownProject,
  screens::hub::{
    self,
    parts::{Entry, boxed},
  },
  theme::HubSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

pub fn empty(ctx: &Ctx, project: &ShownProject, selection: &Selection) -> Element {
  let pane = Column::new()
    .id("instructions-empty-page")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Groups.role())
    .child(boxed("instructions-empty-rows", [empty_row(ctx, project, selection)]));
  hub::list_body("instructions-empty-list", pane)
}

/// The quiet row: what instructions are, with New instruction (06b).
fn empty_row(ctx: &Ctx, project: &ShownProject, selection: &Selection) -> Element {
  let name = project.open.project.name().as_str();
  Entry::new(
    "instructions-empty",
    ctx.t_args("instructions.emptyTitle", [("project", name)]),
  )
  .tile(Icon::BookOpen)
  .hint(ctx.t("instructions.emptyDetail"))
  .end(
    ActionButton::new(
      "instructions-empty-new",
      ButtonKind::Secondary,
      ctx.t("instructions.new"),
    )
    .icon(Icon::Plus)
    .opens(selection.to(View::new_rule(Tab::Project))),
  )
  .into()
}
