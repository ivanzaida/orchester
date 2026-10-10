//! New pipeline is a page, lAQis; slug collisions Hgequ stay in place.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{PromptText, Slug, StarterStep, StarterStepText},
};
use orchester_workspace::{
  background,
  i18n::NAMESPACE,
  messages,
  pipelines::{
    create::{self, Start},
    library::{StarterText, VersionState, library_in},
  },
};

use super::parts;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField, field},
    icon::Icon,
    inspector::{PropertyValue, property},
    message::message_text,
    option_card::OptionCard,
  },
};
const START_WIDTH: f32 = 560.0;
const NAME_WIDTH: f32 = 280.0;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Created(ResourceId);
impl DevtoolsInspectable for Created {}
pub struct NewPipeline {
  name: Signal<String>,
  description: Signal<String>,
  start: Signal<u8>,
  copy: Signal<String>,
}
impl Component for NewPipeline {
  type Props = ShownProject;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      description: ctx.signal(String::new()),
      start: ctx.signal(0),
      copy: ctx.signal(String::new()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<ShownProject>().clone();
    let save = create_action(ctx, &project);
    let nav = Navigation::of(ctx);
    let id = project.open.project.id();
    ctx.watch(&save.state(), move |state| {
      if !state.is_pending()
        && let Some(Created(created)) = state.data
      {
        nav.replace(Route::Project(id, ProjectPage::Pipeline(created)));
      }
    });
    let listed = ctx
      .future(project.open.repo_root, |root| async move {
        background::run(move || library_in(&root).map_err(|e| messages::store_error(&e))).await?
      })
      .state()
      .get();
    let copies: Vec<_> = listed
      .data
      .unwrap_or_default()
      .into_iter()
      .filter(|p| !matches!(p.state, VersionState::DraftOnly))
      .collect();
    let chosen = match self.start.get() {
      0 => Some(Start::Starter),
      1 => self.copy.get().parse().ok().map(Start::Copy),
      _ => Some(Start::Empty),
    };
    let name = self.name.get();
    let description = self.description.get();
    let button = ActionButton::new("pipeline-create", ButtonKind::Primary, ctx.t("pipelines.create"));
    let button = if save.state().get().is_pending() {
      button.loading()
    } else if let Some(start) = chosen.filter(|_| !name.trim().is_empty()) {
      let save = save.clone();
      button.on_click(move || {
        save.run((name.clone(), description.clone(), start));
      })
    } else {
      button.disabled()
    };
    let head = self.header(ctx, id, button);
    let choices = self.choices(ctx, &copies);
    let content = Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Roomy.role())
      .spacing(SpacingSize::Xl)
      .align_items(Alignment::Center)
      .child(parts::caption(&ctx.t("pipelines.onStart")))
      .child(choices);
    let panel = self.inspector(ctx);
    let mut page = parts::page().child(head).child(
      Row::new()
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .width(Dimension::Pct(100.0))
        .child(content)
        .child(panel),
    );
    if let Some(error) = save.state().get().error.or(listed.error) {
      page = page.child(crate::ui::field::field_error(
        "pipeline-create-error",
        &message_text(ctx, &error),
      ));
    }
    page
  }
}
impl NewPipeline {
  fn header(&self, ctx: &mut Ctx, id: orchester_core::id::ProjectId, button: ActionButton) -> Row {
    let nav = Navigation::of(ctx);
    parts::header()
      .child(
        ActionButton::new("pipeline-parent", ButtonKind::Ghost, ctx.t("pipelines.title"))
          .on_click(move || nav.go(Route::Project(id, ProjectPage::Pipelines))),
      )
      .child(
        FieldInput::new("pipeline-name", ctx.t("pipelines.name"), self.name.clone())
          .placeholder(ctx.t("pipelines.namePlaceholder"))
          .width(Dimension::Px(NAME_WIDTH))
          .render(ctx),
      )
      .child(parts::caption(&ctx.t("pipelines.notCreated")))
      .child(Spacer::new().flex(1.0))
      .child(
        ActionButton::new("pipeline-cancel", ButtonKind::Ghost, ctx.t("pipelines.cancel")).on_click({
          let nav = Navigation::of(ctx);
          move || nav.go(Route::Project(id, ProjectPage::Pipelines))
        }),
      )
      .child(button)
  }
  fn choices(&self, ctx: &Ctx, copies: &[orchester_workspace::pipelines::library::LibraryEntry]) -> Column {
    let mut choices = Column::new()
      .width(Dimension::Px(START_WIDTH))
      .spacing(SpacingSize::Md)
      .child(Text::new(&ctx.t("pipelines.startFrom")).variant(TypographyStyle::Title))
      .child(parts::note(&ctx.t("pipelines.startHint")));
    for (value, title, detail, glyph) in [
      (0, "pipelines.starter", "pipelines.starterHint", Icon::Workflow),
      (1, "pipelines.copy", "pipelines.copyHint", Icon::Copy),
      (2, "pipelines.empty", "pipelines.emptyHint", Icon::Plus),
    ] {
      let start = self.start.clone();
      let card = OptionCard::new(
        format!("pipeline-start-{value}"),
        (glyph, ctx.t(title)),
        ctx.t(detail),
        self.start.get() == value,
        move || start.set(value),
      );
      choices = choices.child(card);
      if value == 1 && self.start.get() == 1 {
        choices = choices.child(
          SelectField::new(
            "pipeline-copy",
            ctx.t("pipelines.copy"),
            self.copy.clone(),
            copies
              .iter()
              .map(|p| SelectOption::new(p.id.to_string(), p.name.clone()))
              .collect(),
          )
          .render(ctx),
        );
      }
    }
    choices
  }
  fn inspector(&self, ctx: &mut Ctx) -> Element {
    let slug = Slug::from_name(&self.name.get())
      .map(|s| s.as_str().to_owned())
      .unwrap_or_default();
    let description = FieldInput::new(
      "pipeline-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .text_area()
    .placeholder(ctx.t("pipelines.descriptionPlaceholder"))
    .render(ctx);
    let body = parts::panel_body()
      .child(property(
        "pipeline-slug",
        &ctx.t("pipelines.slug"),
        &slug,
        PropertyValue::Mono,
      ))
      .child(parts::note(&ctx.t("pipelines.slugHint")))
      .child(field(
        "pipeline-description",
        Some(&ctx.t("pipelines.description")),
        description,
        None,
      ))
      .child(parts::section(ctx, "pipelines.nothingSaved"))
      .child(parts::note(&ctx.t("pipelines.createHint")));
    parts::panel(
      ctx,
      &ctx.t("pipelines.pipeline"),
      &ctx.t("pipelines.new"),
      body.into(),
      None,
    )
  }
}
fn starter(ctx: &Ctx) -> StarterText {
  StarterText::new(|step| {
    let area = match step {
      StarterStep::Research => "research",
      StarterStep::Plan => "plan",
      StarterStep::Implement => "implement",
      StarterStep::RunChecks => "runChecks",
      StarterStep::Review => "review",
      StarterStep::CreatePullRequest => "createPullRequest",
    };
    let word = |part| ctx.t_ns(NAMESPACE, &format!("pipelines.starter.{area}.{part}"));
    (
      StarterStepText {
        name: Name::new(&word("name")).expect("translated starter name"),
        instructions: PromptText::new(&word("instructions")).expect("translated starter prompt"),
      },
      word("uses").to_string(),
    )
  })
}
fn create_action(
  ctx: &mut Ctx,
  project: &ShownProject,
) -> lurq::app::ctx::FutureAction<(String, String, Start), Created, messages::Message> {
  let root = project.open.repo_root.clone();
  let project_id = project.open.project.id();
  let changes = crate::services::Services::of(ctx).changes;
  let starter = starter(ctx);
  ctx.future_action(move |(name, description, start): (String, String, Start)| {
    let (root, starter, changes) = (root.clone(), starter.clone(), changes.clone());
    async move {
      background::run(move || {
        let id = create::create_in(&root, &name, &description, start, &starter)?;
        changes.record(project_id, orchester_workspace::tasks::outside::Outside::Pipelines);
        Ok(Created(id))
      })
      .await?
    }
  })
}
