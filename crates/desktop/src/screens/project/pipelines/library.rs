//! Pipelines library (03.04 `LRMQW`) and its empty state (03.A `MqRT7`): the project's
//! pipelines with their version state, the New pipeline dialog, and how versions work.
//! Filtering the library and importing from another project do not exist yet, so the filter
//! and Import from project are unavailable.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{name::Name, resource::StarterStep};

use super::{
  PipelineId, PipelinesNav,
  library_row::{column_headings, library_row},
  new_pipeline::{NewPipelineDialog, NewPipelineProps, create_error},
  starter_text,
};
use crate::{
  activity::{Activity, OperationSpec, operations},
  background,
  messages::{self, Message},
  pipelines::{
    describe::step_count,
    library::{LibraryEntry, NewPipeline, StartFrom, StarterText, VersionState, create_in, library_in},
  },
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    field::{ValueTone, field_error, value_box_sized},
    icon::{Icon, IconSize, icon},
    message_text,
    panel::RecoveryPanel,
    summary_step::summary_step,
  },
  workspace::OpenProject,
};

/// Topbar, filter, side panel, empty state icon and starter card sizes (`MUKeR`, `zzNZi`,
/// `Bm3H2`, `xpS2o`, `e9TFM2`).
const TOPBAR_HEIGHT: f32 = 64.0;
const FILTER_WIDTH: f32 = 150.0;
const FILTER_HEIGHT: f32 = 34.0;
const SIDE_PANEL_WIDTH: f32 = 268.0;
const EMPTY_ICON_SIZE: f32 = 48.0;
const EMPTY_WIDTH: f32 = 640.0;

#[derive(Clone, Debug, PartialEq)]
pub struct LibraryProps {
  pub project: OpenProject,
  /// Changes when pipelines were created or edited, so the library reads them again.
  pub revision: u64,
}

impl lurq::app::component::DevtoolsInspectable for LibraryProps {}

type CreateArgs = (PathBuf, Name, StarterText, OperationSpec);
type CreateAction = FutureAction<CreateArgs, PipelineId, Message>;

pub struct Library {
  dialog_open: Signal<bool>,
  dialog_start: Signal<StartFrom>,
  /// Mount key of the dialog, so each opening starts with empty fields.
  dialog_opened: Signal<u64>,
  nav: PipelinesNav,
}

impl Component for Library {
  type Props = LibraryProps;

  fn create(ctx: &mut Ctx) -> Self {
    let nav = ctx
      .use_context::<PipelinesNav>()
      .expect("the pipelines screen provides PipelinesNav");
    let created = create_starter_action(ctx);
    let open = nav.clone();
    ctx.watch(&created.state(), move |state| {
      if let Some(PipelineId(id)) = state.data {
        open.open(id);
      }
    });
    Self {
      dialog_open: ctx.signal(false),
      dialog_start: ctx.signal(StartFrom::Starter),
      dialog_opened: ctx.signal(0),
      nav,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LibraryProps>().clone();
    let create_starter = create_starter_action(ctx);
    let deps = (props.project.repo_root.clone(), props.revision);
    let load = ctx.future(deps, |(repo_root, _)| async move {
      background::run(move || library_in(&repo_root)).await?.map_err(|error| {
        tracing::error!(%error, "cannot read the project's pipelines");
        messages::store_error(&error)
      })
    });
    let state = load.state().get();
    let project = props.project.project.name().as_str();
    let content = match (&state.data, &state.error) {
      (_, Some(error)) => RecoveryPanel::new(
        Icon::AlertCircle,
        &ctx.t("pipelines.library.loadFailed"),
        &message_text(ctx, error),
      )
      .into(),
      (Some(entries), None) if entries.is_empty() => self.empty(ctx, project, &props.project, &create_starter),
      (Some(entries), None) => self.list(ctx, entries, project),
      (None, None) => Column::new().into(),
    };
    let body = Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Section)
      .spacing(SpacingSize::Section)
      .child(Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(content))
      .child(versions_panel(ctx));
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(self.topbar(ctx, project))
      .child(heading(ctx, project))
      .child(body)
      .child(self.dialog(ctx, &props.project, state.data.as_deref()))
  }
}

