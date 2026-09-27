//! New instruction: a name and, outside Project rules, the role, pipeline or step the rule applies
//! in. Create draft creates the rule as an empty draft for every agent at start and opens it. Not
//! drawn in the design; built like New pipeline (03.05 `c3jD7`).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  name::{Name, NameError},
  resource::InstructionScope,
};

use super::super::ProjectNav;
use crate::{
  activity::{Activity, OperationSpec, operations},
  background,
  instructions::{
    data::{NewInstruction, ProjectInstructions, create_in},
    form::Picked,
  },
  messages::{self, Message},
  navigation::route::{InstructionsTab, Page},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, select_style, text_input},
    form::{Submit, form},
    icon::Icon,
    message_text,
  },
  workspace::OpenProject,
};

/// Dialog width, field and action bar heights (`M793Y3`, `Q85dSj`, `HxwbS` in 03.05).
const DIALOG_WIDTH: f32 = 640.0;
const FIELD_HEIGHT: f32 = 36.0;
const ACTIONS_HEIGHT: f32 = 76.0;

#[derive(Clone)]
pub struct NewInstructionProps {
  pub project: OpenProject,
  pub tab: InstructionsTab,
  pub data: ProjectInstructions,
  pub open: Signal<bool>,
  /// Bumped once the rule exists, to read the rules again.
  pub revision: Signal<u64>,
}

impl PartialEq for NewInstructionProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.tab == other.tab && self.data == other.data
  }
}

impl lurq::app::component::DevtoolsInspectable for NewInstructionProps {}

/// The created rule and the tab it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Created(ResourceId, InstructionsTab);

impl lurq::app::component::DevtoolsInspectable for Created {}

type CreateArgs = (PathBuf, NewInstruction, InstructionsTab, OperationSpec);
type CreateAction = FutureAction<CreateArgs, Created, Message>;

pub struct NewInstructionDialog {
  name: Signal<String>,
  name_input: ElementRef,
  /// The chosen role, pipeline or step; `None` keeps the first.
  scope: Signal<Picked<Option<InstructionScope>>>,
}

impl Component for NewInstructionDialog {
  type Props = NewInstructionProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let props = ctx.props::<NewInstructionProps>().clone();
    let create = create_action(ctx);
    ctx.watch(&create.state(), move |state| {
      if let (false, Some(Created(id, tab))) = (state.is_pending(), state.data) {
        props.open.set(false);
        props.revision.update(|revision| *revision += 1);
        nav.show(Page::Instructions(tab, Some(id)));
      }
    });
    let name_input = ElementRef::new();
    ctx.focus(&name_input);
    Self {
      name: ctx.signal(String::new()),
      name_input,
      scope: ctx.signal(Picked(None)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<NewInstructionProps>().clone();
    let create = create_action(ctx);
    let options = scope_options(ctx, &props);
    let scope = chosen_scope(props.tab, self.scope.get().0, &options);
    let name = self.name.get();
    let checked = Name::new(&name).map_err(|error| match error {
      NameError::Blank => Message::new("errors.nameRequired"),
      NameError::TooLong { .. } => messages::name_error(&error),
    });
    let state = create.state().get();
    let error = state.error.clone().filter(|_| !state.is_pending());
    let submit: Submit = match (&checked, scope) {
      (Ok(name), Some(scope)) if !state.is_pending() => {
        let (create, repo_root, tab) = (create, props.project.repo_root.clone(), props.tab);
        let spec = operations::instruction_create(&props.project, name.as_str());
        let new = NewInstruction {
          name: name.clone(),
          scope,
        };
        Some(Arc::new(move || {
          create.run((repo_root.clone(), new.clone(), tab, spec.clone()))
        }))
      }
      _ => None,
    };
    let dialog = Column::new()
      .id("new-instruction-dialog")
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(header(ctx, &props))
      .child(self.body(ctx, &name, &checked, options))
      .child(actions(ctx, &props.open, submit.clone(), error));
    form(submit, dialog)
  }
}

impl NewInstructionDialog {
  fn body(
    &self,
    ctx: &Ctx,
    name: &str,
    checked: &Result<Name, Message>,
    options: Vec<(InstructionScope, Arc<str>)>,
  ) -> Element {
    let input = text_input(
      ctx,
      "new-instruction-name",
      self.name.clone(),
      checked.is_err() && !name.is_empty(),
      TypographyStyle::FieldLabel,
    )
    .height(Dimension::Px(FIELD_HEIGHT))
    .placeholder(&ctx.t("instructions.create.namePlaceholder"))
    .ref_element(self.name_input.clone());
    let mut name_field = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Field.role())
      .child(Text::new(&ctx.t("instructions.create.name")).variant(TypographyStyle::Caption))
      .child(input);
    if let (Err(error), false) = (checked, name.is_empty()) {
      name_field = name_field.child(field_error(&message_text(ctx, error)));
    }
    let mut body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Step.role())
      .padding_horizontal(ExtraSpacing::Rail.role())
      .padding_bottom(SpacingSize::Lg)
      .spacing(ExtraSpacing::Step.role())
      .child(name_field);
    if !options.is_empty() {
      let options: Vec<(Picked<Option<InstructionScope>>, Arc<str>)> = options
        .into_iter()
        .map(|(scope, label)| (Picked(Some(scope)), label))
        .collect();
      let select = Select::new(self.scope.clone())
        .options(options)
        .placeholder(ctx.t("instructions.settings.choose"))
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(FIELD_HEIGHT))
        .style(select_style(ctx, FIELD_HEIGHT));
      body = body.child(
        Column::new()
          .id("new-instruction-scope")
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Field.role())
          .child(Text::new(&ctx.t("instructions.create.appliesIn")).variant(TypographyStyle::Caption))
          .child(select),
      );
    }
    body.into()
  }
}

