//! The runs-on picker's Effort column (`Effort` `Js9xG` in `v6mWp8`): 196 wide, 8 over and under and 6 at its sides,
//! 2 apart, a `border` stroke on its left. Its 26-high head says *Effort* (12 `text_muted`) and the chosen model (Geist
//! Mono 10.5 `text_muted`); then *Default* with *the CLI's own* (11 `text_muted`) and the efforts the connection lists
//! for the model (12), the chosen one checked. An API provider lists no efforts (05b `wzbRG`, 6 apart): after *Default*
//! its effort is typed in a field, Enter uses it and nothing typed is *Default*, with a hint that says so. A CLI
//! connection that lists no efforts has no column.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::ModelId;

use super::{item::item, keys::EffortItem, picking::Chooser};
use crate::{
  screens::orchestrator::settings::effort_word,
  theme::{ControlHeight, ExtraSpacing, OrchestratorText},
  ui::{field::FieldInput, icon::Icon},
};

/// The column's width (`Js9xG`) and its head's height (`MvASh`).
const WIDTH: f32 = 196.0;
const HEAD_HEIGHT: f32 = 26.0;

/// The column for `model`, of `efforts`, `chosen` checked (`None` the CLI's own) and the keys at `cursor`.
pub(super) fn column(
  ctx: &mut Ctx,
  (model, efforts): (&ModelId, &[EffortItem]),
  (chosen, cursor): (Option<&str>, Option<usize>),
  chooser: &Chooser,
) -> Element {
  let typed = efforts.contains(&EffortItem::Typed);
  let rows: Vec<Element> = efforts
    .iter()
    .enumerate()
    .flat_map(|(index, effort)| {
      let current = cursor == Some(index);
      match effort {
        EffortItem::Typed => typed_field(ctx, chooser, current),
        EffortItem::Default | EffortItem::Listed(_) => vec![choice(ctx, (index, effort), (chosen, current), chooser)],
      }
    })
    .collect();
  // An API provider's column keeps its field and hint 6 apart (`wzbRG`); a CLI's efforts are 2 apart (`Js9xG`).
  let gap = if typed {
    ExtraSpacing::Snug.role()
  } else {
    ExtraSpacing::Tight.role()
  };
  Column::new()
    .id("orchestrator-runs-on-effort")
    .width(Dimension::Px(WIDTH))
    .flex_shrink(0.0)
    .padding(Padding::symmetric(ExtraSpacing::Snug.role(), SpacingSize::Sm))
    .spacing(gap)
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(head(ctx, model))
    .with_children(rows)
    .into()
}

/// *Effort* and the chosen model.
fn head(ctx: &Ctx, model: &ModelId) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.effort"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .nowrap(),
    )
    .child(
      Text::new(model.as_str())
        .id("orchestrator-runs-on-effort-model")
        .variant(OrchestratorText::PlanUse.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0)),
    )
}

/// *Default* or a listed effort, the `index`th place, checked when it is `chosen`.
fn choice(
  ctx: &Ctx,
  (index, effort): (usize, &EffortItem),
  (chosen, current): (Option<&str>, bool),
  chooser: &Chooser,
) -> Element {
  let (parts, value): (Vec<Element>, Option<String>) = match effort {
    EffortItem::Listed(effort) => (vec![label(&effort_word(ctx, effort), true)], Some(effort.clone())),
    EffortItem::Default | EffortItem::Typed => (
      vec![
        label(&ctx.t("orchestrator.runsOn.effortDefault"), false),
        Text::new(&ctx.t("orchestrator.runsOn.effortDefaultDetail"))
          .variant(TypographyStyle::Caption)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .into(),
      ],
      None,
    ),
  };
  let picked = value.as_deref() == chosen;
  let chooser = chooser.clone();
  item(
    format!("orchestrator-runs-on-effort-{index}"),
    parts,
    (picked, current),
    move || chooser.effort(value.clone()),
  )
}

/// An API provider's effort, typed (`Field` `z6ovY`: 30 high, 2 in from the column's sides, *Type an effort…* and a 12
/// `corner-down-left`), and the hint under it (`Hint` `amaNe`, 11/1.45 `text_muted`, 8 in); `current` while the keys
/// are on it.
fn typed_field(ctx: &mut Ctx, chooser: &Chooser, current: bool) -> Vec<Element> {
  let placeholder = ctx.t("orchestrator.runsOn.typeEffort");
  let field = FieldInput::new(
    "orchestrator-runs-on-effort-typed",
    placeholder.clone(),
    chooser.picking.typed_effort.clone(),
  )
  .placeholder(placeholder)
  .box_height(ControlHeight::Row.value())
  .trailing(Icon::CornerDownLeft)
  .element(chooser.picking.effort_field.clone())
  .render(ctx);
  let field = Row::new()
    .width(Dimension::Pct(100.0))
    .describe("current", current.to_string())
    .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
    .child(field);
  let hint = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.typeEffortHint"))
        .id("orchestrator-runs-on-effort-hint")
        .variant(TypographyStyle::Caption)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  vec![field.into(), hint.into()]
}

/// An effort's words, filling the row when `fills`.
fn label(words: &str, fills: bool) -> Element {
  let text = Text::new(words)
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextPrimary)
    .nowrap();
  if fills {
    text.flex(1.0).min_width(Dimension::Px(0.0)).into()
  } else {
    text.into()
  }
}