impl Library {
  fn show_dialog(&self, start: StartFrom) -> impl Fn() + Send + Sync + 'static {
    let (open, chosen, opened) = (
      self.dialog_open.clone(),
      self.dialog_start.clone(),
      self.dialog_opened.clone(),
    );
    move || {
      chosen.set(start);
      opened.update(|count| *count += 1);
      open.set(true);
    }
  }

  /// Breadcrumb and the library actions (`MUKeR`).
  fn topbar(&self, ctx: &Ctx, project: &str) -> Element {
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(TOPBAR_HEIGHT))
      .padding_horizontal(SpacingSize::Section)
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        Text::new(&ctx.t_args("pipelines.library.breadcrumb", [("project", project)]))
          .variant(TypographyStyle::Description),
      )
      .child(Spacer::new().flex(1.0))
      .child(
        ActionButton::new(ctx.t("pipelines.library.import"))
          .icon(Icon::Download)
          .id("pipelines-import")
          .render(ctx),
      )
      .child(
        ActionButton::new(ctx.t("pipelines.library.newPipeline"))
          .primary()
          .icon(Icon::Plus)
          .id("pipelines-new")
          .on_click(self.show_dialog(StartFrom::Starter))
          .render(ctx),
      )
      .into()
  }

  /// Filter, counts, column headings, rows and the ownership note (`Zn2Gx`).
  fn list(&self, ctx: &Ctx, entries: &[LibraryEntry], project: &str) -> Element {
    let with_drafts = entries
      .iter()
      .filter(|entry| {
        matches!(
          entry.state,
          VersionState::DraftOnly | VersionState::PublishedWithDraft(_)
        )
      })
      .count();
    let counts = Message::new(if entries.len() == 1 {
      "pipelines.library.counts.one"
    } else {
      "pipelines.library.counts.other"
    })
    .with("count", entries.len().to_string())
    .with("drafts", with_drafts.to_string());
    let filters = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FILTER_HEIGHT))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(Column::new().width(Dimension::Px(FILTER_WIDTH)).child(value_box_sized(
        ctx,
        "pipelines-filter",
        &ctx.t("pipelines.library.allPipelines"),
        ValueTone::Unavailable,
        None,
        Dimension::Px(FILTER_HEIGHT),
      )))
      .child(Spacer::new().flex(1.0))
      .child(Text::new(&message_text(ctx, &counts)).variant(TypographyStyle::Caption));
    let nav = &self.nav;
    let rows = Column::new()
      .width(Dimension::Pct(100.0))
      .child(column_headings(ctx))
      .with_children(entries.iter().map(|entry| library_row(ctx, entry, nav)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .spacing(SpacingSize::Md)
      .child(filters)
      .child(ScrollVertical::new(rows).width(Dimension::Pct(100.0)))
      .child(ownership_note(ctx, project))
      .into()
  }

  /// No pipelines yet (`X4Uqp`): what a pipeline is, the starter and the two ways to begin.
  fn empty(&self, ctx: &Ctx, project: &str, open: &OpenProject, create: &CreateAction) -> Element {
    let icon_surface = Row::new()
      .size(Dimension::Px(EMPTY_ICON_SIZE), Dimension::Px(EMPTY_ICON_SIZE))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .rounded(RadiusSize::Lg)
      .child(icon(Icon::Workflow, IconSize::Large, PaletteColor::TextPrimary));
    let explanation = Column::new()
      .width(Dimension::Px(EMPTY_WIDTH))
      .spacing(ExtraSpacing::Inline.role())
      .child(
        Text::new(&ctx.t_args("pipelines.empty.title", [("project", project)]))
          .variant(ExtraTypography::DialogTitle.role()),
      )
      .child(Text::new(&ctx.t("pipelines.empty.description")).variant(ExtraTypography::Summary.role()));
    let action = create.state().get();
    let (repo_root, text) = (open.repo_root.clone(), starter_text(ctx));
    let name =
      Name::new(&ctx.t("pipelines.starter.name")).expect("the starter's name in the locale files is not blank");
    let run = create.clone();
    let spec = operations::pipeline_create(open, name.as_str());
    let from_starter = ActionButton::new(ctx.t("pipelines.empty.fromStarter"))
      .primary()
      .icon(Icon::LayoutTemplate)
      .id("pipelines-create-starter")
      .enabled(!action.is_pending())
      .on_click(move || run.run((repo_root.clone(), name.clone(), text.clone(), spec.clone())))
      .render(ctx);
    let empty = ActionButton::new(ctx.t("pipelines.empty.startEmpty"))
      .icon(Icon::Plus)
      .id("pipelines-start-empty")
      .on_click(self.show_dialog(StartFrom::Empty))
      .render(ctx);
    let actions = Row::new()
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(from_starter)
      .child(empty);
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Sm)
      .spacing(SpacingSize::Lg)
      .child(icon_surface)
      .child(explanation)
      .child(starter_preview(ctx, project))
      .child(actions);
    if let Some(error) = action.error.clone().filter(|_| !action.is_pending()) {
      column = column.child(field_error(&message_text(ctx, &error)));
    }
    column.into()
  }

  fn dialog(&self, ctx: &mut Ctx, project: &OpenProject, entries: Option<&[LibraryEntry]>) -> Element {
    if !self.dialog_open.get() {
      return Column::new().into();
    }
    let props = NewPipelineProps {
      project: project.clone(),
      start: self.dialog_start.get(),
      taken: Arc::new(
        entries
          .map(|entries| {
            entries
              .iter()
              .map(|entry| (entry.slug.clone(), entry.name.clone()))
              .collect()
          })
          .unwrap_or_default(),
      ),
      open: self.dialog_open.clone(),
    };
    let key = format!("new-pipeline-{}", self.dialog_opened.get());
    Modal::new(backdrop(ctx.mount_keyed::<NewPipelineDialog>(&key, props)))
      .target(ModalTarget::Root)
      .open(self.dialog_open.clone())
      .into()
  }
}

