//! New pipeline (03.05 `c3jD7`, dialog `M793Y3`): a name, the slug it gives, and whether to
//! start from the starter or with no steps. Create draft creates draft v1 once and opens it;
//! a name whose slug another pipeline has is rejected with that pipeline named.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  name::{Name, NameError},
  resource::Slug,
};

use super::{PipelineId, PipelinesNav, starter_text};
use crate::{
  background,
  messages::{self, Message},
  pipelines::library::{CreateError, NewPipeline, StartFrom, create_in},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, text_input},
    icon::Icon,
    message_text,
    notice::Notice,
  },
  workspace::OpenProject,
};

/// Dialog width, name field and action bar heights, radio size (`M793Y3`, `Q85dSj`, `HxwbS`,
/// `zRQBy`, `TxvHf`).
const DIALOG_WIDTH: f32 = 640.0;
const NAME_HEIGHT: f32 = 36.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const RADIO_SIZE: f32 = 16.0;
const RADIO_DOT_SIZE: f32 = 8.0;

#[derive(Clone)]
pub struct NewPipelineProps {
  pub project: OpenProject,
  pub start: StartFrom,
  /// Slugs and names of the project's pipelines, to name the one a slug belongs to.
  pub taken: Arc<Vec<(Arc<str>, Arc<str>)>>,
  pub open: Signal<bool>,
}

impl PartialEq for NewPipelineProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.start == other.start && self.taken == other.taken
  }
}

impl lurq::app::component::DevtoolsInspectable for NewPipelineProps {}

type CreateArgs = (PathBuf, NewPipeline);
type CreateAction = FutureAction<CreateArgs, PipelineId, Message>;

pub struct NewPipelineDialog {
  name: Signal<String>,
  start: Signal<Option<StartFrom>>,
}

impl Component for NewPipelineDialog {
  type Props = NewPipelineProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<PipelinesNav>()
      .expect("the pipelines screen provides PipelinesNav");
    let create = create_action(ctx);
    ctx.watch(&create.state(), move |state| {
      if let Some(PipelineId(id)) = state.data {
        nav.open(id);
      }
    });
    Self {
      name: ctx.signal(String::new()),
      start: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<NewPipelineProps>().clone();
    let create = create_action(ctx);
    let start = self.start.get().unwrap_or(props.start);
    let name = self.name.get();
    let checked = check_name(&name, &props.taken);
    let state = create.state().get();
    let error = state.error.clone().filter(|_| !state.is_pending());
    let submit = checked.as_ref().ok().map(|name| {
      let (create, repo_root, text, name) = (
        create.clone(),
        props.project.repo_root.clone(),
        starter_text(ctx),
        name.clone(),
      );
      move || {
        let new = NewPipeline {
          name: name.clone(),
          start,
          starter: text.clone(),
        };
        create.run((repo_root.clone(), new));
      }
    });
    let project = props.project.project.name().as_str();
    Column::new()
      .id("new-pipeline-dialog")
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(header(ctx, project))
      .child(self.body(ctx, &name, &checked, start, project))
      .child(actions(ctx, &props.open, submit, state.is_pending(), error))
  }
}

impl NewPipelineDialog {
  fn body(&self, ctx: &Ctx, name: &str, checked: &Result<Name, Message>, start: StartFrom, project: &str) -> Element {
    let input = text_input(
      ctx,
      "new-pipeline-name",
      self.name.clone(),
      checked.is_err() && !name.is_empty(),
      TypographyStyle::FieldLabel,
    )
    .height(Dimension::Px(NAME_HEIGHT))
    .placeholder(&ctx.t("pipelines.create.namePlaceholder"));
    let mut name_field = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Field.role())
      .child(Text::new(&ctx.t("pipelines.create.name")).variant(TypographyStyle::Caption))
      .child(input);
    name_field = match (Slug::from_name(name), checked) {
      (_, Err(error)) if !name.is_empty() => name_field.child(field_error(&message_text(ctx, error))),
      (Some(slug), _) => name_field.child(slug_line(ctx, slug.as_str(), project)),
      _ => name_field,
    };
    let options = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Field.role())
      .child(Text::new(&ctx.t("pipelines.create.startFrom")).variant(TypographyStyle::Caption))
      .child(self.option(ctx, StartFrom::Starter, start))
      .child(self.option(ctx, StartFrom::Empty, start));
    let mut body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Step.role())
      .padding_horizontal(ExtraSpacing::Rail.role())
      .padding_bottom(SpacingSize::Lg)
      .spacing(ExtraSpacing::Step.role())
      .child(name_field)
      .child(options);
    if start == StartFrom::Starter {
      body = body.child(Notice::new(
        Icon::Info,
        ctx.t_args("pipelines.create.usesProjectResources", [("project", project)]),
        ctx.t("pipelines.create.usesProjectResourcesDetail"),
      ));
    }
    body.into()
  }

  /// A start option (`spYQ3`, `t0B5NG`).
  fn option(&self, ctx: &Ctx, option: StartFrom, chosen: StartFrom) -> Element {
    let selected = option == chosen;
    let (id, title, detail) = match option {
      StartFrom::Starter => (
        "new-pipeline-starter",
        "pipelines.create.starterTitle",
        "pipelines.create.starterDetail",
      ),
      StartFrom::Empty => (
        "new-pipeline-empty",
        "pipelines.create.emptyTitle",
        "pipelines.create.emptyDetail",
      ),
    };
    let mut text = Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Tight.role())
      .child(Text::new(&ctx.t(title)).variant(TypographyStyle::Label))
      .child(Text::new(&ctx.t(detail)).variant(ExtraTypography::Detail.role()));
    if option == StartFrom::Starter {
      text = text.child(
        Text::new(&ctx.t("pipelines.create.starterSteps"))
          .variant(ExtraTypography::Detail.role())
          .color(PaletteColor::TextPrimary),
      );
    }
    let (background, border, stroke) = if selected {
      (PaletteColor::SurfaceRaised, PaletteColor::TextPrimary, BorderSize::Md)
    } else {
      (PaletteColor::SurfacePanel, PaletteColor::Border, BorderSize::Sm)
    };
    let start = self.start.clone();
    Row::new()
      .id(id)
      .describe("selected", if selected { "true" } else { "false" })
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .background(background)
      .border(Border::inside(stroke, border))
      .rounded(RadiusSize::Md)
      .cursor(CursorIcon::Pointer)
      .on_click(move |_| start.set(Some(option)))
      .child(radio(selected))
      .child(text)
      .into()
  }
}

