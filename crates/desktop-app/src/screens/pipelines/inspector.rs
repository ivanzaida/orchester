//! Pipeline summary and step inspector (c24Tv / SVBQi).
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::{Ref, Signal},
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

pub fn panel(ctx: &mut Ctx, state: &State, busy: bool, navigation_busy: bool) -> Element {
  let editing = state.get();
  let shown = editing.shown();
  if let Panel::Step(key) = &editing.panel
    && let Some(step) = shown.step(key)
  {
    let props = Props {
      state: state.clone(),
      step: step.clone(),
      locked: busy || editing.viewing.is_some(),
      navigation_locked: navigation_busy || editing.viewing.is_some(),
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
  name: Signal<String>,
  description: Signal<String>,
  /// Whether the fields are locked, as last shown.
  locked: Ref<bool>,
}
impl Component for Summary {
  type Props = SummaryProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<SummaryProps>().clone();
    let editing = props.state.get();
    let summary = Self {
      name: ctx.signal(
        editing
          .inputs
          .get("pipeline-name")
          .cloned()
          .unwrap_or_else(|| editing.name.to_string()),
      ),
      description: ctx.signal(editing.shown().description().to_owned()),
      locked: ctx.create_ref(props.locked),
    };
    follow_identity(
      ctx,
      &props.state,
      (&summary.name, &summary.description),
      &summary.locked,
    );
    summary
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    self.locked.set(props.locked);
    let editing = props.state.get();
    let mut name = FieldInput::new("pipeline-summary-name", ctx.t("pipelines.name"), self.name.clone());
    if let Some(error) = editing.errors.get("pipeline-name") {
      name = name.error(parts::pipeline_name_error(ctx, error));
    }
    let name = if props.locked { name.locked() } else { name }.render(ctx);
    let description = FieldInput::new(
      "pipeline-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .compact_area()
    .placeholder(ctx.t("pipelines.descriptionPlaceholder"));
    let description = if props.locked {
      description.locked()
    } else {
      description
    }
    .render(ctx);
    let mut body = summary_metadata(ctx, &editing, name, description);
    let history = props.state.clone();
    if editing.opened.latest.is_some() {
      body = body.child(
        ActionButton::new(
          "pipeline-summary-history",
          ButtonKind::Secondary,
          ctx.t("pipelines.history"),
        )
        .on_click(move || history.update(|s| s.panel = Panel::History)),
      );
    }
    if editing.opened.latest.is_none() {
      body = body.child(super::parts::text_notice(
        "pipeline-unpublished-note",
        &ctx.t("pipelines.notAvailableTitle"),
        &ctx.t("pipelines.notAvailableDetail"),
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
/// Takes what is typed into the pipeline's `name` and `description` into the draft, once per form, unless
/// the form is `locked` now. Shared by the summary and the settings page.
pub(super) fn follow_identity(
  ctx: &mut Ctx,
  state: &State,
  (name, description): (&Signal<String>, &Signal<String>),
  locked: &Ref<bool>,
) {
  let (rename, unlocked) = (state.clone(), locked.clone());
  ctx.watch(name, move |text| {
    if !unlocked.get() {
      rename.update(|s| {
        s.inputs.insert("pipeline-name".to_owned(), text.clone());
        s.field(
          "pipeline-name",
          orchester_workspace::pipelines::fields::parse_name(text),
          |s, name| s.name = name,
        );
      });
    }
  });
  let (state, locked) = (state.clone(), locked.clone());
  ctx.watch(description, move |text| {
    if !locked.get() {
      state.update(|s| s.edit.set_description(text));
    }
  });
}
pub struct StepInspector {
  name: Signal<String>,
  removing: Signal<bool>,
  /// Whether the step is locked, as last shown.
  locked: Ref<bool>,
}
impl Component for StepInspector {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let editing = props.state.get();
    let name = editing
      .inputs
      .get(&format!("{}:step-name", props.step.key))
      .filter(|_| editing.viewing.is_none())
      .cloned()
      .unwrap_or_else(|| props.step.name.as_str().to_owned());
    let inspector = Self {
      name: ctx.signal(name),
      removing: ctx.signal(false),
      locked: ctx.create_ref(props.locked),
    };
    inspector.follow_name(ctx, &props);
    inspector
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.locked.set(props.locked);
    let editing = props.state.get();
    let title = FieldInput::new("step-name", ctx.t("pipelines.stepName"), self.name.clone());
    let title = if props.locked {
      lurq::components::Text::new(props.step.name.as_str())
        .id("step-name")
        .variant(crate::theme::ExtraTypography::Strong.role())
        .into()
    } else {
      title.render(ctx)
    };
    let fields = ctx.mount_keyed::<Fields>("step-fields", props.clone());
    let mut body = parts::panel_body().child(fields).child(super::step_permissions::rows(
      ctx,
      &props.state,
      &props.step,
      props.locked,
    ));
    if let Err(error) = orchester_workspace::pipelines::narrowing::check_step(&props.step, &editing.opened.assignables)
      && super::step_executor::draft_profile(&props).is_none()
      && super::step_executor::unavailable_tool(&props).is_none()
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
    parts::panel_with_title(
      ctx,
      &ctx.t_args("pipelines.step", [("number", format!("{number:02}"))]),
      title,
      body.into(),
      Some(footer),
      Some(props.state),
    )
  }
}
impl StepInspector {
  /// Takes what is typed into the step's name into the draft, once per inspector, unless it is locked now.
  /// The step's key is part of the inspector's mount key, so it holds for the inspector's life.
  fn follow_name(&self, ctx: &mut Ctx, props: &Props) {
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), self.locked.clone());
    ctx.watch(&self.name, move |text| {
      if !locked.get() {
        state.update(|s| {
          let id = format!("{key}:step-name");
          s.inputs.insert(id.clone(), text.clone());
          s.field(
            &id,
            orchester_workspace::pipelines::fields::parse_name(text),
            |s, value| s.change(&key, |step| step.name = value),
          );
        });
      }
    });
  }
  fn actions(&self, ctx: &Ctx, props: &Props) -> Element {
    let (state, key) = (props.state.clone(), props.step.key.clone());
    if self.removing.get() {
      let keep = self.removing.clone();
      let confirm =
        ActionButton::new("step-remove-confirm", ButtonKind::Danger, ctx.t("pipelines.remove")).on_click(move || {
          state.update(|s| {
            s.edit.remove_step(&key);
            s.panel = Panel::Pipeline;
            let prefix = format!("{key}:");
            s.errors.retain(|id, _| !id.starts_with(&prefix));
            s.inputs.retain(|id, _| !id.starts_with(&prefix));
          })
        });
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
            .child(if props.locked { confirm.disabled() } else { confirm }),
        )
        .into();
    }
    let duplicate = ActionButton::new("step-duplicate", ButtonKind::Secondary, ctx.t("pipelines.duplicate"))
      .icon(crate::ui::icon::Icon::Copy)
      .on_click(move || {
        state.update(|s| {
          if let Some(key) = s.edit.duplicate_step(&key) {
            s.panel = Panel::Step(key);
          }
        })
      });
    let remove = self.removing.clone();
    let remove = ActionButton::new("step-remove", ButtonKind::DangerGhost, ctx.t("pipelines.remove"))
      .icon(crate::ui::icon::Icon::Trash2)
      .on_click(move || remove.set(true));
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .child(if props.locked { duplicate.disabled() } else { duplicate })
      .child(lurq::components::Spacer::new().flex(1.0))
      .child(if props.locked { remove.disabled() } else { remove })
      .into()
  }
}
fn summary_metadata(ctx: &Ctx, editing: &super::state::Editing, name: Element, description: Element) -> Column {
  let shown = editing.shown();
  let published = super::summary::published(ctx, editing);
  let draft = super::summary::draft(ctx, editing);
  parts::panel_body()
    .child(crate::ui::inspector::section_title(
      "pipeline-summary-section",
      &ctx.t("pipelines.pipelineSection"),
      None,
    ))
    .child(crate::ui::inspector::control(
      "pipeline-summary-name-row",
      &ctx.t("pipelines.name"),
      name,
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
      &ctx.t("pipelines.onTaskStart"),
      PropertyValue::Plain,
    ))
    .child(property(
      "pipeline-published",
      &ctx.t("pipelines.publishedLabel"),
      &published,
      PropertyValue::Plain,
    ))
    .child(property(
      "pipeline-draft-state",
      &ctx.t("pipelines.draftLabel"),
      &draft,
      PropertyValue::Muted,
    ))
    .child(property(
      "pipeline-used-by",
      &ctx.t("pipelines.usedBy"),
      &ctx.t_args("pipelines.tasks", [("count", editing.opened.used_by.to_string())]),
      PropertyValue::Plain,
    ))
    .child(property(
      "pipeline-step-count",
      &ctx.t("pipelines.steps"),
      &super::summary::steps(ctx, &shown),
      PropertyValue::Plain,
    ))
    .with_children(
      editing
        .opened
        .latest
        .is_some()
        .then(|| parts::section(ctx, "pipelines.versions")),
    )
    .with_children(editing.opened.latest.is_some().then(|| {
      crate::ui::inspector::note(
        "pipeline-versions-note",
        crate::ui::icon::Icon::Info,
        &ctx.t("pipelines.versionsExplanation"),
      )
    }))
}
