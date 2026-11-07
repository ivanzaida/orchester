//! The panel beside the list (`Detail` `EXAbv` in `t4mtQL`): the chosen resource's head (what it is and its version,
//! its name), its chips (`Chips` `Zi28O`: *Team · vN* and how it stands here) and what its readiness lacks, with the
//! one action that moves it on pinned at its bottom (`Actions` `x3uhYJ`). The panel of each kind is its own task
//! (ORC-832…834): a profile's is [`super::profile`]; until the others have theirs the panel is this frame, and its
//! pinned action is only Edit, which opens the editor the kind has today.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::readiness::{Group, ReadinessStanding};

use super::{
  model::{Entry, Kind, Listing},
  words,
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{AgentTab, ProjectPage, Route, ToolTab},
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
  },
};

/// The pinned actions' height (`Actions` `x3uhYJ`).
const ACTIONS_HEIGHT: f32 = 56.0;

/// What the panel needs besides the resource.
pub(super) struct Shown<'a> {
  pub project: ProjectId,
  /// The person may change the team's resources: an admin of the team.
  pub may_change: bool,
  pub navigation: &'a Navigation,
}

pub(super) fn panel(ctx: &Ctx, listing: &Listing, entry: &Entry, shown: &Shown<'_>) -> Element {
  let scroll = ScrollVertical::new(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        HubSpacing::Sections.role(),
        HubSpacing::BodyTop.role(),
      ))
      .spacing(HubSpacing::BodyTop.role())
      .child(head(ctx, entry, None))
      .child(chips(ctx, listing, entry))
      .with_children(readiness(ctx, listing, entry)),
  )
  .id("team-hub-panel-scroll")
  .width(Dimension::Pct(100.0))
  .flex(1.0)
  .min_height(Dimension::Px(0.0));
  Column::new()
    .id("team-hub-panel")
    .describe("resource", entry.id.to_string())
    .describe("kind", entry.kind.name())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(scroll)
    .with_children(actions(ctx, entry, shown))
    .into()
}

/// The head: what the resource is and its version over its name, and at its end what `end` puts there (*Edit*).
pub(super) fn head(ctx: &Ctx, entry: &Entry, end: Option<Element>) -> Row {
  let mut where_ = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t(entry.kind.where_key()))
        .id("team-hub-panel-kind")
        .variant(HubText::Status.role())
        .nowrap(),
    );
  if let Some(number) = entry.number {
    where_ = where_.child(
      Text::new(&number.to_string())
        .id("team-hub-panel-version")
        .variant(HubText::Hint.role())
        .nowrap(),
    );
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(where_)
        .child(
          Text::new(&entry.name)
            .id("team-hub-panel-title")
            .variant(TypographyStyle::Title),
        ),
    )
    .with_children(end)
}

/// *Edit* in a head (`Edit` in `Agents · All · admin` `t4mtQL`).
pub(super) fn edit_button(ctx: &Ctx, open: Open) -> Element {
  ActionButton::new("team-hub-edit", ButtonKind::Ghost, ctx.t("team_hub.panel.edit"))
    .icon(Icon::Pencil)
    .opens(open)
    .into()
}

pub(super) fn chips(ctx: &Ctx, listing: &Listing, entry: &Entry) -> Row {
  let (words, tone) = words::chip(ctx, listing, entry);
  let team = match entry.number {
    Some(number) => ctx.t_args("team_hub.chip.team", [("version", number.to_string())]),
    None => ctx.t("team_hub.chip.teamUnnumbered"),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(badge("team-hub-chip-team", BadgeTone::Neutral, &team))
    .child(badge("team-hub-chip-here", tone, &words))
}

/// *Ready here?*: the parts this computer lacks, one row each; none for a resource that lacks nothing.
fn readiness(ctx: &Ctx, listing: &Listing, entry: &Entry) -> Option<Column> {
  let Some(ReadinessStanding::Missing(missing)) = &entry.standing else {
    return None;
  };
  if entry.group == Group::OffHere {
    return None;
  }
  let rows = missing.missing().iter().enumerate().map(|(index, part)| {
    Row::new()
      .id(format!("team-hub-missing-{index}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(Icon::TriangleAlert, IconSize::Base, PaletteColor::Warning))
      .child(
        Text::new(&words::missing(ctx, listing, entry, part))
          .variant(HubText::Value.role())
          .flex(1.0),
      )
  });
  Some(
    Column::new()
      .id("team-hub-readiness")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(
            Text::new(&ctx.t("team_hub.readiness.title"))
              .variant(HubText::Kind.role())
              .color(PaletteColor::TextMuted),
          )
          .child(
            Text::new(&ctx.t_args(
              "team_hub.readiness.missing",
              [("count", missing.missing().len().to_string())],
            ))
            .variant(HubText::Hint.role())
            .color(PaletteColor::Warning),
          ),
      )
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .rounded(RadiusSize::Md)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .with_children(rows),
      ),
  )
}

/// The editor the kind has today, which a change of the team's resource opens; an MCP server has none yet.
pub(super) fn editor(entry: &Entry, project: ProjectId) -> Option<Route> {
  let page = match entry.kind {
    Kind::Profile => ProjectPage::Agent(entry.id, AgentTab::default()),
    Kind::Skill => ProjectPage::Skill(entry.id),
    Kind::Tool => ProjectPage::Tool(entry.id, ToolTab::default()),
    Kind::McpServer | Kind::Instruction => return None,
  };
  Some(Route::Project(project, page))
}

/// The pinned actions: Edit for an admin, where the kind has an editor; no bar when there is nothing to do, and
/// nothing disabled in its place (`Member view`, notes rule 15).
fn actions(ctx: &Ctx, entry: &Entry, shown: &Shown<'_>) -> Option<Element> {
  let route = editor(entry, shown.project).filter(|_| shown.may_change)?;
  let open: Open = shown.navigation.to(route);
  Some(
    Row::new()
      .id("team-hub-actions")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ACTIONS_HEIGHT))
      .padding(Padding::horizontal(HubSpacing::Sections.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfacePanel)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        ActionButton::new("team-hub-edit", ButtonKind::Primary, ctx.t("team_hub.panel.edit"))
          .icon(Icon::Pencil)
          .opens(open),
      )
      .into(),
  )
}
