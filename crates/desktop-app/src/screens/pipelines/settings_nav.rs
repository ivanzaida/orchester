//! Section links in Pipeline settings: scroll to the actual section bounds.
use std::collections::BTreeMap;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::ScrollState},
  node::{border::Border, dimension::Dimension, padding::Padding},
};

use super::state::{Panel, State};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::pressable,
  },
};
const KEYS: [&str; 9] = [
  "pipelines.general",
  "pipelines.starts",
  "pipelines.runDefaults",
  "pipelines.failureDefaults",
  "pipelines.permissions",
  "pipelines.sources",
  "pipelines.delivery",
  "pipelines.who",
  "pipelines.archive",
];
pub struct Sections {
  pub scroll: ScrollState,
  pub origin: ElementRef,
  targets: BTreeMap<&'static str, ElementRef>,
  active: Signal<&'static str>,
}
impl Sections {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      scroll: ScrollState::new(),
      origin: ElementRef::new(),
      targets: KEYS.into_iter().map(|key| (key, ElementRef::new())).collect(),
      active: ctx.signal(KEYS[0]),
    }
  }
  pub fn target(&self, key: &'static str) -> ElementRef {
    self.targets[key].clone()
  }
  pub fn navigation(&self, ctx: &Ctx, state: &State, changed: bool) -> Column {
    let back = state.clone();
    Column::new()
      .width(Dimension::Px(212.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inset.role()))
      .spacing(ExtraSpacing::Tight.role())
      .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        ActionButton::new("pipeline-back-steps", ButtonKind::Secondary, ctx.t("pipelines.back"))
          .icon(Icon::ChevronLeft)
          .on_click(move || back.update(|s| s.panel = Panel::Pipeline)),
      )
      .child(Spacer::new().height(Dimension::Px(14.0)))
      .child(Text::new(&ctx.t("pipelines.settingsNav")).variant(ExtraTypography::ColumnHeading.role()))
      .child(Spacer::new().height(Dimension::Px(6.0)))
      .with_children(KEYS.map(|key| {
        let (target, origin, scroll, active) = (
          self.target(key),
          self.origin.clone(),
          self.scroll.clone(),
          self.active.clone(),
        );
        let link = pressable(move || {
          scroll.set_scroll_pending(0.0, target.y() - origin.y());
          active.set(key);
        })
        .id(format!(
          "pipeline-settings-nav-{}",
          key.trim_start_matches("pipelines.")
        ))
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(30.0))
        .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
        .rounded(RadiusSize::Md)
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .align_items(Alignment::Center)
            .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Description).flex(1.0))
            .with_children((changed && key == KEYS[0]).then(|| {
              Text::new(&ctx.t("pipelines.oneChange"))
                .variant(TypographyStyle::FieldLabel)
                .color(ExtraColor::Selection.role())
            })),
        );
        if self.active.get() == key {
          link.background(ExtraColor::Hover.role())
        } else {
          link
        }
      }))
  }
}
