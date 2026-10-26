//! New pipeline (`04 · New pipeline` `uHPNu`): a sub-page under `← Pipelines` with *Cancel* and *Create draft v1*
//! in its header; the pipeline's name, slug and description, and where draft v1 starts from (`super::new_start`). A
//! name whose slug another pipeline has says so on the Slug row with *Open <name>* (`04b` `I49xek`); a library row's
//! *Duplicate* opens it with *Copy a pipeline* chosen.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  name::Name,
  resource::{PromptText, Slug, StarterStep, StarterStepText},
};
use orchester_workspace::{
  background,
  i18n::NAMESPACE,
  messages,
  pipelines::{
    create::{self, Start},
    library::{LibraryEntry, StarterText, VersionState, library_in},
  },
};
use parking_lot::Mutex;

use super::parts;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelineView, ProjectPage, Route},
  theme::{IconSize, PipelinesText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
    row_group::{GroupRow, HintTone, RowGroup, note},
  },
};

/// The name and description fields' width (`Input` `wqf2S`).
const FIELD_WIDTH: f32 = 260.0;

/// The pipeline made in the project.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Created(ProjectId, ResourceId);
impl DevtoolsInspectable for Created {}

/// Where draft v1 starts from: the starter, a copy of a published pipeline, or no steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Begin {
  Starter,
  Copy,
  Empty,
}
impl DevtoolsInspectable for Begin {}

pub struct NewPipeline {
  name: Signal<String>,
  description: Signal<String>,
  start: Signal<Begin>,
  copy: Signal<String>,
  /// Whether the creation's end is followed already; once per page.
  followed: Mutex<bool>,
}
impl Component for NewPipeline {
  type Props = ShownProject;
  fn create(ctx: &mut Ctx) -> Self {
    let (start, copy) = match Navigation::of(ctx).route_now() {
      Route::Project(_, ProjectPage::NewEmptyPipeline) => (Begin::Empty, String::new()),
      Route::Project(_, ProjectPage::CopyPipeline(id)) => (Begin::Copy, id.to_string()),
      _ => (Begin::Starter, String::new()),
    };
    Self {
      name: ctx.signal(String::new()),
      description: ctx.signal(String::new()),
      start: ctx.signal(start),
      copy: ctx.signal(copy),
      followed: Mutex::new(false),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<ShownProject>().clone();
    // First: lurq keeps a component's futures by the order it asks for them.
    let save = create_action(ctx, &project);
    self.follow(ctx, &save);
    let id = project.open.project.id();
    let listed = ctx
      .future(project.open.repo_root, |root| async move {
        background::run(move || library_in(&root).map_err(|e| messages::store_error(&e))).await?
      })
      .state()
      .get();
    let entries = listed.data.clone().unwrap_or_default();
    let copies: Vec<_> = entries
      .iter()
      .filter(|p| !matches!(p.state, VersionState::DraftOnly))
      .cloned()
      .collect();
    let collision = orchester_workspace::pipelines::library::matching_name(&entries, &self.name.get()).cloned();
    let head = self.header(ctx, id, &save, collision.is_some());
    let pipeline = self.pipeline_group(ctx, id, collision.as_ref());
    let start = super::new_start::group(ctx, (&self.start, &self.copy), &copies);
    let mut pane = parts::pane("pipeline-new")
      .child(pipeline)
      .child(start)
      .child(note("pipeline-nothing-saved", &ctx.t("pipelines.createHint")));
    if let Some(error) = save.state().get().error.or(listed.error) {
      pane = pane.child(crate::ui::field::field_error(
        "pipeline-create-error",
        &message_text(ctx, &error),
      ));
    }
    parts::page().child(head).child(parts::body("pipeline-new-body", pane))
  }
}
impl NewPipeline {
  /// Opens the pipeline a creation made, in place of this page; once per page.
  fn follow(&self, ctx: &mut Ctx, save: &CreateAction) {
    let mut followed = self.followed.lock();
    if *followed {
      return;
    }
    *followed = true;
    let nav = Navigation::of(ctx);
    ctx.watch(&save.state(), move |state| {
      if !state.is_pending()
        && state.error.is_none()
        && let Some(Created(project, created)) = state.data
      {
        nav.replace(Route::Project(
          project,
          ProjectPage::Pipeline(created, PipelineView::default()),
        ));
      }
    });
  }

  /// The chosen start, once it names everything it needs.
  fn chosen(&self) -> Option<Start> {
    match self.start.get() {
      Begin::Starter => Some(Start::Starter),
      Begin::Copy => self.copy.get().parse().ok().map(Start::Copy),
      Begin::Empty => Some(Start::Empty),
    }
  }

