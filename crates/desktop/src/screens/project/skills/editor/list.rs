//! The skills of the chosen scope tab (`Y09b1` in `ecJtg`): New skill, the list's label, one item
//! per skill (`jNTVN`) with its version and scope, and who owns them. Items are pointer-only, as
//! the list is outside any form.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, ScrollVertical, Text, TextOverflow},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::resource::Skill;

use super::{super::copy, SkillEditor};
use crate::{
  agents::data::VersionState,
  navigation::route::Page,
  skills::data::SkillEntry,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, icon::Icon, pressable::surface},
};

/// List width, New skill height and item height (`Y09b1`, `SypxD`, `jNTVN`).
const LIST_WIDTH: f32 = 236.0;
const NEW_HEIGHT: f32 = 34.0;
const ITEM_HEIGHT: f32 = 66.0;

impl SkillEditor {
  /// The list, with the skill in the editor as edited so far (`live`, `state`).
  pub(super) fn list(&self, ctx: &Ctx, live: &Skill, state: VersionState) -> Element {
    let creating = self.context.creating.clone();
    let new = ActionButton::new(ctx.t("skills.new"))
      .icon(Icon::Plus)
      .compact()
      .id("skill-list-new")
      .fill_width()
      .height(Dimension::Px(NEW_HEIGHT))
      .on_click(move || creating.set(true));
    let filter = self.tab.get();
    let edited = &self.context.entry;
    let items: Vec<Element> = self
      .context
      .data
      .entries
      .iter()
      .map(|entry| {
        if entry.id == edited.id {
          (entry, Some((live, state)))
        } else {
          (entry, None)
        }
      })
      .filter(|(entry, shown)| filter.admits(shown.map_or(entry.body.scope(), |(live, _)| live.scope())))
      .map(|(entry, shown)| self.item(ctx, entry, shown))
      .collect();
    let project = self.context.project.project.name().as_str();
    Column::new()
      .id("skill-list")
      .width(Dimension::Px(LIST_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfacePanel)
      .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(new.render(ctx))
      .child(Text::new(&ctx.t("skills.list.label")).variant(ExtraTypography::Overline.role()))
      .child(
        ScrollVertical::new(
          Column::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Sm)
            .with_children(items),
        )
        .width(Dimension::Pct(100.0))
        .flex(1.0),
      )
      .child(
        Text::new(&ctx.t_args("skills.list.storage", [("project", project)])).variant(ExtraTypography::Detail.role()),
      )
      .into()
  }

  /// A skill (`jNTVN`): its name in mono over its version and scope; the edited one shows as
  /// edited.
  fn item(&self, ctx: &Ctx, entry: &SkillEntry, edited: Option<(&Skill, VersionState)>) -> Element {
    let (body, state) = edited.unwrap_or((&entry.body, entry.state));
    let metadata = ctx.t_args(
      "skills.list.metadata",
      [
        ("version", &*copy::short_version(ctx, state)),
        ("scope", &*copy::scope_short(ctx, body.scope(), &self.context.data)),
      ],
    );
    let text = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(&entry.name)
          .variant(ExtraTypography::MonoValue.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .child(
        Text::new(&metadata)
          .variant(ExtraTypography::Footnote.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      );
    let selected = edited.is_some();
    let frame: Button = surface()
      .id(Arc::<str>::from(format!("skill-{}", entry.slug)))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ITEM_HEIGHT))
      .padding(ExtraSpacing::Stack.role())
      .rounded(RadiusSize::Md)
      .background(if selected {
        PaletteColor::AccentMuted
      } else {
        PaletteColor::SurfacePanel
      })
      .child(text);
    if selected {
      return frame.describe("state", "selected").into();
    }
    self
      .context
      .nav
      .link(Page::Skills(Some(entry.id)))
      .on(frame)
      .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
      .into()
  }
}
