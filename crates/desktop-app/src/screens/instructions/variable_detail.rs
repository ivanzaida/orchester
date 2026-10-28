//! The chosen variable's detail in the catalog (`Detail` `M6fbYX` in 02d), and what a rule does when a variable it
//! uses has no value at its event (`When this value is unavailable` `hPvK0`).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::Variable;
use orchester_workspace::instructions::{form::Events, preview::Resolved};

use super::{panels::Props, words};
use crate::{
  screens::hub::parts::{boxed, group, head},
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText, IconSize, instructions},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};
/// The chosen variable's value at the rule's event, under what it is the value for (`context`, `x9Cv4`), then Insert.
pub fn detail(
  ctx: &Ctx,
  props: &Props,
  variable: Variable,
  resolved: Option<&Resolved<'_>>,
  context: Option<Element>,
) -> Element {
  let value = resolved.and_then(|r| r.value(variable));
  let caption = context.unwrap_or_else(|| {
    Text::new(&ctx.t_args(
      "instructions.valueAtEvent",
      [("event", words::event(ctx, props.events.get()).to_uppercase())],
    ))
    .id("instruction-variable-context")
    .variant(instructions::overline())
    .into()
  });
  let shown = match &value {
    Some(value) => Text::new(value)
      .id("instruction-variable-value")
      .variant(instructions::detail_value())
      .color(PaletteColor::TextPrimary),
    None => Text::new(&ctx.t(if variable == Variable::RunRevision {
      "instructions.unavailableCommit"
    } else {
      "instructions.unavailable"
    }))
    .id("instruction-variable-value")
    .variant(HubText::Hint.role())
    .color(PaletteColor::Warning),
  };
  Column::new()
    .id("instruction-variable-detail")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(caption)
    .child(shown.selectable(true).width(Dimension::Pct(100.0)))
    // A run's revision without a value is recovered under the prompt instead (`hPvK0`); any other variable can be
    // inserted whether or not the preview's run has its value.
    .with_children((value.is_some() || variable != Variable::RunRevision).then(|| insert(ctx, props, variable)))
    .into()
}
fn insert(ctx: &Ctx, props: &Props, variable: Variable) -> Element {
  let input = props.input.clone();
  let frozen = props.frozen.clone();
  let expression = format!("{{{{{}}}}}", variable.name());
  let button = ActionButton::new(
    "instruction-variable-insert",
    ButtonKind::Secondary,
    ctx.t_args("instructions.insert", [("expression", expression.as_str())]),
  )
  .icon(Icon::Plus)
  .on_click(move || {
    if !frozen.get_untracked() {
      input.report(input.insert(&expression));
    }
  });
  Row::new()
    .child(if props.frozen.get() { button.disabled() } else { button })
    .into()
}
/// *When this value is unavailable*: Require value, which blocks the injection, Inject at Agent resume, and a fallback
/// value that cannot be chosen yet; a radio and its words in each row (`XIcKy`). While the rule is frozen the radios
/// are faded and fixed.
pub fn policy(ctx: &Ctx, (events, frozen): (&Signal<Events>, &Signal<bool>), variable: Variable) -> Element {
  let resume = events.get() == Events::Resume;
  let fixed = frozen.get();
  let choose = |toggle: Toggle, to: Events| {
    if fixed {
      return toggle;
    }
    let (events, frozen) = (events.clone(), frozen.clone());
    toggle.on_change(move |_| {
      if !frozen.get_untracked() {
        events.set(to);
      }
    })
  };
  let require = choose(
    Toggle::new(
      "instruction-variable-require",
      ToggleKind::Radio,
      !resume,
      ctx.t("instructions.requireValue"),
    )
    .hide_label(),
    Events::Start,
  );
  let resumed = choose(
    Toggle::new(
      "instruction-variable-resume",
      ToggleKind::Radio,
      resume,
      ctx.t("instructions.resumePolicy"),
    )
    .hide_label(),
    Events::Resume,
  );
  let fallback = Toggle::new(
    "instruction-variable-fallback",
    ToggleKind::Radio,
    false,
    ctx.t("instructions.fallbackPolicy"),
  )
  .hide_label();
  let rows: Vec<Element> = vec![
    option(
      require.into(),
      &ctx.t("instructions.requireValue"),
      (
        &ctx.t_args("instructions.requireNote", [("variable", variable.name())]),
        false,
      ),
    ),
    option(
      resumed.into(),
      &ctx.t("instructions.resumePolicy"),
      (&ctx.t("instructions.resumePolicyNote"), false),
    ),
    option(
      fallback.into(),
      &ctx.t("instructions.fallbackPolicy"),
      (&ctx.t("instructions.noFallback"), true),
    ),
  ];
  let id = "instruction-variable-policy";
  group(
    id,
    head(id, &ctx.t("instructions.policyTitle"), None, None),
    boxed(&format!("{id}-rows"), rows),
  )
  .into()
}
/// A radio, then its label and what choosing it does; a locked option's words follow a lock.
fn option(radio: Element, label: &str, (hint, locked): (&str, bool)) -> Element {
  let hint = Text::new(hint)
    .variant(HubText::Hint.role())
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  let hint: Element = if locked {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted))
      .child(hint)
      .into()
  } else {
    hint.width(Dimension::Pct(100.0)).into()
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(HubSpacing::FieldGap.role())
    .align_items(Alignment::Start)
    .child(radio)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(label).variant(ExtraTypography::Strong.role()))
        .child(hint),
    )
    .into()
}
