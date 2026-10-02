//! A step execution's header in the timeline (`mDnc3`): disclosure, step number, step and agent,
//! engine and time span, tokens with those before the first edit, and state. A tool step names its
//! tool and runs, and has no model and no tokens. A step that never ran after a failure is faded
//! and reads *Not started* (`hep0l`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::tokens::compact_tokens;

use super::{super::heading::step_state, engine, first_edit, toggles::Toggles};
use crate::{
  display,
  tasks::timeline::StepHead,
  theme::{ExtraColor, ExtraSpacing, UNPARSED_OPACITY},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    status_label::compact_status_label,
  },
};

/// Pressing the header opens or closes the step; pressing its tokens, when `tokens` says what to do
/// then, opens the Details pane on it.
pub(in crate::screens::project::task) fn step_header(
  ctx: &Ctx,
  (head, key, open): (&StepHead, &Arc<str>, bool),
  toggles: &Toggles,
  tokens: Option<&Arc<dyn Fn() + Send + Sync>>,
) -> Element {
  let ran = head.execution.is_some();
  let title = ctx.t_args(
    "runs.activity.stepTitle",
    [("step", &*head.step), ("agent", &*head.agent)],
  );
  let tokens = (!head.command && ran).then(|| {
    let used = ctx.t_args(
      "runs.tokens",
      [("count", compact_tokens(head.usage.counted()).as_str())],
    );
    let used = first_edit::header_tokens(ctx, used, head.before_edit.as_ref(), head.ended.is_some());
    let text = Text::new(&used).variant(TypographyStyle::Mono).nowrap();
    match tokens {
      Some(press) => {
        let press = press.clone();
        Element::from(pressable(surface().id("activity-step-tokens"), move || press()).child(text))
      }
      None => text.into(),
    }
  });
  let (glyph, state) = if ran {
    step_state(head.status)
  } else {
    (Icon::CircleDashed, "runs.step.notStarted")
  };
  let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(chevron, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(&format!("{:02}", head.number))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted),
    )
    .child(Text::new(&title).variant(TypographyStyle::Label).nowrap())
    .with_children(ran.then(|| {
      Text::new(&header_meta(ctx, head))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }))
    .child(Spacer::new().flex(1.0))
    .with_children(tokens)
    .child(compact_status_label(glyph, &ctx.t(state)));
  let header = if ran {
    let (toggles, key) = (toggles.clone(), key.clone());
    pressable(surface(), move || toggles.flip(&key, open)).child(line)
  } else {
    surface().opacity(UNPARSED_OPACITY).child(line)
  };
  header
    .id("activity-step-header")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Group.role())
    .padding_bottom(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .describe(
      "execution",
      head
        .execution
        .map(|execution| execution.to_string())
        .unwrap_or_default(),
    )
    .describe("open", open.to_string())
    .into()
}

/// "Codex / Sol · 1 invocation · 12:01–12:02", "Command tool · run 1 · started 12:05".
fn header_meta(ctx: &Ctx, header: &StepHead) -> Arc<str> {
  let start = header.started.map(display::clock).unwrap_or_default();
  let count = header.invocations.to_string();
  let (engine, running, one, other) = if header.command {
    (
      ctx.t("runs.activity.commandTool"),
      "runs.activity.commandHeaderRunning",
      "runs.activity.commandHeaderFinished.one",
      "runs.activity.commandHeaderFinished.other",
    )
  } else {
    let engine = engine(ctx, header.vendor.as_ref(), &header.model);
    // Orchester runs a native agent itself (04.H10 `rL1aI`).
    let engine = match header.provider {
      Some(provider) if provider.is_native() => ctx.t_args("runs.activity.engineNative", [("engine", &*engine)]),
      _ => engine,
    };
    (
      engine,
      "runs.activity.headerRunning",
      "runs.activity.headerFinished.one",
      "runs.activity.headerFinished.other",
    )
  };
  let Some(ended) = header.ended else {
    return ctx.t_args(running, [("engine", &*engine), ("count", &count), ("start", &start)]);
  };
  let key = if header.invocations == 1 { one } else { other };
  let end = display::clock(ended);
  ctx.t_args(
    key,
    [
      ("engine", &*engine),
      ("count", &count),
      ("start", &start),
      ("end", &end),
    ],
  )
}
