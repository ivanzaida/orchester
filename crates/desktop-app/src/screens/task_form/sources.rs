//! Context › Sources of New task and Edit task (`Sources block` `zcFQJ` in `WLL0c`): the task's own sources with
//! *Add source*, then what it gets from the project and from the pipeline version it runs, read-only, and how many a
//! run starts with. A new task keeps its sources on the form until Create stores them with it; a stored task's are
//! saved as they change. Adding one is asked in place first (`connections::widen`); the rules are
//! `orchester_workspace::sources::task`.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ResourceId, TaskId},
  resource::VersionNumber,
  source::SourcePath,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  sources::{
    check::Kind,
    listed::{Listed, inspect_all},
    task::{self, FromPipeline, Inherited},
  },
};

use crate::{
  mcp::input_gate::Guarded,
  navigation::Navigation,
  routes::{PipelinePanel, ProjectPage, ProjectSettingsTab, Route},
  screens::connections::widen::{WidenConfirm, WidenProps},
  theme::{ExtraSpacing, ExtraTypography, SourcesSpacing},
  ui::{
    add_source::{AddSource, AddSourceProps, Layout},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    source_row::SourceRow,
    unavailable::reason_line,
  },
};

#[derive(Clone)]
pub struct SourcesProps {
  pub repo_root: PathBuf,
  pub project: orchester_core::id::ProjectId,
  /// Where the project's own sources are listed, in their rows (`Folder · Kontur`).
  pub project_name: Arc<str>,
  /// The stored task, whose sources are saved as they change; `None` for a new task.
  pub task: Option<TaskId>,
  /// The published pipeline version the task runs.
  pub pipeline: Option<(ResourceId, VersionNumber)>,
  /// A new task's sources, stored when it is created.
  pub drafted: Signal<Vec<SourcePath>>,
  pub locked: bool,
}

impl PartialEq for SourcesProps {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.repo_root,
      self.task,
      self.pipeline,
      self.locked,
      self.drafted.id(),
    ) == (
      &other.repo_root,
      other.task,
      other.pipeline,
      other.locked,
      other.drafted.id(),
    )
  }
}

impl DevtoolsInspectable for SourcesProps {}

/// What the section shows: the task's own sources and what it inherits, as they are on disk now.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read {
  own: Vec<Listed>,
  inherited: Inherited,
}

impl DevtoolsInspectable for Read {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Change {
  Add(SourcePath),
  Remove(SourcePath),
}

impl DevtoolsInspectable for Change {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Done;

impl DevtoolsInspectable for Done {}

type Save = FutureAction<(PathBuf, TaskId, Option<(ResourceId, VersionNumber)>, Change), Done, Message>;

pub struct TaskSources {
  save: Save,
  revision: Signal<u64>,
  asking: Signal<Option<SourcePath>>,
}

impl Component for TaskSources {
  type Props = SourcesProps;

  fn create(ctx: &mut Ctx) -> Self {
    let save = ctx.future_action(
      |(root, task, pipeline, change): (PathBuf, TaskId, Option<(ResourceId, VersionNumber)>, Change)| async move {
        background::run(move || match &change {
          // Only a person's confirmed question adds one: the question is the confirmation.
          Change::Add(path) => task::add_in(&root, task, pipeline, path, true),
          Change::Remove(path) => task::remove_in(&root, task, path).map(drop),
        })
        .await??;
        Ok(Done)
      },
    );
    let sources = Self {
      save,
      revision: ctx.signal(0),
      asking: ctx.signal(None),
    };
    let (revision, asking) = (sources.revision.clone(), sources.asking.clone());
    ctx.watch(&sources.save.state(), move |state| {
      if state.is_fulfilled() && !state.is_pending() {
        asking.set(None);
        revision.update(|revision| *revision += 1);
      }
    });
    sources
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SourcesProps>().clone();
    let read = self.read(ctx, &props);
    let busy = props.locked || self.save.state().get().is_pending();
    let mut block = Column::new()
      .id("task-form-sources")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SourcesSpacing::Section.role()))
      .spacing(ExtraSpacing::Loose.role())
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Xs)
          .child(Text::new(&ctx.t("task_form.sources.label")).variant(ExtraTypography::Strong.role()))
          .child(
            Text::new(&ctx.t("task_form.sources.description"))
              .id("task-form-sources-description")
              .variant(ExtraTypography::Prose.role())
              .width(Dimension::Pct(100.0)),
          ),
      );
    let Some(read) = read else {
      return block;
    };
    let own: Vec<SourcePath> = read.own.iter().map(|listed| listed.path.clone()).collect();
    let place = task::place(&props.repo_root, &props.project_name, &read.inherited, &own);
    block = block
      .child(subhead(
        ctx,
        "task-form-sources-own",
        "task_form.sources.thisTask",
        own.len(),
        None,
      ))
      .with_children(self.own_rows(ctx, &props, &read.own, busy));
    if let Some(path) = self.asking.get() {
      block = block.child(self.question(ctx, &props, &path));
    }
    if let Some(error) = self.save.state().get().error {
      block = block.child(field_error("task-form-sources-error", &message_text(ctx, &error)));
    }
    let ask = self.asking.clone();
    block = block.child(ctx.mount_keyed::<AddSource>(
      "task-form-sources-add",
      AddSourceProps {
        id: "task-form-sources-add".into(),
        place: Some(place),
        layout: Layout::Line,
        add: ctx.t("task_form.sources.add"),
        locked: busy || self.asking.get().is_some(),
        on_add: Arc::new(move |path| ask.set(Some(path))),
      },
    ));
    block = inherited(ctx, &props, &read.inherited, block);
    block
      .child(summary(ctx, task::run_count(&read.inherited, &own)))
      .child(reason_line(
        "task-form-context-reason",
        &ctx.t("task_form.fields.context.reason"),
      ))
  }
}

