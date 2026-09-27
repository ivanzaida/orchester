//! The chosen variable (`y9PWLw` in `EfTiT`, `BoSgn` in `m2WjL`): its name and type, what it
//! holds, its expression, its value in the previewed context or why it has none, where it comes
//! from, and Insert. The catalog's copy per variable and group.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  clipboard::copy_to_clipboard,
  components::{Button, Column, Row, Spacer, Text},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::resource::{InjectionEvent, Variable, VariableGroup, VariableType};

use super::{
  super::{copy, editor::InstructionEditor},
  with_variable,
};
use crate::{
  instructions::preview::Resolved,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    command_row::Copied,
    field::{ValueTone, value_box},
    icon::{Icon, IconSize, icon},
    pressable::{Ring, keyboard_stop},
    property::property,
    status_label::status_label,
  },
};

/// Insert button height (`Vlvnl`).
const INSERT_HEIGHT: f32 = 40.0;

impl InstructionEditor {
  pub(in super::super) fn variable_details(
    &self,
    ctx: &Ctx,
    variable: Variable,
    resolved: Option<&Resolved<'_>>,
  ) -> Element {
    let value = resolved.and_then(|resolved| resolved.value(variable));
    let type_label = ctx.t_args(
      "variables.typeInGroup",
      [
        ("type", &*type_label(ctx, variable.value_type())),
        ("group", &*group_label(ctx, variable.group())),
      ],
    );
    let identity = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .child(Text::new(variable.name()).variant(ExtraTypography::VariableName.role()))
      .child(Row::new().child(status_label(Icon::Type, &type_label)));
    let expression = Row::new()
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Group.role())
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(
        Text::new(&format!("{{{{{}}}}}", variable.name()))
          .variant(ExtraTypography::MonoField.role())
          .selectable(true)
          .flex(1.0),
      )
      .child(self.copy_expression(variable));
    let properties = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Step.role())
      .spacing(ExtraSpacing::Stack.role())
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(property(&ctx.t("variables.available"), &availability(ctx, variable)))
      .child(property(
        &ctx.t("variables.source"),
        &group_label(ctx, variable.group()),
      ));
    let mut column = Column::new()
      .id("variable-details")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Rail.role())
      .spacing(ExtraSpacing::Form.role())
      .child(identity)
      .child(Text::new(&description(ctx, variable)).variant(ExtraTypography::Summary.role()))
      .child(expression)
      .child(value_preview(
        ctx,
        variable,
        value.as_deref(),
        resolved,
        self.preview_event.get().0,
      ))
      .child(properties);
    if value.is_none() {
      column = column.child(missing_policy(ctx));
    }
    column
      .child(Spacer::new().flex(1.0))
      .child(self.insert_button(ctx, variable))
      .into()
  }

  /// Copies the expression (`Rm6AE`); a check shows the clipboard took it.
  fn copy_expression(&self, variable: Variable) -> Element {
    let expression: Arc<str> = format!("{{{{{}}}}}", variable.name()).into();
    let copied = self.copied.clone();
    let done = matches!(&copied.get().0, Some((text, true)) if *text == expression);
    keyboard_stop(Button::empty().button(), Ring::Outer)
      .id("variable-copy")
      .cursor(CursorIcon::Pointer)
      .rounded(RadiusSize::Sm)
      .on_click(move |_| {
        let success = copy_to_clipboard(&*expression);
        if !success {
          tracing::warn!("the clipboard did not take the variable expression");
        }
        copied.set(Copied(Some((expression.clone(), success))));
      })
      .child(icon(
        if done { Icon::Check } else { Icon::Copy },
        IconSize::Dismiss,
        PaletteColor::TextSecondary,
      ))
      .into()
  }

  /// Insert `{{name}}`: adds the expression to the prompt as one edit and closes the catalog.
  fn insert_button(&self, ctx: &Ctx, variable: Variable) -> Element {
    let expression = format!("{{{{{}}}}}", variable.name());
    let (text, open) = (self.text.clone(), self.variables_open.clone());
    ActionButton::new(ctx.t_args("variables.insert", [("expression", expression.as_str())]))
      .primary()
      .icon(Icon::Plus)
      .id("variable-insert")
      .in_form()
      .fill_width()
      .height(Dimension::Px(INSERT_HEIGHT))
      .on_click(move || {
        text.update(|text| *text = with_variable(text, variable));
        open.set(false);
      })
      .render(ctx)
  }
}

