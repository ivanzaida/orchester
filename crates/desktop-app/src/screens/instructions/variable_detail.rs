//! Available and missing-value detail x5WE6i / kdAW2.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::Variable;
use orchester_workspace::instructions::{form::Events, preview::Resolved};

use super::{panels::Props, variables, words};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, instructions},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};
pub fn detail(ctx: &Ctx, props: &Props, variable: Variable, resolved: Option<&Resolved<'_>>) -> Element {
  let value = resolved.and_then(|r| r.value(variable));
  let missing = value.is_none();
  let head = head(ctx, variable);
  let box_value = value_box(ctx, props, variable, value);
  let mut detail = Column::new()
    .id("instruction-variable-detail")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Inline.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .border_inside(
      BorderSize::Sm,
      if missing {
        PaletteColor::Warning
      } else {
        PaletteColor::Border
      },
    )
    .child(head)
    .child(Text::new(&variables::description(ctx, variable)).variant(TypographyStyle::Description))
    .child(box_value);
  if missing {
    let note = if variable == Variable::RunRevision {
      resolved.map_or_else(
        || ctx.t("instructions.noEmptyValue"),
        |r| {
          ctx.t_args(
            "instructions.revisionExplanation",
            [("role", r.role()), ("step", r.step.name.as_str())],
          )
        },
      )
    } else {
      ctx.t("instructions.noEmptyValue")
    };
    detail = detail.child(Text::new(&note).variant(TypographyStyle::Description));
    if variable == Variable::RunRevision {
      detail = detail.child(policy(ctx, props, variable));
    }
  } else {
    detail = detail.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(icon(Icon::CircleCheck, IconSize::Xs, PaletteColor::Success))
        .child(
          Text::new(&ctx.t_args(
            "instructions.available",
            [("group", variables::group_label(ctx, variable.group()).to_lowercase())],
          ))
          .variant(TypographyStyle::Caption)
          .flex(1.0),
        ),
    );
  }
  // The missing revision frame offers recovery policies instead of an insertion action.
  if missing && variable == Variable::RunRevision {
    return detail.into();
  }
  detail.child(insert(ctx, props, variable)).into()
}
fn insert(ctx: &Ctx, props: &Props, variable: Variable) -> Element {
  let input = props.input.clone();
  let frozen = props.frozen.clone();
  let expression = format!("{{{{{}}}}}", variable.name());
  let button = ActionButton::new(
    "instruction-variable-insert",
    ButtonKind::Primary,
    ctx.t_args("instructions.insert", [("expression", expression.as_str())]),
  )
  .icon(Icon::CornerDownLeft)
  .on_click(move || {
    if !frozen.get_untracked() {
      input.report(input.insert(&expression));
    }
  });
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(if props.frozen.get() { button.disabled() } else { button })
    .child(
      Text::new(&ctx.t("instructions.enter"))
        .variant(TypographyStyle::Caption)
        .padding(Padding::symmetric(
          ExtraSpacing::Pair.role(),
          ExtraSpacing::Tight.role(),
        ))
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .rounded(ExtraRadius::Tile.role())
        .opacity(crate::theme::DISABLED_OPACITY),
    )
    .into()
}
fn policy(ctx: &Ctx, props: &Props, variable: Variable) -> Element {
  let events = props.events.clone();
  let frozen = props.frozen.clone();
  let resume = Toggle::new(
    "instruction-variable-resume",
    ToggleKind::Radio,
    props.events.get() == Events::Resume,
    ctx.t("instructions.resumePolicy"),
  );
  let resume = if props.frozen.get() {
    resume
  } else {
    resume.on_change(move |_| {
      if !frozen.get_untracked() {
        events.set(Events::Resume);
      }
    })
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("instructions.policyTitle")).variant(TypographyStyle::FieldLabel))
    .child(
      Toggle::new(
        "instruction-variable-require",
        ToggleKind::Radio,
        props.events.get() != Events::Resume,
        ctx.t("instructions.requireValue"),
      )
      .on_change({
        let events = props.events.clone();
        let frozen = props.frozen.clone();
        move |_| {
          if !frozen.get_untracked() {
            events.set(Events::Start);
          }
        }
      }),
    )
    .child(
      Text::new(&ctx.t_args("instructions.requireNote", [("variable", variable.name())]))
        .variant(TypographyStyle::Caption),
    )
    .child(resume)
    .child(Text::new(&ctx.t("instructions.resumePolicyNote")).variant(TypographyStyle::Caption))
    .child(Toggle::new(
      "instruction-variable-fallback",
      ToggleKind::Radio,
      false,
      ctx.t("instructions.fallbackPolicy"),
    ))
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("instructions.noFallback"))
            .variant(TypographyStyle::Caption)
            .flex(1.0),
        ),
    )
    .into()
}
fn head(ctx: &Ctx, variable: Variable) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(variable.name())
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)))
    .child(
      Text::new(&ctx.t_args(
        "instructions.variableTypeGroup",
        [
          ("type", variables::value_type(ctx, variable.value_type()).to_string()),
          ("group", variables::group_label(ctx, variable.group()).to_string()),
        ],
      ))
      .variant(TypographyStyle::Caption)
      .nowrap(),
    )
}
fn value_box(ctx: &Ctx, props: &Props, variable: Variable, value: Option<String>) -> Column {
  let missing = value.is_none();
  let value_text = value.unwrap_or_else(|| {
    ctx
      .t(if variable == Variable::RunRevision {
        "instructions.unavailableCommit"
      } else {
        "instructions.unavailable"
      })
      .to_string()
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inline.role(),
      ExtraSpacing::TextArea.role(),
    ))
    .spacing(ExtraSpacing::Fine.role())
    .background(ExtraColor::CodeBackground.role())
    .rounded(ExtraRadius::Tile.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(
      Text::new(&ctx.t_args(
        "instructions.valueAtEvent",
        [("event", words::event(ctx, props.events.get()).to_uppercase())],
      ))
      .variant(instructions::overline()),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .with_children(missing.then(|| icon(Icon::CircleHelp, IconSize::Xs, PaletteColor::Warning)))
        .child(
          Text::new(&value_text)
            .id("instruction-variable-value")
            .variant(if missing {
              TypographyStyle::Description
            } else {
              TypographyStyle::Mono
            })
            .color(if missing {
              PaletteColor::Warning
            } else {
              PaletteColor::TextPrimary
            })
            .selectable(true)
            .flex(1.0),
        ),
    )
}
