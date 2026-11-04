//! A team module's properties beside its Summary (`Properties` `unqtz` in `01e · Module · member` `FuHYN`): the local
//! column's geometry ([`super::properties`]), every row text as the server reads it (`docs/design/team-authority.md`
//! §1): State, Assignee (a 13 `user`), Claim (a 13 `user-check` in `success` and *olha · renewed 15:40*, in `warning`
//! once it is stale), Branch (`git-branch`), Delivers to (`git-merge`), Start date, Target date and Created. Changing
//! them is an Admin's and comes with the team's actions; until then no row opens anything.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::module::ModuleState;

use super::{
  properties::{DOT, PROPERTIES_WIDTH, date_value, property},
  words,
};
use crate::{
  screens::{settings::orchester::moment, usage::words::date_time},
  team::pages::{TeamClaim, TeamModule},
  theme::{ExtraRadius, ExtraSpacing, IconSize, ModulesSpacing},
  ui::icon::{Icon, icon},
};

/// The column of `module`'s properties.
pub fn column(ctx: &Ctx, module: &TeamModule) -> Element {
  let details = module.module.details();
  let key = |key: &str| ctx.t(key);
  let branch = glyphed(
    "module-property-branch-value",
    (Icon::GitBranch, PaletteColor::TextSecondary),
    (&module.branch, PaletteColor::TextPrimary),
  );
  let created = text(
    "module-property-created-value",
    &date_time(ctx, module.module.created_at()),
    PaletteColor::TextPrimary,
  );
  let rows = [
    (
      ("module-property-state", key("modules.properties.state")),
      state(ctx, details.state()),
    ),
    (
      ("module-property-assignee", key("modules.team.properties.assignee")),
      assignee(ctx, module),
    ),
    (
      ("module-property-claim", key("modules.team.properties.claim")),
      claim(ctx, module.claim.as_ref()),
    ),
    (
      ("module-property-branch", key("modules.team.properties.branch")),
      branch,
    ),
    (
      ("module-property-delivers", key("modules.team.properties.deliversTo")),
      delivers(ctx, module),
    ),
    (
      ("module-property-start", key("modules.properties.start")),
      date_value(ctx, "module-property-start", details.start()),
    ),
    (
      ("module-property-target", key("modules.properties.target")),
      date_value(ctx, "module-property-target", details.target()),
    ),
    (
      ("module-property-created", key("modules.properties.created")),
      created.into(),
    ),
  ]
  .map(|((id, key), value)| property((id, &key), value, None));
  Column::new()
    .id("module-properties")
    .width(Dimension::Px(PROPERTIES_WIDTH))
    .flex_shrink(0.0)
    .padding(Padding::new().left(ModulesSpacing::Properties.role()))
    .spacing(ExtraSpacing::Tight.role())
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(rows)
    .into()
}

/// The state's dot and word.
fn state(ctx: &Ctx, state: ModuleState) -> Element {
  let dot = Row::new()
    .size(Dimension::Px(DOT), Dimension::Px(DOT))
    .rounded(ExtraRadius::Full.role())
    .background(match state {
      ModuleState::Open => PaletteColor::Success,
      ModuleState::Closed => PaletteColor::TextMuted,
    });
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(dot)
    .child(text(
      "module-property-state-value",
      &words::state(ctx, state),
      PaletteColor::TextPrimary,
    ))
    .into()
}

/// Who the module is assigned to, or *Not assigned*.
fn assignee(ctx: &Ctx, module: &TeamModule) -> Element {
  let id = "module-property-assignee-value";
  match &module.assignee {
    Some(person) => glyphed(
      id,
      (Icon::User, PaletteColor::TextSecondary),
      (&words::person(ctx, person), PaletteColor::TextPrimary),
    ),
    None => muted(id, &ctx.t("modules.team.properties.notAssigned")),
  }
}

/// The branch the module's delivery takes its work to: its base branch, or else the branch checked out when its
/// delivery runs.
fn delivers(ctx: &Ctx, module: &TeamModule) -> Element {
  let checked_out = ctx.t("modules.form.base.checkedOut");
  let base = module
    .module
    .base()
    .branch()
    .map_or(&*checked_out, |branch| branch.as_str());
  glyphed(
    "module-property-delivers-value",
    (Icon::GitMerge, PaletteColor::TextSecondary),
    (base, PaletteColor::TextPrimary),
  )
}

/// Who holds the module's claim and when their work last renewed it, or that it is stale; *Not claimed* without one.
fn claim(ctx: &Ctx, claim: Option<&TeamClaim>) -> Element {
  let id = "module-property-claim-value";
  let Some(claim) = claim else {
    return glyphed(
      id,
      (Icon::CircleDashed, PaletteColor::TextMuted),
      (&ctx.t("modules.team.notClaimed"), PaletteColor::TextMuted),
    );
  };
  let holder = words::person(ctx, &claim.holder);
  if claim.stale {
    let words = ctx.t_args("modules.team.properties.stale", [("holder", holder)]);
    return glyphed(
      id,
      (Icon::UserRoundCheck, PaletteColor::Warning),
      (&words, PaletteColor::Warning),
    );
  }
  let words = ctx.t_args(
    "modules.team.properties.renewed",
    [("holder", holder.to_string()), ("time", moment(ctx, claim.renewed))],
  );
  glyphed(
    id,
    (Icon::UserRoundCheck, PaletteColor::Success),
    (&words, PaletteColor::TextPrimary),
  )
}

/// A 13 glyph and its words, 6 apart.
fn glyphed(id: &str, (glyph, tone): (Icon, PaletteColor), (words, color): (&str, PaletteColor)) -> Element {
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .min_width(Dimension::Px(0.0))
    .child(icon(glyph, IconSize::Base, tone))
    .child(text(id, words, color))
    .into()
}

fn muted(id: &str, words: &str) -> Element {
  text(id, words, PaletteColor::TextMuted).into()
}

fn text(id: &str, words: &str, color: PaletteColor) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(TypographyStyle::Description)
    .color(color)
    .nowrap()
}