fn create_starter_action(ctx: &mut Ctx) -> CreateAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, name, starter, spec): CreateArgs| {
    let activity = activity.clone();
    async move {
      let new = NewPipeline {
        name,
        start: StartFrom::Starter,
        starter,
      };
      let root = repo_root.clone();
      let create = async move {
        let created = background::run(move || create_in(&repo_root, new)).await?;
        created.map(PipelineId).map_err(|error| {
          tracing::error!(%error, "cannot create the starter pipeline");
          create_error(&error)
        })
      };
      let target = |created: &PipelineId| Some(operations::pipeline(&root, created.0));
      activity.track_to(spec, create, target).await
    }
  })
}

/// Page heading (`ZTqai`).
fn heading(ctx: &Ctx, project: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t("pipelines.library.title")).variant(ExtraTypography::ScreenTitle.role()))
    .child(
      Text::new(&ctx.t_args("pipelines.library.description", [("project", project)]))
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    )
    .into()
}

/// The starter's six steps (`e9TFM2`).
fn starter_preview(ctx: &Ctx, project: &str) -> Element {
  const STEPS: [(&str, &str); 6] = [
    ("pipelines.starter.research.name", "pipelines.empty.steps.research"),
    ("pipelines.starter.plan.name", "pipelines.empty.steps.plan"),
    ("pipelines.starter.implement.name", "pipelines.empty.steps.implement"),
    ("pipelines.starter.runChecks.name", "pipelines.empty.steps.runChecks"),
    ("pipelines.starter.review.name", "pipelines.empty.steps.review"),
    (
      "pipelines.starter.createPullRequest.name",
      "pipelines.empty.steps.createPullRequest",
    ),
  ];
  let header = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::LayoutTemplate, IconSize::Option, PaletteColor::TextPrimary))
    .child(
      Text::new(&ctx.t("pipelines.empty.starterTitle"))
        .variant(TypographyStyle::Label)
        .flex(1.0),
    )
    .child(Text::new(&message_text(ctx, &step_count(StarterStep::ALL.len()))).variant(TypographyStyle::Caption));
  let steps = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Tight.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(
      STEPS
        .iter()
        .enumerate()
        .map(|(index, (name, summary))| summary_step(index + 1, &ctx.t(name), &ctx.t(summary))),
    );
  Column::new()
    .width(Dimension::Px(EMPTY_WIDTH))
    .padding(ExtraSpacing::Step.role())
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header)
    .child(steps)
    .child(
      Text::new(&ctx.t_args("pipelines.empty.starterResult", [("project", project)]))
        .variant(ExtraTypography::Detail.role()),
    )
    .into()
}

/// Pipelines belong to the project (`hepfF`).
fn ownership_note(ctx: &Ctx, project: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(Icon::FolderLock, IconSize::Notice, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t_args("pipelines.library.ownership", [("project", project)]))
        .variant(ExtraTypography::Explanation.role())
        .flex(1.0),
    )
    .into()
}

/// How versions work (`Bm3H2`).
fn versions_panel(ctx: &Ctx) -> Element {
  let item = |glyph: Icon, title: &str, detail: &str| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .child(icon(glyph, IconSize::Option, PaletteColor::TextSecondary))
      .child(
        Column::new()
          .flex(1.0)
          .spacing(SpacingSize::Sm)
          .child(
            Text::new(&ctx.t(title))
              .variant(TypographyStyle::FieldLabel)
              .color(PaletteColor::TextPrimary),
          )
          .child(Text::new(&ctx.t(detail)).variant(ExtraTypography::Detail.role())),
      )
  };
  Column::new()
    .width(Dimension::Px(SIDE_PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_left(SpacingSize::Lg)
    .spacing(ExtraSpacing::Step.role())
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("pipelines.versions.title")).variant(ExtraTypography::SectionHeading.role()))
    .child(item(
      Icon::Pencil,
      "pipelines.versions.draftTitle",
      "pipelines.versions.draftDetail",
    ))
    .child(item(
      Icon::Upload,
      "pipelines.versions.publishTitle",
      "pipelines.versions.publishDetail",
    ))
    .child(item(
      Icon::Pin,
      "pipelines.versions.keepTitle",
      "pipelines.versions.keepDetail",
    ))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("pipelines.versions.importHint")).variant(ExtraTypography::Detail.role()))
    .into()
}