/// "VALUE AT AGENT START · RUN 071" and the value, or why there is none (`cmjiA`).
fn value_preview(
  ctx: &Ctx,
  variable: Variable,
  value: Option<&str>,
  resolved: Option<&Resolved<'_>>,
  event: InjectionEvent,
) -> Element {
  let context = resolved.and_then(|resolved| resolved.run).map_or_else(
    || ctx.t("variables.newRun"),
    |run| super::super::preview::run_label(ctx, run),
  );
  let label = ctx.t_args(
    "variables.valueAt",
    [
      ("event", copy::event_label(ctx, event).to_uppercase().as_str()),
      ("context", context.to_uppercase().as_str()),
    ],
  );
  let value: Arc<str> = match value {
    Some(value) if value.trim().is_empty() => ctx.t("variables.emptyValue"),
    Some(value) => value.into(),
    None => ctx.t_args(
      "variables.unavailable",
      [("reason", &*unavailable_reason(ctx, variable, resolved))],
    ),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&label).variant(ExtraTypography::Overline.role()))
    .child(Text::new(&value).variant(TypographyStyle::Body).selectable(true))
    .into()
}

fn unavailable_reason(ctx: &Ctx, variable: Variable, resolved: Option<&Resolved<'_>>) -> Arc<str> {
  let key = match (resolved, variable.group()) {
    (None, _) => "variables.reasons.noPreview",
    (Some(resolved), VariableGroup::Run) if resolved.run.is_none() => "variables.reasons.noRun",
    (Some(_), VariableGroup::Run) => "variables.reasons.noAttempt",
    (Some(_), _) => "variables.reasons.notRecorded",
  };
  ctx.t(key)
}

/// "When this value is unavailable" (`b95FoZ`): injection is blocked; fallbacks do not exist
/// yet, so the policy is shown as the only one.
fn missing_policy(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("variables.whenUnavailable")).variant(TypographyStyle::Caption))
    .child(value_box(
      ctx,
      "variable-missing-policy",
      &ctx.t("variables.requireValue"),
      ValueTone::Unavailable,
      None,
    ))
    .child(Text::new(&ctx.t("variables.noFallback")).variant(ExtraTypography::Hint.role()))
    .into()
}

pub(super) fn group_label(ctx: &Ctx, group: VariableGroup) -> Arc<str> {
  ctx.t(match group {
    VariableGroup::Task => "variables.groups.task",
    VariableGroup::Project => "variables.groups.project",
    VariableGroup::Agent => "variables.groups.agent",
    VariableGroup::Pipeline => "variables.groups.pipeline",
    VariableGroup::Run => "variables.groups.run",
    VariableGroup::Step => "variables.groups.step",
  })
}

/// The group's category item id and icon (`VYDHZ` … `WRmQ5`).
pub(super) const fn group_item(group: VariableGroup) -> (&'static str, Icon) {
  match group {
    VariableGroup::Task => ("variables-task", Icon::CircleCheck),
    VariableGroup::Project => ("variables-project", Icon::FolderGit),
    VariableGroup::Agent => ("variables-agent", Icon::Bot),
    VariableGroup::Pipeline => ("variables-pipeline", Icon::Workflow),
    VariableGroup::Run => ("variables-run", Icon::Play),
    VariableGroup::Step => ("variables-step", Icon::Square),
  }
}

fn type_label(ctx: &Ctx, value_type: VariableType) -> Arc<str> {
  ctx.t(match value_type {
    VariableType::String => "variables.types.string",
    VariableType::StringList => "variables.types.stringList",
    VariableType::AttachmentList => "variables.types.attachmentList",
    VariableType::Timestamp => "variables.types.timestamp",
    VariableType::Repository => "variables.types.repository",
    VariableType::Integer => "variables.types.integer",
    VariableType::Tokens => "variables.types.tokens",
  })
}

/// The copy key of `variable` under `variables.catalog`, e.g. `taskTitle`.
const fn key(variable: Variable) -> &'static str {
  match variable {
    Variable::TaskId => "taskId",
    Variable::TaskTitle => "taskTitle",
    Variable::TaskDescription => "taskDescription",
    Variable::TaskAcceptanceCriteria => "taskAcceptanceCriteria",
    Variable::TaskAttachments => "taskAttachments",
    Variable::TaskCreatedAt => "taskCreatedAt",
    Variable::ProjectId => "projectId",
    Variable::ProjectName => "projectName",
    Variable::ProjectRepository => "projectRepository",
    Variable::ProjectDefaultBranch => "projectDefaultBranch",
    Variable::AgentId => "agentId",
    Variable::AgentName => "agentName",
    Variable::AgentRole => "agentRole",
    Variable::AgentModel => "agentModel",
    Variable::PipelineId => "pipelineId",
    Variable::PipelineName => "pipelineName",
    Variable::PipelineVersion => "pipelineVersion",
    Variable::RunId => "runId",
    Variable::RunAttempt => "runAttempt",
    Variable::RunBudget => "runBudget",
    Variable::RunBranch => "runBranch",
    Variable::RunRevision => "runRevision",
    Variable::StepId => "stepId",
    Variable::StepName => "stepName",
  }
}

pub(super) fn description(ctx: &Ctx, variable: Variable) -> Arc<str> {
  ctx.t(&format!("variables.catalog.{}.description", key(variable)))
}

fn availability(ctx: &Ctx, variable: Variable) -> Arc<str> {
  ctx.t(&format!("variables.catalog.{}.availability", key(variable)))
}