/// Where a rule of the dialog's tab can apply: the roles, the pipelines or their steps. Project
/// rules apply in the whole project and offer no choice (see [`chosen_scope`]).
fn scope_options(ctx: &Ctx, props: &NewInstructionProps) -> Vec<(InstructionScope, Arc<str>)> {
  let data = &props.data;
  match props.tab {
    InstructionsTab::ProjectRules => Vec::new(),
    InstructionsTab::AgentRoles => data
      .roles()
      .into_iter()
      .map(|role| {
        let label = Arc::from(role.as_str());
        (InstructionScope::AgentRole { role }, label)
      })
      .collect(),
    InstructionsTab::PipelineOverrides => data
      .pipelines
      .iter()
      .map(|pipeline| {
        (
          InstructionScope::Pipeline { pipeline: pipeline.id },
          pipeline.name.clone(),
        )
      })
      .collect(),
    InstructionsTab::StepOverrides => data
      .pipelines
      .iter()
      .flat_map(|pipeline| {
        pipeline.steps.iter().map(|step| {
          let label = ctx.t_args(
            "instructions.pipelineStep",
            [("pipeline", &*pipeline.name), ("step", &*step.name)],
          );
          let scope = InstructionScope::Step {
            pipeline: pipeline.id,
            step: step.key.clone(),
          };
          (scope, label)
        })
      })
      .collect(),
  }
}

/// The scope the rule is created in: the project, or the chosen or else the first option.
fn chosen_scope(
  tab: InstructionsTab,
  chosen: Option<InstructionScope>,
  options: &[(InstructionScope, Arc<str>)],
) -> Option<InstructionScope> {
  if tab == InstructionsTab::ProjectRules {
    return Some(InstructionScope::Project);
  }
  chosen.or_else(|| options.first().map(|(scope, _)| scope.clone()))
}

fn header(ctx: &Ctx, props: &NewInstructionProps) -> Element {
  let project = props.project.project.name().as_str();
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("instructions.create.title")).variant(ExtraTypography::DialogTitle.role()))
    .child(
      Text::new(&ctx.t_args("instructions.create.explanation", [("project", project)]))
        .variant(ExtraTypography::Explanation.role()),
    )
    .into()
}

/// Cancel and Create draft, with why the create failed.
fn actions(ctx: &Ctx, open: &Signal<bool>, submit: Submit, error: Option<Message>) -> Element {
  let close = open.clone();
  let cancel = ActionButton::new(ctx.t("common.cancel"))
    .icon(Icon::X)
    .id("new-instruction-cancel")
    .in_form()
    .on_click(move || close.set(false))
    .render(ctx);
  let create = ActionButton::new(ctx.t("instructions.create.create"))
    .primary()
    .icon(Icon::Plus)
    .id("new-instruction-create")
    .in_form();
  let create = match submit {
    Some(submit) => create.on_click(move || submit()),
    None => create.on_click(|| {}).enabled(false),
  };
  let mut row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()));
  row = match error {
    Some(error) => row.child(Column::new().flex(1.0).child(field_error(&message_text(ctx, &error)))),
    None => row.child(Spacer::new().flex(1.0)),
  };
  row.child(cancel).child(create.render(ctx)).into()
}

fn create_action(ctx: &mut Ctx) -> CreateAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, new, tab, spec): CreateArgs| {
    let activity = activity.clone();
    async move {
      let root = repo_root.clone();
      let create = async move {
        let created = background::run(move || create_in(&repo_root, new)).await?;
        created.map(|id| Created(id, tab)).map_err(|error| {
          tracing::error!(%error, "cannot create the instruction");
          messages::store_error(&error)
        })
      };
      let target = |created: &Created| Some(operations::instruction(&root, created.0));
      activity.track_to(spec, create, target).await
    }
  })
}
