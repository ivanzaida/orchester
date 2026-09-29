//! New skill: its name. Create draft creates the skill as an empty draft for the whole project and
//! opens it in the editor, where its scope, description, body and files are written. Not drawn in
//! the design; built like New instruction.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  name::{Name, NameError},
};

use super::super::ProjectNav;
use crate::{
  activity::{Activity, OperationSpec, skill_operations},
  background,
  messages::{self, Message},
  navigation::route::Page,
  skills::data::create_in,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, text_input},
    form::{Submit, form},
    icon::Icon,
    message_text,
  },
  workspace::OpenProject,
};

/// Dialog width, field and action bar heights, as New instruction's.
const DIALOG_WIDTH: f32 = 640.0;
const FIELD_HEIGHT: f32 = 36.0;
const ACTIONS_HEIGHT: f32 = 76.0;

#[derive(Clone)]
pub struct NewSkillProps {
  pub project: OpenProject,
  pub open: Signal<bool>,
  /// Bumped once the skill exists, to read the skills again.
  pub revision: Signal<u64>,
}

impl PartialEq for NewSkillProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
  }
}

impl lurq::app::component::DevtoolsInspectable for NewSkillProps {}

/// The created skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Created(ResourceId);

impl lurq::app::component::DevtoolsInspectable for Created {}

type CreateArgs = (PathBuf, Name, OperationSpec);
type CreateAction = FutureAction<CreateArgs, Created, Message>;

pub struct NewSkillDialog {
  name: Signal<String>,
  name_input: ElementRef,
}

impl Component for NewSkillDialog {
  type Props = NewSkillProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    let props = ctx.props::<NewSkillProps>().clone();
    let create = create_action(ctx);
    ctx.watch(&create.state(), move |state| {
      if let (false, Some(Created(id))) = (state.is_pending(), state.data) {
        props.open.set(false);
        props.revision.update(|revision| *revision += 1);
        nav.show(Page::Skills(Some(id)));
      }
    });
    let name_input = ElementRef::new();
    ctx.focus(&name_input);
    Self {
      name: ctx.signal(String::new()),
      name_input,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<NewSkillProps>().clone();
    let create = create_action(ctx);
    let name = self.name.get();
    let checked = Name::new(&name).map_err(|error| match error {
      NameError::Blank => Message::new("errors.nameRequired"),
      NameError::TooLong { .. } => messages::name_error(&error),
    });
    let state = create.state().get();
    let error = state.error.clone().filter(|_| !state.is_pending());
    let submit: Submit = match &checked {
      Ok(name) if !state.is_pending() => {
        let (create, repo_root, name) = (create, props.project.repo_root.clone(), name.clone());
        let spec = skill_operations::skill_create(&props.project, name.as_str());
        Some(Arc::new(move || {
          create.run((repo_root.clone(), name.clone(), spec.clone()))
        }))
      }
      _ => None,
    };
    let input = text_input(
      ctx,
      "new-skill-name",
      self.name.clone(),
      checked.is_err() && !name.is_empty(),
      ExtraTypography::MonoField.role(),
    )
    .height(Dimension::Px(FIELD_HEIGHT))
    .placeholder(&ctx.t("skills.create.namePlaceholder"))
    .ref_element(self.name_input.clone());
    let mut field = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Field.role())
      .child(Text::new(&ctx.t("skills.create.name")).variant(TypographyStyle::Caption))
      .child(input);
    if let (Err(error), false) = (&checked, name.is_empty()) {
      field = field.child(field_error(&message_text(ctx, error)));
    }
    let project = props.project.project.name().as_str();
    let header = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(ExtraSpacing::Rail.role())
      .spacing(SpacingSize::Sm)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Text::new(&ctx.t("skills.create.title")).variant(ExtraTypography::DialogTitle.role()))
      .child(
        Text::new(&ctx.t_args("skills.create.explanation", [("project", project)]))
          .variant(ExtraTypography::Explanation.role()),
      );
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Step.role())
      .padding_horizontal(ExtraSpacing::Rail.role())
      .padding_bottom(SpacingSize::Lg)
      .child(field);
    let dialog = Column::new()
      .id("new-skill-dialog")
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(header)
      .child(body)
      .child(actions(ctx, &props.open, submit.clone(), error));
    form(submit, dialog)
  }
}

/// Cancel and Create draft, with why the create failed.
fn actions(ctx: &Ctx, open: &Signal<bool>, submit: Submit, error: Option<Message>) -> Element {
  let close = open.clone();
  let cancel = ActionButton::new(ctx.t("common.cancel"))
    .icon(Icon::X)
    .id("new-skill-cancel")
    .in_form()
    .on_click(move || close.set(false))
    .render(ctx);
  let create = ActionButton::new(ctx.t("skills.create.create"))
    .primary()
    .icon(Icon::Plus)
    .id("new-skill-create")
    .in_form();
  let create = match submit {
    Some(submit) => create.on_click(move || submit()),
    None => create.on_click(|| {}).enabled(false),
  };
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()));
  let row = match error {
    Some(error) => row.child(Column::new().flex(1.0).child(field_error(&message_text(ctx, &error)))),
    None => row.child(Spacer::new().flex(1.0)),
  };
  row.child(cancel).child(create.render(ctx)).into()
}

fn create_action(ctx: &mut Ctx) -> CreateAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, name, spec): CreateArgs| {
    let activity = activity.clone();
    async move {
      let root = repo_root.clone();
      let create = async move {
        let created = background::run(move || create_in(&repo_root, name)).await?;
        created.map(Created).map_err(|error| {
          tracing::error!(%error, "cannot create the skill");
          messages::store_error(&error)
        })
      };
      let target = |created: &Created| Some(skill_operations::skill(&root, created.0));
      activity.track_to(spec, create, target).await
    }
  })
}
