//! Dependency levels on the canvas: r6PIM step cards, qXE7Z parallel groups and qATGg start.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{RepositoryAccess, Step, StepKind};
use orchester_workspace::pipelines::{describe::executor_line, fields::unused_step_name, levels::levels};

use super::{
  parts,
  state::{Panel, State},
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
  },
};
/// r6PIM.
const CARD_WIDTH: f32 = 240.0;

pub fn canvas(ctx: &mut Ctx, state: &State, busy: bool) -> Element {
  let editing = state.get();
  let shown = editing.shown();
  let readonly = editing.viewing.is_some() || busy;
  let toolbar = toolbar(ctx, state, readonly);
  let start = state.clone();
  let mut flow = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Roomy.role())
    .align_items(Alignment::Center)
    .child(
      ActionButton::new("pipeline-start", ButtonKind::Secondary, ctx.t("pipelines.onStart"))
        .on_click(move || start.update(|s| s.panel = Panel::Pipeline)),
    );
  if shown.steps().is_empty() {
    flow = flow
      .spacing(SpacingSize::Xl)
      .child(Text::new(&ctx.t("pipelines.emptySteps")).variant(TypographyStyle::Title))
      .child(parts::note(&ctx.t("pipelines.emptyStepsHint")));
  }
  for level in levels(shown.steps()) {
    flow = flow.child(icon(Icon::ArrowDown, IconSize::Lg, PaletteColor::BorderStrong));
    let mut group = Column::new().spacing(SpacingSize::Sm);
    if level.len() > 1 {
      group = group
        .background(ExtraColor::Hover.role())
        .rounded(crate::theme::pipelines::parallel_radius())
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
        .padding(
          Padding::new()
            .top(SpacingSize::Sm)
            .left(ExtraSpacing::Loose.role())
            .right(ExtraSpacing::Loose.role())
            .bottom(ExtraSpacing::Loose.role()),
        )
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .child(parts::caption(&ctx.t("pipelines.parallelTitle")))
            .child(parts::caption(&ctx.t("pipelines.parallelHint"))),
        );
    }
    let mut row = Row::new().spacing(SpacingSize::Md).justify(Justify::Center);
    for index in level {
      let step = &shown.steps()[index];
      row = row.child(card(ctx, state, step, index));
    }
    group = group.child(row);
    flow = flow.child(group);
  }
  Column::new()
    .id("pipeline-canvas")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceBase)
    .child(toolbar)
    .child(ScrollVertical::new(flow).flex(1.0).min_height(Dimension::Px(0.0)))
    .into()
}
fn toolbar(ctx: &Ctx, state: &State, readonly: bool) -> Element {
  let add_state = state.clone();
  let first = ctx.t("pipelines.newStep");
  let numbered = ctx.t("pipelines.newStepNumber");
  let add = ActionButton::new("pipeline-add-step", ButtonKind::Secondary, ctx.t("pipelines.addStep"))
    .icon(Icon::Plus)
    .on_click(move || {
      add_state.update(|s| {
        let name = unused_step_name(s.edit.steps(), &first, |n| {
          numbered.replace("{{number}}", &n.to_string())
        });
        let after = match &s.panel {
          Panel::Step(key) => Some(key.clone()),
          _ => None,
        };
        let key = s.edit.add_step(name, after.as_ref());
        s.panel = Panel::Step(key);
      });
    });
  let settings = state.clone();
  let history = state.clone();
  Row::new()
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .child(if readonly { add.disabled() } else { add })
    .child(
      ActionButton::new("pipeline-settings", ButtonKind::Ghost, ctx.t("pipelines.settings"))
        .icon(Icon::Settings)
        .on_click(move || settings.update(|s| s.panel = Panel::Settings)),
    )
    .child(
      ActionButton::new("pipeline-history", ButtonKind::Ghost, ctx.t("pipelines.history"))
        .icon(Icon::History)
        .on_click(move || history.update(|s| s.panel = Panel::History)),
    )
    .into()
}
fn card(ctx: &Ctx, state: &State, step: &Step, index: usize) -> Element {
  let editing = state.get();
  let selected = editing.panel == Panel::Step(step.key.clone());
  let key = step.key.clone();
  let select = state.clone();
  let outcome = match step.kind {
    StepKind::Tool { .. } => "pipelines.toolBadge",
    StepKind::Agent {
      repository: RepositoryAccess::ReadOnly,
      ..
    } => "pipelines.readBadge",
    _ => "pipelines.writeBadge",
  };
  let mut body = Column::new()
    .width(Dimension::Px(CARD_WIDTH))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Lg)
    .border(Border::inside(
      BorderSize::Sm,
      if selected {
        PaletteColor::Info
      } else {
        PaletteColor::BorderStrong
      },
    ))
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&format!("{:02}", index + 1)).variant(ExtraTypography::MonoNote.role()))
        .child(Text::new(step.name.as_str()).variant(ExtraTypography::Strong.role())),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(
          if matches!(step.kind, StepKind::Tool { .. }) {
            Icon::Wrench
          } else {
            Icon::Bot
          },
          IconSize::Sm,
          PaletteColor::TextMuted,
        ))
        .child(
          Text::new(&message_text(ctx, &executor_line(step, &editing.opened.assignables)))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(step_badge(&ctx.t(outcome), false));
  if step.approval_gate {
    body = body.child(step_badge(&ctx.t("pipelines.gateBadge"), true));
  }
  if let Some(route) = &step.on_failure {
    let target = editing
      .shown()
      .step(&route.return_to)
      .map(|s| s.name.as_str().to_owned())
      .unwrap_or_else(|| route.return_to.to_string());
    body = body.child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .child(icon(Icon::CornerDownLeft, IconSize::Sm, PaletteColor::TextMuted))
        .child(parts::caption(&target)),
    );
  }
  pressable(move || select.update(|s| s.panel = Panel::Step(key.clone())))
    .id(format!("pipeline-step-{}", step.key))
    .child(body)
    .into()
}
/// Co8O6: compact rectangular badge, not the round status chip.
fn step_badge(text: &str, gate: bool) -> Row {
  Row::new()
    .height(Dimension::Px(18.0))
    .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .background(if gate {
      PaletteColor::WarningMuted
    } else {
      PaletteColor::SurfaceInput
    })
    .border(Border::inside(
      BorderSize::Sm,
      if gate {
        PaletteColor::WarningMuted
      } else {
        PaletteColor::Border
      },
    ))
    .child(icon(
      if gate { Icon::Hand } else { Icon::BookOpen },
      IconSize::Xs,
      if gate {
        PaletteColor::Warning
      } else {
        PaletteColor::TextSecondary
      },
    ))
    .child(Text::new(text).variant(TypographyStyle::Caption).color(if gate {
      PaletteColor::Warning
    } else {
      PaletteColor::TextSecondary
    }))
}
