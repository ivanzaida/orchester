//! What the list says while a form is open: a group *New* with one row, the chosen one (`Section · New`, `Item · New
//! agent profile` in A7–A12), which stands where the new resource will be once it is created.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Button, Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::super::model::{Kind, ROW_TALL};
use crate::{
  theme::{ExtraColor, ExtraSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::inert,
  },
};

/// A group's title row, as the list's (`Section · Needs you here` `o6TEKq`).
const SECTION_HEIGHT: f32 = 32.0;

/// What the kind's glyph is, as its rows have it.
pub(super) const fn glyph(kind: Kind) -> Icon {
  match kind {
    Kind::Profile => Icon::Bot,
    Kind::McpServer => Icon::Plug,
    Kind::Tool => Icon::Wrench,
    Kind::Instruction => Icon::FileText,
    Kind::Skill => Icon::Sparkles,
  }
}

/// The key of the new resource's name: *New agent profile*, the form's title and its row's.
pub(super) const fn title_key(kind: Kind) -> &'static str {
  match kind {
    Kind::Profile => "team_new.title.profile",
    Kind::McpServer => "team_new.title.mcp",
    Kind::Tool => "team_new.title.tool",
    Kind::Instruction => "team_new.title.instruction",
    Kind::Skill => "team_new.title.skill",
  }
}

/// The group *New* and its row, over the list's groups.
pub(in crate::screens::team_hub) fn new_row(ctx: &Ctx, kind: Kind) -> Element {
  // The new resource is the group's only row.
  let one = 1;
  let row = inert(Button::empty())
    .id("team-hub-row-new")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_TALL))
    .padding(Padding::new().left(ExtraSpacing::Inset.role()).right(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .describe("state", "chosen")
    .describe("kind", kind.name())
    .background(ExtraColor::Hover.role())
    .border_left(Border::inside(BorderSize::Lg, PaletteColor::Accent))
    .child(icon(glyph(kind), IconSize::Lg, PaletteColor::TextSecondary))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Text::new(&ctx.t(title_key(kind)))
            .id("team-hub-row-new-name")
            .variant(HubText::RowName.role())
            .nowrap(),
        )
        .child(
          Text::new(&ctx.t("team_new.notCreated"))
            .id("team-hub-row-new-line")
            .variant(HubText::RowLine.role())
            .nowrap(),
        ),
    );
  Column::new()
    .id("team-hub-new-group")
    .width(Dimension::Pct(100.0))
    .child(
      Row::new()
        .id("team-hub-section-new")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(SECTION_HEIGHT))
        .padding(Padding::horizontal(SpacingSize::Lg))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("team_new.group"))
            .id("team-hub-section-new-title")
            .variant(HubText::GroupTitle.role())
            .nowrap(),
        )
        .child(
          Text::new(&one.to_string())
            .id("team-hub-section-new-count")
            .variant(HubText::Hint.role())
            .nowrap(),
        ),
    )
    .child(row)
    .into()
}
