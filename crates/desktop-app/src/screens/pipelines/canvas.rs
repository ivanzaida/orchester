//! Dependency levels on the canvas: r6PIM step cards, qXE7Z parallel groups and qATGg start.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollBoth, Stack, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{RepositoryAccess, Step, StepKind};
use orchester_workspace::pipelines::{describe::executor_line, fields::unused_step_name, levels::levels};

use super::{
  parts,
  state::{Panel, State},
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, pipelines::*},
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
  let readonly = editing.viewing.is_some() || busy;
  let toolbar = toolbar(ctx, state, readonly);
  let flow = graph(ctx, state);
  Stack::new()
    .id("pipeline-canvas")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .background(crate::theme::pipelines::canvas_color())
    .child(
      ScrollBoth::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .min_width(Dimension::Px(graph_width(&editing.shown())))
          .align_items(Alignment::Center)
          .child(flow),
      )
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0)),
    )
    .child(Column::new().child(toolbar).absolute_position(12.0, 12.0))
    .into()
}
fn graph(ctx: &Ctx, state: &State) -> Element {
  let editing = state.get();
  let shown = editing.shown();
  let top = if shown.steps().is_empty() {
    EMPTY_TOP
  } else if editing.viewing.is_some() {
    GRAPH_TOP + 28.0
  } else {
    GRAPH_TOP
  };
  let center = if shown.steps().is_empty() {
    GRAPH_WIDTH / 2.0
  } else {
    GRAPH_CENTER
  };
  let width = graph_width(&shown);
  let center = center + (width - GRAPH_WIDTH) / 2.0;
  let mut graph = Stack::new().width(Dimension::Px(width));
  let start = state.clone();
  graph = graph.child(
    Column::new()
      .child(
        pressable(move || start.update(|s| s.panel = Panel::Pipeline))
          .id("pipeline-start")
          .child(start_node(ctx)),
      )
      .absolute_position(center - 64.0, top),
  );
  let mut y = top + 26.0;
  let mut positions = std::collections::BTreeMap::new();
  for level in levels(shown.steps()) {
    graph = graph.child(Column::new().child(edge(ctx)).absolute_position(center - 6.0, y));
    y += EDGE_HEIGHT;
    let parallel = level.len() > 1;
    let width = level.len() as f32 * (CARD_WIDTH + CARD_GAP) - CARD_GAP;
    let x = center - width / 2.0;
    let height = level
      .iter()
      .map(|i| {
        CARD_HEIGHT
          + if shown.steps()[*i].approval_gate {
            GATE_HEIGHT
          } else {
            0.0
          }
      })
      .fold(CARD_HEIGHT, f32::max);
    if parallel {
      graph = graph.child(
        Column::new()
          .child(parallel_frame(ctx, width, height))
          .absolute_position(x - 12.0, y),
      );
      y += PARALLEL_HEAD;
    }
    for (column, index) in level.into_iter().enumerate() {
      let step = &shown.steps()[index];
      let card_x = x + column as f32 * (CARD_WIDTH + CARD_GAP);
      positions.insert(step.key.clone(), (card_x, y, height));
      graph = graph.child(
        Column::new()
          .child(card(ctx, state, step, index))
          .absolute_position(card_x, y),
      );
    }
    y += height + if parallel { PARALLEL_FOOT } else { 0.0 };
  }
  if shown.steps().is_empty() {
    graph = empty_graph(ctx, state, graph, y);
    y += 200.0;
  }
  graph = super::graph_routes::routes(ctx, graph, &shown, &positions);
  graph.height(Dimension::Px(y + GRAPH_TOP)).into()
}
pub fn start_node(ctx: &Ctx) -> Row {
  Row::new()
    .height(Dimension::Px(26.0))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(crate::theme::ExtraRadius::Full.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .child(icon(Icon::Zap, IconSize::Sm, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t("pipelines.onStart")).variant(ExtraTypography::MonoSmall.role()))
}
pub fn edge(ctx: &Ctx) -> Column {
  Column::new()
    .width(Dimension::Px(12.0))
    .height(Dimension::Px(EDGE_HEIGHT))
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .width(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
        .flex(1.0)
        .background(PaletteColor::BorderStrong),
    )
    .child(icon(Icon::ChevronDown, IconSize::Xs, PaletteColor::BorderStrong))
}

fn add_button(ctx: &Ctx, state: &State, id: &str) -> ActionButton {
  ActionButton::new(id, ButtonKind::Ghost, ctx.t("pipelines.addStep"))
    .icon(Icon::Plus)
    .on_click(add_action(ctx, state))
}
fn add_action(ctx: &Ctx, state: &State) -> impl Fn() + Send + Sync + 'static {
  let add_state = state.clone();
  let first = ctx.t("pipelines.newStep");
  let numbered = ctx.t("pipelines.newStepNumber");
  move || {
    add_state.update(|s| {
      if s.viewing.is_some() {
        return;
      }
      let name = unused_step_name(s.edit.steps(), &first, |n| {
        numbered.replace("{{number}}", &n.to_string())
      });
      let after = match &s.panel {
        Panel::Step(key) => Some(key.clone()),
        _ => None,
      };
      let key = s.edit.add_step(name, after.as_ref());
      s.panel = Panel::Step(key);
    })
  }
}
fn toolbar(ctx: &Ctx, state: &State, readonly: bool) -> Element {
  if state.get().viewing.is_some() {
    return super::history::toolbar(ctx, state);
  }
  let add = add_button(ctx, state, "pipeline-add-step");
  let settings = state.clone();
  let history = state.clone();
  Row::new()
    .padding(SpacingSize::Xs)
    .background(PaletteColor::SurfaceRaised)
    .rounded(crate::theme::ExtraRadius::Full.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .spacing(SpacingSize::Xs)
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
  let body = card_body(ctx, &editing, step, index, selected);
  let reference = if editing.comparing {
    Some(&editing.opened.body)
  } else if editing.panel == Panel::Publish {
    editing.opened.latest.as_ref().map(|v| &v.body)
  } else {
    None
  };
  if reference.is_some_and(|before| before.steps().iter().find(|s| s.key == step.key) != Some(step)) {
    let tagged = Stack::new().child(body).child(
      Row::new()
        .padding(Padding::horizontal(SpacingSize::Sm))
        .spacing(SpacingSize::Xs)
        .rounded(crate::theme::ExtraRadius::Full.role())
        .background(PaletteColor::SurfacePanel)
        .border(Border::inside(BorderSize::Sm, ExtraColor::Selection.role()))
        .child(icon(Icon::FilePen, IconSize::Xs, ExtraColor::Selection.role()))
        .child(parts::caption(&ctx.t("pipelines.changed")).color(ExtraColor::Selection.role()))
        .hit_test(lurq::node::HitTestBehavior::None)
        .absolute_position(CARD_WIDTH - 88.0, 0.0),
    );
    return pressable(move || select.update(|s| s.panel = Panel::Step(key.clone())))
      .id(format!("pipeline-step-{}", step.key))
      .child(tagged)
      .into();
  }
  pressable(move || select.update(|s| s.panel = Panel::Step(key.clone())))
    .id(format!("pipeline-step-{}", step.key))
    .child(body)
    .into()
}
/// Co8O6: compact rectangular badge, not the round status chip.
fn step_badge(text: &str, glyph: Icon) -> Row {
  let gate = glyph == Icon::Hand;
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
      glyph,
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
fn empty_graph(ctx: &Ctx, state: &State, mut graph: Stack, y: f32) -> Stack {
  graph = graph.child(
    Column::new()
      .child(edge(ctx))
      .absolute_position((GRAPH_WIDTH / 2.0) - 6.0, y),
  );
  graph = graph.child(
    Column::new()
      .child(crate::ui::add_slot::add_slot(
        ctx,
        "pipeline-add-first",
        &ctx.t("pipelines.emptySteps"),
        &ctx.t("pipelines.emptyStepsHint"),
        add_action(ctx, state),
      ))
      .absolute_position((GRAPH_WIDTH / 2.0) - CARD_WIDTH / 2.0, y + EDGE_HEIGHT),
  );
  let starter = super::new_pipeline::starter(ctx);
  let target = state.clone();
  graph = graph.child(
    Column::new()
      .child(
        ActionButton::new("pipeline-use-starter", ButtonKind::Ghost, ctx.t("pipelines.useStarter"))
          .icon(Icon::Workflow)
          .on_click(move || {
            target.update(|s| {
              if s.viewing.is_some() {
                return;
              }
              let description = s.edit.description().to_owned();
              s.edit = orchester_core::resource::PipelineEdit::new(
                starter.pipeline(&s.opened.assignables).with_description(description),
              );
              s.epoch += 1;
            })
          }),
      )
      .absolute_position(
        (GRAPH_WIDTH / 2.0) - 70.0,
        y + EDGE_HEIGHT + crate::ui::add_slot::HEIGHT + 12.0,
      ),
  );

  graph
}
fn card_body(ctx: &Ctx, editing: &super::state::Editing, step: &Step, index: usize, selected: bool) -> Column {
  let invalid = orchester_workspace::pipelines::narrowing::check_step(step, &editing.opened.assignables).is_err()
    || matches!(
      step.kind,
      StepKind::Agent { profile: None, .. } | StepKind::Tool { tool: None }
    );
  let mut body = Column::new()
    .width(Dimension::Px(CARD_WIDTH))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Lg)
    .border(Border::inside(
      BorderSize::Sm,
      if selected {
        ExtraColor::Selection.role()
      } else {
        PaletteColor::BorderStrong
      },
    ))
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&format!("{:02}", index + 1)).variant(ExtraTypography::MonoNote.role()))
        .child(
          Text::new(step.name.as_str())
            .variant(ExtraTypography::Strong.role())
            .flex(1.0),
        )
        .with_children(invalid.then(|| icon(Icon::CircleAlert, IconSize::Sm, PaletteColor::Danger))),
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
            .color(if invalid {
              PaletteColor::Danger
            } else {
              PaletteColor::TextSecondary
            }),
        ),
    );
  if step.approval_gate {
    body = body.child(step_badge(&ctx.t("pipelines.gateBadge"), Icon::Hand));
  }
  body = body.child(outcome_badge(ctx, step, invalid));
  if invalid && !selected {
    body = body.border(Border::inside(BorderSize::Sm, PaletteColor::Danger));
  }
  body
}
fn outcome_badge(ctx: &Ctx, step: &Step, invalid: bool) -> Row {
  let outcome = match step.kind {
    StepKind::Tool { .. } => "pipelines.toolBadge",
    StepKind::Agent {
      repository: RepositoryAccess::ReadOnly,
      ..
    } => "pipelines.readBadge",
    _ => "pipelines.writeBadge",
  };
  step_badge(
    &ctx.t(if matches!(step.kind, StepKind::Tool { .. }) && invalid {
      "pipelines.toolUnavailableBadge"
    } else {
      outcome
    }),
    match step.kind {
      StepKind::Tool { .. } if invalid => Icon::Lock,
      StepKind::Tool { .. } => Icon::Terminal,
      StepKind::Agent {
        repository: RepositoryAccess::Write,
        ..
      } => Icon::FilePen,
      _ => Icon::BookOpen,
    },
  )
}

fn parallel_frame(ctx: &Ctx, width: f32, height: f32) -> Column {
  Column::new()
    .width(Dimension::Px(width + 24.0))
    .height(Dimension::Px(height + PARALLEL_HEAD + PARALLEL_FOOT))
    .background(ExtraColor::Hover.role())
    .rounded(parallel_radius())
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .padding(SpacingSize::Sm)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(icon(Icon::GitBranch, IconSize::Sm, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("pipelines.parallelTitle")).variant(ExtraTypography::MonoSmall.role()))
        .child(parts::caption(&ctx.t("pipelines.parallelHint"))),
    )
}
fn graph_width(shown: &orchester_core::resource::PipelineEdit) -> f32 {
  let widest = levels(shown.steps()).iter().map(Vec::len).max().unwrap_or(0);
  (widest as f32 * (CARD_WIDTH + CARD_GAP) + 88.0).max(GRAPH_WIDTH)
}