impl TaskSources {
  /// The task's own sources (the stored task's or the form's) and what it inherits; what was read before stays on
  /// screen while they are read again.
  fn read(&self, ctx: &mut Ctx, props: &SourcesProps) -> Option<Read> {
    let drafted = props.task.is_none().then(|| props.drafted.get());
    let deps = (
      props.repo_root.clone(),
      props.task,
      props.pipeline,
      drafted,
      self.revision.get(),
    );
    ctx
      .future(deps, |(root, task, pipeline, drafted, _)| async move {
        background::run(move || {
          let store = |error: orchester_store::StoreError| messages::store_error(&error);
          let own = match (task, drafted) {
            (Some(task), _) => task::own_in(&root, task).map_err(store)?,
            (None, drafted) => drafted.unwrap_or_default(),
          };
          Ok::<_, Message>(Read {
            own: inspect_all(&own),
            inherited: task::inherited_in(&root, pipeline).map_err(store)?,
          })
        })
        .await?
      })
      .state()
      .get()
      .data
  }

  fn own_rows(&self, ctx: &mut Ctx, props: &SourcesProps, own: &[Listed], busy: bool) -> Vec<Element> {
    own
      .iter()
      .enumerate()
      .map(|(index, listed)| {
        let meta = match listed.kind {
          Some(Kind::Folder) => ctx.t("task_form.sources.folderOwn"),
          Some(Kind::File) => ctx.t("task_form.sources.fileOwn"),
          None => ctx.t("task_form.sources.goneOwn"),
        };
        let mut row = SourceRow::new(
          format!("task-form-source-{index}"),
          listed.path.as_str(),
          listed.kind,
          meta,
        )
        .status(listed.state);
        if let Some(problem) = listed.problem() {
          row = row.note(message_text(ctx, &problem));
        }
        if !busy {
          let label = ctx.t_args("ui.source.remove", [("path", listed.path.as_str())]);
          row = row.removable(label, self.remover(props, &listed.path));
        }
        row.render(ctx)
      })
      .collect()
  }

  fn remover(&self, props: &SourcesProps, path: &SourcePath) -> impl Fn() + Send + Sync + 'static {
    let (save, drafted, path) = (self.save.clone(), props.drafted.clone(), path.clone());
    let (root, task, pipeline) = (props.repo_root.clone(), props.task, props.pipeline);
    move || match task {
      Some(task) if !save.state().get_untracked().is_pending() => {
        save.run((root.clone(), task, pipeline, Change::Remove(path.clone())));
      }
      Some(_) => {}
      None => drafted.update(|drafted| drafted.retain(|source| *source != path)),
    }
  }

  fn question(&self, ctx: &mut Ctx, props: &SourcesProps, path: &SourcePath) -> Element {
    let reasons = task::widenings(path)
      .iter()
      .map(|reason| message_text(ctx, reason))
      .collect::<Vec<_>>();
    let (save, drafted, keep, asking) = (
      self.save.clone(),
      props.drafted.clone(),
      self.asking.clone(),
      self.asking.clone(),
    );
    let (root, task, pipeline, path) = (props.repo_root.clone(), props.task, props.pipeline, path.clone());
    ctx.mount_keyed::<WidenConfirm>(
      "task-form-sources-confirm",
      WidenProps {
        id: "task-form-sources-question".into(),
        card: false,
        affirming: false,
        automation: Guarded::PermissionWidening,
        title: ctx.t("task_form.sources.question"),
        reasons: reasons.into(),
        confirm: ctx.t("task_form.sources.confirm"),
        keep: ctx.t("task_form.sources.keep"),
        on_keep: Arc::new(move || keep.set(None)),
        on_confirm: Arc::new(move || match task {
          Some(task) if !save.state().get_untracked().is_pending() => {
            save.run((root.clone(), task, pipeline, Change::Add(path.clone())));
          }
          Some(_) => {}
          None => {
            drafted.update(|drafted| {
              if !drafted.contains(&path) {
                drafted.push(path.clone());
              }
            });
            asking.set(None);
          }
        }),
      },
    )
  }
}

