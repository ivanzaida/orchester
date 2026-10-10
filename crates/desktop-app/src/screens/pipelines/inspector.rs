//! Pipeline summary and step inspector (c24Tv / SVBQi).
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};

use super::{
  parts,
  state::{Panel, State},
  step_fields::{Fields, Props},
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  field::FieldInput,
  inspector::{PropertyValue, property},
  message::message_text,
};

pub fn panel(ctx: &mut Ctx, state: &State, busy: bool) -> Element {
  let editing = state.get();
  let shown = editing.shown();
  if let Panel::Step(key) = &editing.panel
    && let Some(step) = shown.step(key)
  {
    let props = Props {
      state: state.clone(),
      step: step.clone(),
      locked: busy || editing.viewing.is_some(),
    };
    return ctx.mount_keyed::<StepInspector>(&format!("step-inspector-{key}-{}", editing.epoch), props);
  }
  ctx.mount_keyed::<Summary>(
    &format!("pipeline-summary-{}", editing.epoch),
    SummaryProps {
      state: state.clone(),
      locked: busy || editing.viewing.is_some(),
    },
  )
}
#[derive(Clone)]
pub struct SummaryProps {
  pub state: State,
  pub locked: bool,
}
impl PartialEq for SummaryProps {
  fn eq(&self, other: &Self) -> bool {
    self.locked == other.locked
  }
}
impl DevtoolsInspectable for SummaryProps {}
pub struct Summary {
  description: Signal<String>,
}
impl Component for Summary {
  type Props = SummaryProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      description: ctx.signal(ctx.props::<SummaryProps>().state.get().shown().description().to_owned()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    let editing = props.state.get();
    let (state, locked) = (props.state.clone(), props.locked);
    ctx.watch(&self.description, move |text| {
      if !locked {
        state.update(|s| s.edit.set_description(text));
      }
    });
    let description = FieldInput::new(
      "pipeline-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .compact_area();
    let description = if props.locked {
      description.locked()
    } else {
      description
    }
    .render(ctx);
    let mut body = summary_metadata(ctx, &editing, description);
    let history = props.state.clone();
    body = body.child(
      ActionButton::new(
        "pipeline-summary-history",
        ButtonKind::Secondary,
        ctx.t("pipelines.history"),
      )
      .on_click(move || history.update(|s| s.panel = Panel::History)),
    );
    if editing.opened.latest.is_none() {
      body = body.child(crate::ui::inspector::note(
        "pipeline-unpublished-note",
        crate::ui::icon::Icon::Info,
        &ctx.t("pipelines.createHint"),
      ));
    }
    let state = props.state;
    body = body.child(
      ActionButton::new(
        "pipeline-summary-settings",
        ButtonKind::Secondary,
        ctx.t("pipelines.settings"),
      )
      .on_click(move || state.update(|s| s.panel = Panel::Settings)),
    );
    parts::panel(
      ctx,
      &ctx.t("pipelines.pipeline"),
      &editing.opened.name,
      body.into(),
      None,
      None,
    )
  }
}
pub struct StepInspector {
  removing: Signal<bool>,
}
impl Component for StepInspector {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      removing: ctx.signal(false),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let editing = props.state.get();
    let fields = ctx.mount_keyed::<Fields>("step-fields", props.clone());
    let mut body = parts::panel_body().child(fields).child(super::step_permissions::rows(
      ctx,
      &props.state,
      &props.step,
      props.locked,
    ));
    if let Err(error) = orchester_workspace::pipelines::narrowing::check_step(&props.step, &editing.opened.assignables)
    {
      body = body.child(crate::ui::field::field_error(
        "step-executor-error",
        &message_text(ctx, &error),
      ));
    }
    let footer = self.actions(ctx, &props);
    let number = editing
      .shown()
      .steps()
      .iter()
      .position(|s| s.key == props.step.key)
      .map_or(0, |n| n + 1);
    parts::panel(
      ctx,
      &ctx.t_args("pipelines.step", [("number", format!("{number:02}"))]),
      props.step.name.as_str(),
      body.into(),
      Some(footer),
      Some(props.state.clone()),
    )
  }
}
impl StepInspector {
  fn actions(&self, ctx: &Ctx, props: &Props) -> Element {
    let (state, key) = (props.state.clone(), props.step.key.clone());
    if self.removing.get() {
      let keep = self.removing.clone();
      return Column::new()
        .padding(SpacingSize::Md)
        .spacing(SpacingSize::Sm)
        .child(parts::note(&ctx.t("pipelines.removeHint")))
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .child(
              ActionButton::new("step-keep", ButtonKind::Secondary, ctx.t("pipelines.keep"))
                .on_click(move || keep.set(false)),
            )
            .child(
              ActionButton::new("step-remove-confirm", ButtonKind::Danger, ctx.t("pipelines.remove")).on_click(
                move || {
                  state.update(|s| {
                    s.edit.remove_step(&key);
                    s.panel = Panel::Pipeline;
                    let prefix = format!("{key}:");
                    s.errors.retain(|id, _| !id.starts_with(&prefix));
                    s.inputs.retain(|id, _| !id.starts_with(&prefix));
                  })
                },
              ),
            ),
        )
        .into();
    }
    let duplicate =
      ActionButton::new("step-duplicate", ButtonKind::Secondary, ctx.t("pipelines.duplicate")).on_click(move || {
        state.update(|s| {
          if let Some(key) = s.edit.duplicate_step(&key) {
            s.panel = Panel::Step(key);
          }
        })
      });
    let remove = self.removing.clone();
    let remove =
      ActionButton::new("step-remove", ButtonKind::Ghost, ctx.t("pipelines.remove")).on_click(move || remove.set(true));
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .child(if props.locked { duplicate.disabled() } else { duplicate })
      .child(if props.locked { remove.disabled() } else { remove })
      .into()
  }
}
fn summary_metadata(ctx: &Ctx, editing: &super::state::Editing, description: Element) -> Column {
  let body = parts::panel_body()
    .child(parts::section(ctx, "pipelines.general"))
    .child(property(
      "pipeline-summary-name",
      &ctx.t("pipelines.name"),
      &editing.opened.name,
      PropertyValue::Plain,
    ))
    .child(property(
      "pipeline-slug",
      &ctx.t("pipelines.slug"),
      &editing.opened.slug,
      PropertyValue::Mono,
    ))
    .child(crate::ui::inspector::control(
      "pipeline-description-row",
      &ctx.t("pipelines.description"),
      description,
    ))
    .child(parts::section(ctx, "pipelines.runsTitle"))
    .child(property(
      "pipeline-starts",
      &ctx.t("pipelines.starts"),
      &ctx.t("pipelines.onStart"),
      PropertyValue::Plain,
    ))
    .child(property(
      "pipeline-step-count",
      &ctx.t("pipelines.steps"),
      &editing.shown().steps().len().to_string(),
      PropertyValue::Mono,
    ))
    .child(property(
      "pipeline-used-by",
      &ctx.t("pipelines.usedBy"),
      &ctx.t_args("pipelines.tasks", [("count", editing.opened.used_by.to_string())]),
      PropertyValue::Plain,
    ));
  body
    .child(parts::section(ctx, "pipelines.versions"))
    .child(property(
      "pipeline-published",
      &ctx.t("pipelines.publishedLabel"),
      &editing.opened.latest.as_ref().map_or_else(
        || ctx.t("pipelines.never"),
        |v| ctx.t_args("pipelines.version", [("version", v.number.get().to_string())]),
      ),
      PropertyValue::Plain,
    ))
    .child(property(
      "pipeline-draft-state",
      &ctx.t("pipelines.draftLabel"),
      &ctx.t(if editing.opened.draft_saved_at.is_some() {
        "pipelines.saved"
      } else {
        "pipelines.noDraft"
      }),
      PropertyValue::Plain,
    ))
    .child(property(
      "pipeline-gate-count",
      &ctx.t("pipelines.gates"),
      &editing
        .shown()
        .steps()
        .iter()
        .filter(|s| s.approval_gate)
        .count()
        .to_string(),
      PropertyValue::Mono,
    ))
}
