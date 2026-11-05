//! What a team task's Summary adds (`01c` `Tb55G`, `docs/design/team-authority.md` §1–3): for a member, one calm line
//! naming who changes the task (`Read-only line` `drVo6`); what the last team action did not do (`DpoZG` in `02f`,
//! [`team_controls::refusal`]); the claim its work runs under as one strip ([`super::claim`]); and the properties
//! *Assignee* and *Claim* first, *Module* after the pipeline and *Merges into* the module's branch in place of *Base*
//! (`iqkBC`, `RwzsJ`, `q254Rz`, `P81IsR`), each read-only.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::properties::{Look, row, value};
use crate::{
  screens::task::{TaskContext, team_controls},
  team::{
    lock,
    pages::{TeamClaim, TeamPerson},
  },
  theme::{ExtraSpacing, IconSize, TaskSpacing},
  ui::icon::{Icon, icon},
};

/// The member's line, the last action's refusal and the claim's strip, over the Summary's other parts.
pub fn top(ctx: &mut Ctx, task: &TaskContext) -> Vec<Element> {
  let Some(team) = &task.team else {
    return Vec::new();
  };
  let lock = team.standing.member().then(|| {
    let words = lock::lock_words(
      ctx,
      team.standing.admins.as_deref(),
      ("summary.team.readOnly", "summary.team.readOnlyUnnamed"),
    );
    lock::locked("task-team-read-only", &words).into()
  });
  let refusal = team_controls::refusal(ctx, task);
  let claim = super::claim::line(ctx, task);
  lock.into_iter().chain(refusal).chain(claim).collect()
}

/// A property's row, as the Summary's other properties are.
fn property(id: &str, key: &str, value: Element) -> Element {
  row(id, key, value, TaskSpacing::PropertyRow.role())
}

/// *Assignee* and *Claim*, a team task's first properties; none for a local task's.
pub fn first_rows(ctx: &Ctx, task: &TaskContext) -> Vec<Element> {
  let Some(team) = &task.team else {
    return Vec::new();
  };
  vec![
    property(
      "task-property-assignee",
      &ctx.t("summary.team.properties.assignee"),
      assignee(ctx, team.task.assignee.as_ref()),
    ),
    property(
      "task-property-claim",
      &ctx.t("summary.team.properties.claim"),
      claim(ctx, team.task.claim.as_ref()),
    ),
  ]
}

/// *Module*: the name of a team task's module, as the server reads it, with the modules' `target` (the design draws
/// `boxes`, which the bundled `lucide.ttf` maps nowhere the app names yet).
pub fn module_row(ctx: &Ctx, task: &TaskContext) -> Option<Element> {
  let module = task.team.as_ref()?.module.as_ref()?;
  let name = glyphed(
    "task-property-module",
    (Icon::Target, PaletteColor::TextSecondary),
    module.module.details().name().as_str(),
  );
  Some(property(
    "task-property-module",
    &ctx.t("summary.team.properties.module"),
    name,
  ))
}

/// *Merges into*: the branch of a team task's module, which its runs' work merges into, with a `git-merge`.
pub fn merges_row(ctx: &Ctx, task: &TaskContext) -> Option<Element> {
  let module = task.team.as_ref()?.module.as_ref()?;
  let branch = glyphed(
    "task-property-merges",
    (Icon::GitMerge, PaletteColor::TextSecondary),
    &module.branch,
  );
  Some(property(
    "task-property-merges",
    &ctx.t("summary.team.properties.mergesInto"),
    branch,
  ))
}

/// *ivan (you)* with a `user`, or *Not assigned*.
fn assignee(ctx: &Ctx, assignee: Option<&TeamPerson>) -> Element {
  let id = "task-property-assignee";
  match assignee {
    Some(person) => glyphed(
      id,
      (Icon::User, PaletteColor::TextSecondary),
      &person_words(ctx, person),
    ),
    None => value(id, &ctx.t("summary.team.properties.notAssigned"), Look::Muted).into(),
  }
}

/// *You · this computer*, *You*, the holder, or *olha · stale* in `warning`; *Not claimed* without a claim.
fn claim(ctx: &Ctx, claim: Option<&TeamClaim>) -> Element {
  let id = "task-property-claim";
  let Some(claim) = claim else {
    return value(id, &ctx.t("summary.team.properties.notClaimed"), Look::Muted).into();
  };
  if claim.stale {
    let words = ctx.t_args(
      "summary.team.properties.stale",
      [("holder", claim.holder.name.as_str())],
    );
    return glyphed(id, (Icon::UserRoundCheck, PaletteColor::Warning), &words);
  }
  let words = match (claim.holder.you, claim.here) {
    (true, true) => ctx.t("summary.team.properties.here"),
    (true, false) => ctx.t("summary.team.properties.yours"),
    (false, _) => claim.holder.name.as_str().into(),
  };
  glyphed(id, (Icon::UserRoundCheck, PaletteColor::Success), &words)
}

fn person_words(ctx: &Ctx, person: &TeamPerson) -> std::sync::Arc<str> {
  if person.you {
    ctx.t_args("summary.team.properties.you", [("name", person.name.as_str())])
  } else {
    person.name.as_str().into()
  }
}

/// A 13 glyph and its words in 12 `text_primary`, 6 apart.
fn glyphed(id: &str, (glyph, tone): (Icon, PaletteColor), words: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, tone))
    .child(value(&format!("{id}-label"), words, Look::Primary))
    .into()
}