/// A list's heading: its name, how many it holds, and where it is changed.
fn subhead(ctx: &Ctx, id: &str, key: &str, count: usize, link: Option<Element>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary),
    )
    .child(
      Text::new(&ctx.t_args("task_form.sources.count", [("count", count.to_string())]))
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Caption),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(link)
}

/// A link at a heading's end (`Link · Edit in Project settings` `QcwW7`): 12 `text_secondary` and an `arrow-up-right`.
fn link(ctx: &mut Ctx, id: &str, key: &str, route: Route) -> Element {
  let open = Navigation::of(ctx).to(route);
  opening(open)
    .id(id.to_owned())
    .child(
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Description))
        .child(icon(
          Icon::ArrowUpRight,
          crate::theme::IconSize::Sm,
          PaletteColor::TextSecondary,
        )),
    )
    .into()
}

/// What the task gets from the project and from its pipeline, read-only.
fn inherited(ctx: &mut Ctx, props: &SourcesProps, inherited: &Inherited, mut block: Column) -> Column {
  let settings = Route::Project(props.project, ProjectPage::Settings(ProjectSettingsTab::Sources));
  let edit = link(
    ctx,
    "task-form-sources-settings",
    "task_form.sources.editInSettings",
    settings,
  );
  block = block.child(subhead(
    ctx,
    "task-form-sources-project",
    "task_form.sources.fromProject",
    inherited.project.len(),
    Some(edit),
  ));
  for (index, listed) in inherited.project.iter().enumerate() {
    let origin = props.project_name.to_string();
    block = block.child(locked_row(
      ctx,
      &format!("task-form-project-source-{index}"),
      listed,
      &origin,
    ));
  }
  let Some((pipeline, _)) = props.pipeline.filter(|_| !inherited.pipeline.is_empty()) else {
    return block;
  };
  let route = Route::Project(props.project, ProjectPage::Pipeline(pipeline, PipelinePanel::Pipeline));
  let view = link(
    ctx,
    "task-form-sources-pipeline-link",
    "task_form.sources.viewPipeline",
    route,
  );
  block = block.child(subhead(
    ctx,
    "task-form-sources-pipeline",
    "task_form.sources.fromPipeline",
    inherited.pipeline.len(),
    Some(view),
  ));
  for (index, from) in inherited.pipeline.iter().enumerate() {
    let origin = pipeline_origin(ctx, from);
    block = block.child(locked_row(
      ctx,
      &format!("task-form-pipeline-source-{index}"),
      &from.listed,
      &origin,
    ));
  }
  block
}

fn pipeline_origin(ctx: &Ctx, from: &FromPipeline) -> String {
  message_text(ctx, &from.origin()).to_string()
}

/// A source listed elsewhere: where, and a lock in place of the cross.
fn locked_row(ctx: &mut Ctx, id: &str, listed: &Listed, origin: &str) -> Element {
  let meta = match listed.kind {
    Some(Kind::Folder) => ctx.t_args("task_form.sources.folderOf", [("origin", origin)]),
    Some(Kind::File) => ctx.t_args("task_form.sources.fileOf", [("origin", origin)]),
    None => ctx.t_args("task_form.sources.goneOf", [("origin", origin)]),
  };
  let mut row = SourceRow::new(id, listed.path.as_str(), listed.kind, meta)
    .status(listed.state)
    .locked();
  if let Some(problem) = listed.problem() {
    row = row.note(message_text(ctx, &problem));
  }
  row.render(ctx)
}

/// How many sources a run of the task starts with (`Summary` `dvmIE`).
fn summary(ctx: &Ctx, count: usize) -> Row {
  let words = match count {
    0 => ctx.t("task_form.sources.summaryNone"),
    1 => ctx.t("task_form.sources.summaryOne"),
    count => ctx.t_args("task_form.sources.summary", [("count", count.to_string())]),
  };
  Row::new()
    .id("task-form-sources-summary")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::Info, crate::theme::IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0),
    )
}

/// The Context section: the task's sources, and that links and notes are not available yet.
pub fn context_section(ctx: &mut Ctx, props: SourcesProps) -> Element {
  let block = ctx.mount_keyed::<TaskSources>("task-form-sources", props);
  crate::ui::form_section::section(
    "task-form-context-section",
    &ctx.t("task_form.sections.context"),
    None,
    vec![block],
  )
  .into()
}
