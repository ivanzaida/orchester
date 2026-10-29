//! Profiles while they are read: the list (`03 Agents · Profiles · loading` `KHDar`), the Profiles heading and the
//! versions note real, its summary and each row's mark, name, connection line, use and version state bones, lower rows
//! fading; a profile's page (`04 Agents · Profile · loading` `EV7of`), `← Profiles`, the group headings, row labels
//! and their fixed help lines real, the mark, name, summary and every value bones. *Reading agent profiles…*
//! (`agents.reading`) names both.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{AgentsNav, page_head};
use crate::{
  screens::hub::{
    self, bones,
    parts::{boxed, field, group, head, note, stacked_field},
  },
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    skeleton::{self, Skeleton, bone, fading},
  },
};

/// Each listed profile's bones: its name, its connection line, its use and its version state.
const ROWS: [(f32, f32, f32, f32); 4] = [
  (38.0, 376.0, 46.0, 66.0),
  (58.0, 360.0, 60.0, 78.0),
  (42.0, 408.0, 46.0, 66.0),
  (40.0, 322.0, 52.0, 96.0),
];
/// The profile page's head: its mark, its name and what it says under it (`Page head` in `EV7of`).
const MARK: f32 = 40.0;
const NAME: f32 = 98.0;
const SUMMARY: (f32, f32) = (258.0, 116.0);
/// A field's frame on the profile's page, and its value's bones.
const FIELD: f32 = 284.0;

/// The list while the profiles are read.
pub(super) fn list(ctx: &mut Ctx) -> Element {
  let rows = ROWS
    .iter()
    .enumerate()
    .map(|(index, (name, line, uses, state))| {
      let end = Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(skeleton::line(ctx, HubText::Hint.role(), *uses))
        .child(bones::status(ctx, *state));
      bones::entry(ctx, Some(bones::tile().into()), (*name, *line), vec![end.into()]).id(format!("agents-reading-{index}"))
    })
    .collect();
  let rows: Vec<Element> = fading(rows).into_iter().map(Element::from).collect();
  let pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Groups.role())
    .child(group(
      "agents-reading",
      bones::head(ctx, "agents-reading", &ctx.t("agents.listTitle"), Some(252.0)),
      boxed("agents-reading-rows", rows),
    ))
    .child(note("agents-version-note", Icon::History, &ctx.t("agents.versionNote")));
  let body = Skeleton::new("agents-loading", ctx.t("agents.reading")).show(ctx, pane);
  hub::list_body("agents-reading-list", body)
}

/// A profile's page while the profiles are read.
pub(super) fn profile(ctx: &mut Ctx, nav: &AgentsNav) -> Element {
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(bone(MARK, MARK).rounded(RadiusSize::Lg))
    .child(
      Column::new()
        .spacing(ExtraSpacing::Snug.role())
        .child(skeleton::line(ctx, TypographyStyle::Title, NAME))
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(skeleton::line(ctx, HubText::Hint.role(), SUMMARY.0))
            .child(bone(4.0, 4.0))
            .child(skeleton::line(ctx, HubText::Hint.role(), SUMMARY.1)),
        ),
    );
  let pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Groups.role())
    .child(page_head::crumb(ctx, nav))
    .child(top)
    .with_children(fading(vec![role(ctx), connection(ctx)]))
    .child(note("agents-reading-assignment", Icon::Info, &ctx.t("agents.assignmentNote")));
  let body = Skeleton::new("agents-loading", ctx.t("agents.reading")).show(ctx, pane);
  hub::list_body("agents-reading-page", body)
}

/// The role group: the name and the responsibility in their fields, the role instructions in their box.
fn role(ctx: &Ctx) -> Column {
  let t = |key: &str| ctx.t(key);
  let instructions = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Md)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .background(PaletteColor::SurfaceInput)
    .child(skeleton::line(ctx, TypographyStyle::Description, 98.0))
    .child(skeleton::line(ctx, TypographyStyle::Description, 456.0))
    .child(skeleton::line(ctx, TypographyStyle::Description, 284.0));
  let rows = vec![
    field("agents-reading-name", &t("agents.role"), Some(&t("agents.roleHint")), skeleton::field(ctx, FIELD, 42.0)).into(),
    field(
      "agents-reading-responsibility",
      &t("agents.responsibility"),
      Some(&t("agents.responsibilityHint")),
      skeleton::field(ctx, FIELD, 166.0),
    )
    .into(),
    stacked_field(
      "agents-reading-instructions",
      (&t("agents.instructions"), Some(&t("agents.instructionsHint"))),
      instructions,
      Some(&t("agents.markdown")),
    )
    .into(),
  ];
  group(
    "agents-reading-role",
    head("agents-reading-role", &t("agents.roleSection"), None, None),
    boxed("agents-reading-role-rows", rows),
  )
}

/// The connection and model group: the connection with its line, the model and the effort in their selects.
fn connection(ctx: &Ctx) -> Column {
  let t = |key: &str| ctx.t(key);
  let select = |value: f32| {
    skeleton::field(ctx, FIELD, value).child(lurq::components::Spacer::new().flex(1.0)).child(icon(
      Icon::ChevronDown,
      IconSize::Md,
      PaletteColor::TextMuted,
    ))
  };
  let connection = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(HubSpacing::FieldGap.role())
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&t("agents.connection")).variant(crate::theme::ExtraTypography::Strong.role()))
        .child(skeleton::line(ctx, HubText::Hint.role(), 216.0)),
    )
    .child(skeleton::field(ctx, FIELD, 162.0));
  let rows = vec![
    connection.into(),
    field("agents-reading-model", &t("agents.model"), Some(&t("agents.modelHint")), select(98.0)).into(),
    field("agents-reading-effort", &t("agents.effort"), Some(&t("agents.effortHint")), select(22.0)).into(),
  ];
  group(
    "agents-reading-connection",
    head("agents-reading-connection", &t("agents.connectionModel"), None, None),
    boxed("agents-reading-connection-rows", rows),
  )
}
