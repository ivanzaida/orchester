//! The *Fix CI automatically* row under the CI card's top border (`docs/design/pr-ci-status.md`, "Fix CI
//! automatically"): the switch and its label, *up to* in 12 `text_muted`, the limit's select (*1, 2, 3, 5 fixes*), and
//! at the row's end how far it got (*none used*, *1 of 3 automatic fixes used on this run*, `Badge/Warning` *Limit
//! reached* with *3 of 3 used*) or why it waits (*Off · CI waits for you when it fails*). While CI cannot be read the
//! switch and *up to* are off and faded to half and the select is locked, with *Needs gh to read checks*. Flipping the
//! switch or choosing another limit sets it for this run only, which restarts its count; only a person sets it, so
//! automation driving the window is refused and cannot lift the run's limit.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::RunId,
  pull_request::ci::fix::{AutoFix, FixLimit, RunFixes},
  run::Answerer,
};
use parking_lot::Mutex;

use super::actions::{AutoRequest, CiActions};
use crate::{
  mcp::input_gate::{InputGate, persons_press},
  theme::DISABLED_OPACITY,
  ui::{
    chip::{BadgeTone, badge},
    field::SelectField,
    toggle::{Toggle, ToggleKind},
  },
};

/// The limit's select (`IL2Ph` in `WvQKS`).
const LIMIT_WIDTH: f32 = 112.0;
/// What the switch and the limit set, as an MCP client refused reads it: a change restarts the run's count, so it is a
/// person's alone.
const AUTO_FIX: &str = "change how a run's CI is fixed automatically";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoProps {
  /// The card's element id, which the row's ids start with.
  pub card: Arc<str>,
  pub root: PathBuf,
  pub run: RunId,
  pub fixes: RunFixes,
  /// CI was read: the switch can be used.
  pub readable: bool,
  /// The agent a Fix CI goes to.
  pub agent: Arc<str>,
}

impl lurq::app::component::DevtoolsInspectable for AutoProps {}

pub struct AutoRow {
  limit: Signal<u32>,
  /// The switch as stored, which a choice in the select is told from.
  stored: Arc<Mutex<Option<(RunId, AutoFix)>>>,
}

impl Component for AutoRow {
  type Props = AutoProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<AutoProps>().clone();
    let row = Self {
      limit: ctx.signal(props.fixes.auto.limit.get()),
      stored: Arc::new(Mutex::new(Some((props.run, props.fixes.auto)))),
    };
    let (stored, actions, root) = (row.stored.clone(), CiActions::of(ctx), props.root);
    // Signals change while lurq delivers the input that changed them, so a client's choice is told from a person's:
    // only a person sets it (`set_auto_fix_in`), and a client's is refused to it; the next render puts the select back.
    let (auto_action, gate) = (actions.auto.clone(), ctx.use_context::<InputGate>());
    ctx.watch(&row.limit, move |count: &u32| {
      let Some(limit) = FixLimit::of(*count) else {
        return;
      };
      let Some((run, auto)) = *stored.lock() else {
        return;
      };
      if auto.limit != limit && persons_press(gate.as_ref(), AUTO_FIX) {
        auto_action.run(AutoRequest {
          root: root.clone(),
          run,
          auto: AutoFix { limit, ..auto },
          by: Answerer::Person,
        });
      }
    });
    // A refused choice (automation driving the window) leaves the select on the stored limit. The lock is let go
    // before the select changes, as the select's watch takes it again.
    let (stored, limit) = (row.stored.clone(), row.limit.clone());
    ctx.watch(&actions.auto.state(), move |result| {
      if !result.is_rejected() {
        return;
      }
      let kept = stored.lock().map(|(_, auto)| auto.limit.get());
      if let Some(kept) = kept
        && limit.get_untracked() != kept
      {
        limit.set(kept);
      }
    });
    row
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AutoProps>().clone();
    let auto = props.fixes.auto;
    *self.stored.lock() = Some((props.run, auto));
    if self.limit.get_untracked() != auto.limit.get() {
      self.limit.set(auto.limit.get());
    }
    let id = format!("{}-auto", props.card);
    let actions = CiActions::of(ctx);
    let switch = Toggle::new(
      format!("{id}-switch"),
      ToggleKind::Switch,
      auto.on && props.readable,
      ctx.t("ci.auto.label"),
    )
    .label_style(TypographyStyle::Description);
    let switch = if props.readable {
      let (root, run, gate) = (props.root.clone(), props.run, ctx.use_context::<InputGate>());
      switch.on_change(move |on| {
        if persons_press(gate.as_ref(), AUTO_FIX) {
          actions.auto.run(AutoRequest {
            root: root.clone(),
            run,
            auto: AutoFix { on, ..auto },
            by: Answerer::Person,
          });
        }
      })
    } else {
      switch.settled()
    };
    let mut lead = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(switch)
      .child(
        Text::new(&ctx.t("ci.auto.upTo"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      );
    if !props.readable {
      lead = lead.opacity(DISABLED_OPACITY);
    }
    Row::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Md))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(lead)
      .child(self.select(ctx, &id, props.readable))
      .child(Row::new().flex(1.0))
      .child(end(ctx, &id, &props))
  }
}

impl AutoRow {
  /// The limit's select, locked while CI cannot be read.
  fn select(&self, ctx: &Ctx, id: &str, readable: bool) -> Element {
    let options = FixLimit::ALL
      .into_iter()
      .map(|limit| SelectOption::new(limit.get(), fixes(ctx, limit.get())))
      .collect();
    let select = SelectField::new(
      format!("{id}-limit"),
      ctx.t("ci.auto.upTo"),
      self.limit.clone(),
      options,
    )
    .width(Dimension::Px(LIMIT_WIDTH));
    if readable { select } else { select.locked() }.render(ctx)
  }
}

/// *3 fixes*.
fn fixes(ctx: &Ctx, count: u32) -> Arc<str> {
  if count == 1 {
    ctx.t("ci.auto.fixes.one")
  } else {
    ctx.t_args("ci.auto.fixes.other", [("count", count.to_string())])
  }
}

/// How far the switch got, or why it waits, at the row's end.
fn end(ctx: &Ctx, id: &str, props: &AutoProps) -> Element {
  let fixes = &props.fixes;
  let (used, limit) = (fixes.used().to_string(), fixes.auto.limit.get().to_string());
  let hint = |words: Arc<str>| -> Element {
    Text::new(&words)
      .id(format!("{id}-hint"))
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap()
      .into()
  };
  if !props.readable {
    return hint(ctx.t("ci.auto.needsGh"));
  }
  if !fixes.auto.on {
    return hint(ctx.t("ci.auto.offHint"));
  }
  if fixes.limit_reached() {
    return Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(badge(
        &format!("{id}-limit-reached"),
        BadgeTone::Warning,
        &ctx.t("ci.state.limitReached"),
      ))
      .child(hint(
        ctx.t_args("ci.auto.limitReached", [("used", used), ("limit", limit)]),
      ))
      .into();
  }
  if fixes.used() == 0 {
    return hint(ctx.t_args("ci.auto.noneUsed", [("agent", props.agent.to_string())]));
  }
  hint(ctx.t_args("ci.auto.used", [("used", used), ("limit", limit)]))
}