/// Radio mark (`zRQBy`, `FY1Wy`).
fn radio(selected: bool) -> Element {
  let color = if selected {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::Border
  };
  let mut mark = Row::new()
    .size(Dimension::Px(RADIO_SIZE), Dimension::Px(RADIO_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .border(Border::inside(BorderSize::Md, color))
    .rounded(ExtraRadius::Notice.role());
  if selected {
    mark = mark.child(
      Row::new()
        .size(Dimension::Px(RADIO_DOT_SIZE), Dimension::Px(RADIO_DOT_SIZE))
        .background(PaletteColor::TextPrimary)
        .rounded(ExtraRadius::Notice.role()),
    );
  }
  mark.into()
}

fn slug_line(ctx: &Ctx, slug: &str, project: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipelines.create.slug"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .child(
      Text::new(slug)
        .variant(ExtraTypography::MonoValue.role())
        .id("new-pipeline-slug"),
    )
    .child(
      Text::new(&ctx.t_args("pipelines.create.slugExplanation", [("project", project)]))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// The name as a pipeline name, or why it cannot be one: blank, too long, no slug, or a slug
/// another pipeline already has.
fn check_name(name: &str, taken: &[(Arc<str>, Arc<str>)]) -> Result<Name, Message> {
  let valid = Name::new(name).map_err(|error| match error {
    NameError::Blank => Message::new("errors.nameRequired"),
    NameError::TooLong { .. } => messages::name_error(&error),
  })?;
  let slug = Slug::from_name(valid.as_str()).ok_or_else(|| Message::new("pipelines.create.noSlug"))?;
  match taken.iter().find(|(existing, _)| **existing == *slug.as_str()) {
    Some((_, existing)) => Err(slug_taken(slug.as_str(), existing)),
    None => Ok(valid),
  }
}

fn slug_taken(slug: &str, existing: &str) -> Message {
  Message::new("pipelines.create.slugTaken")
    .with("slug", slug.to_owned())
    .with("pipeline", existing.to_owned())
}

pub fn create_error(error: &CreateError) -> Message {
  match error {
    CreateError::Store(error) => messages::store_error(error),
    CreateError::NoSlug => Message::new("pipelines.create.noSlug"),
    CreateError::SlugTaken { slug, existing } => slug_taken(slug, existing),
  }
}

fn create_action(ctx: &mut Ctx) -> CreateAction {
  ctx.future_action(|(repo_root, new): CreateArgs| async move {
    let created = background::run(move || create_in(&repo_root, new)).await?;
    created.map(PipelineId).map_err(|error| {
      tracing::error!(%error, "cannot create the pipeline");
      create_error(&error)
    })
  })
}

/// Title and explanation (`eDhZP`).
fn header(ctx: &Ctx, project: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("pipelines.create.title")).variant(ExtraTypography::DialogTitle.role()))
    .child(
      Text::new(&ctx.t_args("pipelines.create.explanation", [("project", project)]))
        .variant(ExtraTypography::Explanation.role()),
    )
    .into()
}

/// Cancel and Create draft (`HxwbS`), with why the create failed.
fn actions(
  ctx: &Ctx,
  open: &Signal<bool>,
  submit: Option<impl Fn() + Send + Sync + 'static>,
  pending: bool,
  error: Option<Message>,
) -> Element {
  let close = open.clone();
  let cancel = ActionButton::new(ctx.t("common.cancel"))
    .icon(Icon::X)
    .id("new-pipeline-cancel")
    .on_click(move || close.set(false))
    .render(ctx);
  let create = ActionButton::new(ctx.t("pipelines.create.create"))
    .primary()
    .icon(Icon::Plus)
    .id("new-pipeline-create");
  let ready = submit.is_some() && !pending;
  let create = create
    .on_click(move || {
      if let Some(submit) = &submit {
        submit();
      }
    })
    .enabled(ready);
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
