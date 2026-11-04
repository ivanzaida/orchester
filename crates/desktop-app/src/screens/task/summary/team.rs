//! What a team task's Summary adds (`01c` `Tb55G`, `docs/design/team-authority.md` §1–2): for a member, one calm line
//! naming who changes the task (`Read-only line` `drVo6`); the claim its work runs under as one strip (`Claim · Claimed
//! by you on this computer` `LR58S`: a 1 `border` outline, radius 6, 10 over and under and 12 at its sides, a 14
//! `user-check` in `success`, 10 to the title in 12.5 `text_primary` and the detail under it in 12 `text_secondary`, 2
//! apart; in `warning` once the server finds it stale); and the properties *Assignee* and *Claim* first, *Module* after
//! the pipeline and *Merges into* the module's branch in place of *Base* (`iqkBC`, `RwzsJ`, `q254Rz`, `P81IsR`). Each
//! is read-only: claiming, taking over and changing them come with the team's actions.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::run::RunStatus;
use orchester_workspace::sharing::device;

use super::properties::{Look, row, value};
use crate::{
  screens::{settings::orchester::moment, task::TaskContext},
  team::{
    lock,
    pages::{TeamClaim, TeamPerson},
  },
  theme::{ExtraSpacing, HubText, IconSize, TaskSpacing},
  ui::icon::{Icon, icon},
};

/// The member's line and the claim's strip, over the Summary's other parts.
pub fn top(ctx: &Ctx, task: &TaskContext) -> Vec<Element> {
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
  let claim = team.task.claim.as_ref().map(|claim| strip(ctx, task, claim));
  lock.into_iter().chain(claim).collect()
}

/// Whether the task's latest run runs on this computer: its running step renews a claim held here.
fn runs_here(task: &TaskContext) -> bool {
  task
    .detail
    .run
    .as_ref()
    .is_some_and(|run| *run.run.status() == RunStatus::Running)
}

/// The minutes a claim goes without renewal before the server finds it stale.
fn stale_minutes(claim: &TeamClaim) -> String {
  let millis = claim.stale_at.unix_millis() - claim.renewed.unix_millis();
  (millis / 60_000).max(0).to_string()
}

fn strip(ctx: &Ctx, task: &TaskContext, claim: &TeamClaim) -> Element {
  let (time, minutes) = (moment(ctx, claim.renewed), stale_minutes(claim));
  let holder = &claim.holder.name;
  let module = task
    .team
    .as_ref()
    .and_then(|team| team.module.as_ref())
    .map(|module| module.module.details().name().as_str().to_owned());
  let detail = || {
    ctx.t_args(
      "summary.team.claim.detail",
      [("time", time.as_str()), ("minutes", &minutes)],
    )
  };
  let (title, detail, tone) = if claim.stale {
    (
      ctx.t_args("summary.team.claim.staleTitle", [("holder", holder.as_str())]),
      ctx.t_args(
        "summary.team.claim.staleDetail",
        [("time", time.as_str()), ("minutes", &minutes)],
      ),
      PaletteColor::Warning,
    )
  } else if claim.holder.you && runs_here(task) {
    let computer = device::computer_name();
    (
      ctx.t("summary.team.claim.hereTitle"),
      ctx.t_args(
        "summary.team.claim.hereDetail",
        [("computer", computer.as_str()), ("time", &time), ("minutes", &minutes)],
      ),
      PaletteColor::Success,
    )
  } else if let (true, Some(module)) = (claim.under_module, module) {
    let title = ctx.t_args(
      "summary.team.claim.moduleTitle",
      [("module", module.as_str()), ("holder", holder.as_str())],
    );
    (title, detail(), PaletteColor::Success)
  } else if claim.holder.you {
    (ctx.t("summary.team.claim.yoursTitle"), detail(), PaletteColor::Success)
  } else {
    let title = ctx.t_args("summary.team.claim.otherTitle", [("holder", holder.as_str())]);
    (title, detail(), PaletteColor::Success)
  };
  Row::new()
    .id("task-team-claim")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .describe("stale", if claim.stale { "true" } else { "false" })
    .child(icon(Icon::UserRoundCheck, IconSize::Md, tone))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&title)
            .id("task-team-claim-title")
            .variant(HubText::TeamLine.role())
            .color(PaletteColor::TextPrimary)
            .width(Dimension::Pct(100.0)),
        )
        .child(
          Text::new(&detail)
            .id("task-team-claim-detail")
            .variant(TypographyStyle::Description)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
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
      claim(ctx, task, team.task.claim.as_ref()),
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
fn claim(ctx: &Ctx, task: &TaskContext, claim: Option<&TeamClaim>) -> Element {
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
  let words = match (claim.holder.you, runs_here(task)) {
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
