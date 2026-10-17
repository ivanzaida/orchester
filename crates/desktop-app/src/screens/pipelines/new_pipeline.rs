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
    library::{StarterText, VersionState, library_in},
  },
};
use parking_lot::Mutex;

use super::parts;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelinePanel, ProjectPage, Route},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField},
    icon::Icon,
    inspector::{PropertyValue, property},
    message::message_text,
    option_card::OptionCard,
  },
};
const START_WIDTH: f32 = 560.0;
const NAME_WIDTH: f32 = 280.0;

/// The pipeline made in the project.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Created(ProjectId, ResourceId);
impl DevtoolsInspectable for Created {}
pub struct NewPipeline {
  name: Signal<String>,
  description: Signal<String>,
  start: Signal<u8>,
  copy: Signal<String>,
  /// Whether the creation's end is followed already; once per page.
  followed: Mutex<bool>,
}
impl Component for NewPipeline {
  type Props = ShownProject;
  fn create(ctx: &mut Ctx) -> Self {
    let empty = matches!(
      Navigation::of(ctx).route_now(),
      Route::Project(_, ProjectPage::NewEmptyPipeline)
    );
    Self {
      name: ctx.signal(String::new()),
      description: ctx.signal(String::new()),
      start: ctx.signal(if empty { 2 } else { 0 }),
      copy: ctx.signal(String::new()),
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
    let copies: Vec<_> = listed
      .data
      .clone()
      .unwrap_or_default()
      .into_iter()
      .filter(|p| !matches!(p.state, VersionState::DraftOnly))
      .collect();
    let collision = listed
      .data
      .as_ref()
      .and_then(|entries| orchester_workspace::pipelines::library::matching_name(entries, &self.name.get()))
      .cloned();
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
    } else if let Some(start) = chosen.filter(|_| !name.trim().is_empty() && collision.is_none()) {
      let save = save.clone();
      button.on_click(move || {
        save.run((name.clone(), description.clone(), start));
      })
    } else {
      button.disabled()
    };
    let head = self.header(ctx, id, button, collision.is_some());
    let choices = self.choices(ctx, &copies);
    let content = Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Roomy.role())
      .background(crate::theme::pipelines::canvas_color())
      .align_items(Alignment::Center)
      .child(super::canvas::start_node(ctx))
      .child(super::canvas::edge(ctx))
      .child(choices);
    let panel = self.inspector(ctx, collision.as_ref().map(|entry| entry.name.as_ref()));
    let strip = collision_strip(ctx, id, collision);
    let mut page = parts::page().child(head).with_children(strip).child(
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
          ProjectPage::Pipeline(created, PipelinePanel::default()),
        ));
      }
    });
  }
  fn header(&self, ctx: &mut Ctx, id: orchester_core::id::ProjectId, button: ActionButton, collision: bool) -> Row {
    let nav = Navigation::of(ctx);
    let mut name = FieldInput::new("pipeline-name", ctx.t("pipelines.name"), self.name.clone())
      .placeholder(ctx.t("pipelines.namePlaceholder"))
      .width(Dimension::Px(NAME_WIDTH));
    if collision {
      name = name.invalid();
    }
    let name = name.render(ctx);
    parts::header()
      .spacing(ExtraSpacing::Snug.role())
      .child(
        crate::ui::pressable::opening(nav.to(Route::Project(id, ProjectPage::Pipelines)))
          .id("pipeline-parent")
          .child(Text::new(&ctx.t("pipelines.title")).variant(TypographyStyle::Button)),
      )
      .child(crate::ui::icon::icon(
        Icon::ChevronRight,
        crate::theme::IconSize::Sm,
        lurq::app::theme::PaletteColor::TextMuted,
      ))
      .child(name)
      .child(crate::ui::icon::icon(
        Icon::CircleDashed,
        crate::theme::IconSize::Sm,
        lurq::app::theme::PaletteColor::TextMuted,
      ))
      .child(parts::caption(&ctx.t("pipelines.notCreated")))
      .child(Spacer::new().flex(1.0))
      .with_children((self.name.get().trim().is_empty() || collision).then(|| {
        crate::ui::unavailable::reason_line(
          "pipeline-name-required",
          &ctx.t(if collision {
            "pipelines.freeSlugRequired"
          } else {
            "pipelines.nameRequired"
          }),
        )
        .width(Dimension::Px(141.0))
      }))
      .child(
        ActionButton::new("pipeline-cancel", ButtonKind::Secondary, ctx.t("pipelines.cancel")).on_click({
          let nav = Navigation::of(ctx);
          move || nav.go(Route::Project(id, ProjectPage::Pipelines))
        }),
      )
      .child(button)
  }
  fn choices(&self, ctx: &Ctx, copies: &[orchester_workspace::pipelines::library::LibraryEntry]) -> Column {
    let mut choices = Column::new()
      .width(Dimension::Px(START_WIDTH))
      .spacing(ExtraSpacing::Loose.role())
      .child(Text::new(&ctx.t("pipelines.startFrom")).variant(TypographyStyle::Title))
      .child(parts::note(&ctx.t("pipelines.startHint")))
      .child(Spacer::new().height(Dimension::Px(4.0)));
    for (value, title, detail, glyph) in [
      (0, "pipelines.starter", "pipelines.starterHint", Icon::Workflow),
      (1, "pipelines.copy", "pipelines.copyHint", Icon::Copy),
      (2, "pipelines.empty", "pipelines.noSteps", Icon::Plus),
    ] {
      let start = self.start.clone();
      let mut card = OptionCard::new(
        format!("pipeline-start-{value}"),
        (glyph, ctx.t(title)),
        ctx.t(detail),
        self.start.get() == value,
        move || start.set(value),
      )
      .inline_summary();
      if value == 0 {
        card = card.content(starter_chips(ctx, true));
      }
      if value == 2 {
        card = card.content(parts::note(&ctx.t("pipelines.emptyHint")));
      }
      if value == 1 {
        card = card.content(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(
              SelectField::new(
                "pipeline-copy",
                ctx.t("pipelines.copy"),
                self.copy.clone(),
                copies
                  .iter()
                  .map(|p| {
                    SelectOption::new(p.id.to_string(), p.name.clone()).detail(match p.state {
                      VersionState::Published(v) | VersionState::PublishedWithDraft(v) => {
                        ctx.t_args("pipelines.version", [("version", v.get().to_string())])
                      }
                      VersionState::DraftOnly => ctx.t("pipelines.never"),
                    })
                  })
                  .collect(),
              )
              .placeholder(ctx.t("pipelines.choosePipeline"))
              .width(Dimension::Px(180.0))
              .leading(Icon::Workflow)
              .render(ctx),
            )
            .child(parts::caption(&ctx.t("pipelines.copyVersionHint"))),
        );
      }
      choices = choices.child(card);
    }
    choices.child(crate::ui::unavailable::reason_line(
      "pipeline-presets",
      &ctx.t("pipelines.presets"),
    ))
  }
  fn inspector(&self, ctx: &mut Ctx, collision: Option<&str>) -> Element {
    let slug = Slug::from_name(&self.name.get())
      .map(|s| s.as_str().to_owned())
      .unwrap_or_else(|| ctx.t("pipelines.unset").to_string());
    let description = FieldInput::new(
      "pipeline-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .compact_area()
    .placeholder(ctx.t("pipelines.descriptionPlaceholder"))
    .render(ctx);
    let body = parts::panel_body()
      .child(crate::ui::inspector::section_title(
        "pipeline-new-section",
        &ctx.t("pipelines.pipelineSection"),
        None,
      ))
      .child(property(
        "pipeline-slug",
        &ctx.t("pipelines.slug"),
        &slug,
        if collision.is_some() {
          PropertyValue::MonoDanger
        } else {
          PropertyValue::Mono
        },
      ))
      .child(crate::ui::inspector::control(
        "pipeline-slug-hint",
        "",
        match collision {
          Some(name) => crate::ui::field::field_error(
            "pipeline-slug-collision",
            &ctx.t_args("pipelines.slugUsedBy", [("name", name)]),
          )
          .into(),
          None => Element::from(parts::note(&ctx.t("pipelines.slugHint"))),
        },
      ))
      .child(crate::ui::inspector::control(
        "pipeline-description-row",
        &ctx.t("pipelines.description"),
        description,
      ))
      .child(parts::section(ctx, "pipelines.runsTitle"))
      .child(property(
        "pipeline-new-starts",
        &ctx.t("pipelines.starts"),
        &ctx.t("pipelines.onTaskStart"),
        PropertyValue::Plain,
      ))
      .child(property(
        "pipeline-new-published",
        &ctx.t("pipelines.publishedLabel"),
        &ctx.t("pipelines.never"),
        PropertyValue::Muted,
      ))
      .child(property(
        "pipeline-new-used",
        &ctx.t("pipelines.usedBy"),
        &ctx.t("pipelines.notUsed"),
        PropertyValue::Muted,
      ))
      .child(parts::text_notice(
        "pipeline-nothing-saved",
        &ctx.t("pipelines.nothingSaved"),
        &ctx.t("pipelines.createHint"),
      ));
    parts::panel(
      ctx,
      &ctx.t("pipelines.pipeline"),
      &ctx.t("pipelines.new"),
      body.into(),
      None,
      None,
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
fn collision_strip(
  ctx: &mut Ctx,
  id: orchester_core::id::ProjectId,
  collision: Option<orchester_workspace::pipelines::library::LibraryEntry>,
) -> Option<Element> {
  collision.map(|existing| {
    let nav = Navigation::of(ctx);
    Row::new()
      .id("pipeline-create-error")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(40.0))
      .padding(lurq::node::padding::Padding::horizontal(ExtraSpacing::Tabs.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(lurq::app::theme::PaletteColor::DangerMuted)
      .child(crate::ui::icon::icon(
        Icon::CircleAlert,
        crate::theme::IconSize::Sm,
        lurq::app::theme::PaletteColor::Danger,
      ))
      .child(
        parts::note(&ctx.t_args(
          "pipelines.slugCollision",
          [("slug", existing.slug.to_string()), ("name", existing.name.to_string())],
        ))
        .color(lurq::app::theme::PaletteColor::TextPrimary)
        .flex(1.0),
      )
      .child(
        ActionButton::new(
          "pipeline-open-existing",
          ButtonKind::Ghost,
          ctx.t_args("pipelines.openPipeline", [("name", existing.name.to_string())]),
        )
        .icon(Icon::ArrowUpRight)
        .opens(nav.to(Route::Project(
          id,
          ProjectPage::Pipeline(existing.id, PipelinePanel::default()),
        ))),
      )
      .into()
  })
}
pub(super) fn starter_chips(ctx: &Ctx, compact: bool) -> Row {
  let starter = starter(ctx).pipeline(&Default::default());
  let mut chain = Row::new()
    .width(Dimension::Pct(100.0))
    .wrap()
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center);
  for (stage, level) in orchester_workspace::pipelines::levels::levels(starter.steps())
    .iter()
    .enumerate()
  {
    let group: Element = if compact {
      let mut inline = Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center);
      for (position, index) in level.iter().enumerate() {
        if position > 0 {
          inline = inline.child(parts::note(&ctx.t("pipelines.parallel")).id("pipeline-starter-parallel-marker"));
        }
        inline = inline.child(starter_chip(&starter.steps()[*index], *index));
      }
      inline.into()
    } else {
      let mut group = Column::new().spacing(SpacingSize::Xs);
      for index in level {
        group = group.child(starter_chip(&starter.steps()[*index], *index));
      }
      if level.len() > 1 {
        group = group
          .id("pipeline-starter-parallel")
          .padding(SpacingSize::Xs)
          .border_inside(lurq::app::theme::BorderSize::Sm, lurq::app::theme::PaletteColor::Border)
          .rounded(lurq::app::theme::RadiusSize::Md);
      }
      group.into()
    };
    let mut stage_row = Row::new()
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center);
    if stage > 0 {
      stage_row = stage_row.child(crate::ui::icon::icon(
        Icon::ArrowRight,
        crate::theme::IconSize::Xs,
        lurq::app::theme::PaletteColor::TextMuted,
      ));
    }
    chain = chain.child(stage_row.child(group));
  }
  chain
}
fn starter_chip(step: &orchester_core::resource::Step, index: usize) -> Row {
  Row::new()
    .height(Dimension::Px(24.0))
    .align_items(Alignment::Center)
    .padding(lurq::node::padding::Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .border_inside(lurq::app::theme::BorderSize::Sm, lurq::app::theme::PaletteColor::Border)
    .rounded(lurq::app::theme::RadiusSize::Sm)
    .child(Text::new(&format!("{:02}", index + 1)).variant(crate::theme::ExtraTypography::MonoSmall.role()))
    .child(parts::caption(step.name.as_str()))
    .with_children(step.approval_gate.then(|| {
      crate::ui::icon::icon(
        Icon::Hand,
        crate::theme::IconSize::Xs,
        lurq::app::theme::PaletteColor::Warning,
      )
    }))
}