  /// `← Pipelines`, the title with why it cannot be created yet, *Cancel* and *Create draft v1* (`Header` `Bh02q`).
  fn header(&self, ctx: &mut Ctx, project: ProjectId, save: &CreateAction, collision: bool) -> Element {
    let nav = Navigation::of(ctx);
    let (name, description) = (self.name.get(), self.description.get());
    let button = ActionButton::new("pipeline-create", ButtonKind::Primary, ctx.t("pipelines.create")).icon(Icon::Plus);
    let button = if save.state().get().is_pending() {
      button.loading()
    } else if let Some(start) = self.chosen().filter(|_| !name.trim().is_empty() && !collision) {
      let save = save.clone();
      button.on_click(move || save.run((name.clone(), description.clone(), start)))
    } else {
      button.disabled()
    };
    let reason = (self.name.get().trim().is_empty() || collision).then(|| {
      Text::new(&ctx.t(if collision {
        "pipelines.freeSlugRequired"
      } else {
        "pipelines.nameRequired"
      }))
      .id("pipeline-name-required")
      .variant(lurq::app::theme::TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
    });
    let cancel = ActionButton::new("pipeline-cancel", ButtonKind::Ghost, ctx.t("pipelines.cancel"))
      .opens(nav.to(Route::Project(project, ProjectPage::Pipelines)));
    parts::header()
      .child(parts::back_link(
        "pipeline-parent",
        &ctx.t("pipelines.title"),
        Press::Open(nav.to(Route::Project(project, ProjectPage::Pipelines))),
      ))
      .child(
        parts::title_row("pipeline-new-title", &ctx.t("pipelines.new"))
          .with_children(reason)
          .child(cancel)
          .child(button),
      )
      .child(
        Row::new()
          .id("pipeline-new-status")
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .padding(lurq::node::padding::Padding::new().bottom(SpacingSize::Md))
          .child(icon(Icon::CircleDashed, IconSize::Base, PaletteColor::TextMuted))
          .child(Text::new(&ctx.t("pipelines.nothingSaved")).variant(PipelinesText::Status.role())),
      )
      .into()
  }

  /// Name, Slug and Description (`Pipeline` `wMCMV`).
  fn pipeline_group(&self, ctx: &mut Ctx, project: ProjectId, collision: Option<&LibraryEntry>) -> RowGroup {
    let mut name = FieldInput::new("pipeline-name", ctx.t("pipelines.name"), self.name.clone())
      .placeholder(ctx.t("pipelines.namePlaceholder"))
      .width(Dimension::Px(FIELD_WIDTH));
    if collision.is_some() {
      name = name.invalid();
    }
    let name = name.render(ctx);
    let description = FieldInput::new(
      "pipeline-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .placeholder(ctx.t("pipelines.descriptionPlaceholder"))
    .width(Dimension::Px(FIELD_WIDTH))
    .render(ctx);
    let slug = Slug::from_name(&self.name.get())
      .map(|s| s.as_str().to_owned())
      .unwrap_or_else(|| ctx.t("pipelines.unset").to_string());
    let slug_row = GroupRow::new("pipeline-slug", ctx.t("pipelines.slug"));
    let slug_row = match collision {
      Some(existing) => {
        let nav = Navigation::of(ctx);
        slug_row
          .toned_hint(
            ctx.t_args("pipelines.slugUsedBy", [("name", existing.name.to_string())]),
            HintTone::Danger,
          )
          .end(
            ActionButton::new(
              "pipeline-open-existing",
              ButtonKind::Ghost,
              ctx.t_args("pipelines.openPipeline", [("name", existing.name.to_string())]),
            )
            .icon(Icon::ArrowUpRight)
            .opens(nav.to(Route::Project(
              project,
              ProjectPage::Pipeline(existing.id, PipelineView::default()),
            ))),
          )
      }
      None => slug_row.hint(ctx.t("pipelines.slugHint")),
    };
    RowGroup::new("pipeline-new-pipeline", ctx.t("pipelines.pipelineSection"))
      .row(GroupRow::new("pipeline-name-row", ctx.t("pipelines.name")).end(name))
      .row(slug_row.mono_value(&slug))
      .row(
        GroupRow::new("pipeline-description-row", ctx.t("pipelines.description"))
          .hint(ctx.t("pipelines.descriptionHelp"))
          .end(description),
      )
  }
}

pub(super) fn starter(ctx: &Ctx) -> StarterText {
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
type CreateAction = lurq::app::ctx::FutureAction<(String, String, Start), Created, messages::Message>;
fn create_action(ctx: &mut Ctx, project: &ShownProject) -> CreateAction {
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
        Ok(Created(project_id, id))
      })
      .await?
    }
  })
}
