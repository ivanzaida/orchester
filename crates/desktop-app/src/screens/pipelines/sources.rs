//! The sources of a pipeline draft: Settings › Sources for every step (`Sources for every step` `Zh2WC` in
//! `A29iv`), the project's beside the pipeline's own, read-only, with *Add folder* and what the steps add; and a
//! step's Sources page (`02f · step sources` `z4g9jL`), the step's own with *Add* over what it inherits from the
//! project and the pipeline. They change the draft only (`orchester_workspace::pipelines::sources`); publishing a
//! version that reads more asks a person (`super::editor`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{resource::StepKey, source::SourcePath};
use orchester_workspace::{
  background,
  messages::{self, Message},
  pipelines::{
    sources::{self as drafted, Listing},
    write::next_version,
  },
  sources::{listed::Listed, project},
  workspace::OpenProject,
};

use super::{
  parts,
  state::{State, View},
};
use crate::{
  theme::ExtraTypography,
  ui::{
    add_source::{AddSource, AddSourceProps, Layout},
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
    pressable::Press,
    row_group::{GroupRow, RowGroup, note},
    state::load_error,
  },
};

/// The project the page edits a pipeline of, which its Sources read the project's sources from.
#[derive(Clone)]
pub struct EditorProject(pub OpenProject);

impl DevtoolsInspectable for EditorProject {}

#[derive(Clone)]
struct Props {
  state: State,
  /// The step whose sources show, with its number and name for the page's way back; `None` for the pipeline's
  /// own.
  step: Option<(StepKey, String)>,
  locked: bool,
}

/// The props are equal when they name the same list: the draft is read from the state each render.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.step, self.locked) == (&other.step, other.locked)
  }
}

impl DevtoolsInspectable for Props {}

/// The project's sources as they are on disk now, and the *Try again* they were read after.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectSources(Vec<Listed>, u64);

impl DevtoolsInspectable for ProjectSources {}

/// Settings › Sources for every step.
pub fn settings_section(ctx: &mut Ctx, state: &State, locked: bool) -> Element {
  ctx.mount_keyed::<Sources>(
    "pipeline-settings-sources",
    Props {
      state: state.clone(),
      step: None,
      locked,
    },
  )
}

/// A step's Sources page, under its head that leads back to `step` (`03 Implement`).
pub fn step_section(ctx: &mut Ctx, props: &super::step_fields::Props, back: String) -> Element {
  ctx.mount_keyed::<Sources>(
    &format!("step-sources-{}", props.step.key),
    Props {
      state: props.state.clone(),
      step: Some((props.step.key.clone(), back)),
      locked: props.locked || props.viewing,
    },
  )
}

struct Sources {
  /// Read the project's sources again: *Try again* after reading them failed.
  revision: Signal<u64>,
}

impl Component for Sources {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let open = ctx.use_context::<EditorProject>().map(|project| project.0);
    let root = open.as_ref().map(|open| open.repo_root.clone());
    let revision = self.revision.get();
    let read = ctx
      .future((root, revision), |(root, revision)| async move {
        let Some(root) = root else {
          return Ok(None);
        };
        background::run(move || project::load_in(&root).map(|listed| Some(ProjectSources(listed, revision))))
          .await?
          .map_err(|error| messages::store_error(&error))
      })
      .state()
      .get();
    // A failed read shows in place of the project's sources and of *Add*: without them a path cannot be checked
    // against what the project already lists, and the step's inherited count would be too low. What was read
    // before it failed does not show again while it is retried.
    let failure = read
      .error
      .clone()
      .filter(|_| read.is_rejected())
      .map(|error| self.load_failed(ctx, &props, &error));
    let project = read
      .data
      .flatten()
      .filter(|read| failure.is_none() && read.1 == revision)
      .map(|read| read.0);
    let place = open.as_ref().zip(project.as_ref()).map(|(open, listed)| {
      let paths: Vec<SourcePath> = listed.iter().map(|listed| listed.path.clone()).collect();
      drafted::place(
        &open.repo_root,
        open.project.name().as_str(),
        &paths,
        &props.state.get().edit,
        &listing(&props),
      )
    });
    let project = project.unwrap_or_default();
    match &props.step {
      None => settings(ctx, &props, &project, (place, failure)),
      Some((key, back)) => step(ctx, &props, (key, back), &project, (place, failure)),
    }
  }
}

impl Sources {
  /// Reading the project's sources failed: why, that nothing can be added until they are read, and *Try again*.
  fn load_failed(&self, ctx: &Ctx, props: &Props, error: &Message) -> Element {
    let revision = self.revision.clone();
    let id = if props.step.is_some() {
      "step-sources-load-error"
    } else {
      "pipeline-sources-load-error"
    };
    let body = ctx.t_args(
      "pipelines.sourcesSection.loadFailedBody",
      [("reason", message_text(ctx, error).to_string())],
    );
    load_error(
      ctx,
      id,
      (&ctx.t("pipelines.sourcesSection.loadFailed"), &body),
      move || revision.update(|revision| *revision += 1),
    )
  }
}

/// Where *Add* checks a path, and why reading the project's sources failed, shown in place of *Add*.
type Adding = (Option<orchester_workspace::sources::check::Place>, Option<Element>);

fn listing(props: &Props) -> Listing {
  props
    .step
    .clone()
    .map_or(Listing::Pipeline, |(key, _)| Listing::Step(key))
}

/// Adds a path that passed the check to the list `props` names.
fn adder(props: &Props) -> Arc<dyn Fn(SourcePath) + Send + Sync> {
  let (state, listing) = (props.state.clone(), listing(props));
  Arc::new(move |path| state.update(|editing| drafted::add(&mut editing.edit, &listing, path)))
}

/// A source of the list `props` names, with its cross unless the list is locked.
fn removable(ctx: &mut Ctx, props: &Props, (id, path): (&str, &SourcePath), hint: Arc<str>) -> GroupRow {
  let row = GroupRow::new(id, path.as_str()).mono().hint(hint);
  if props.locked {
    return row;
  }
  let (state, listing, path) = (props.state.clone(), listing(props), path.clone());
  let label = ctx.t_args("ui.source.remove", [("path", path.as_str())]);
  row.end(
    IconButton::new(format!("{id}-remove"), Icon::X, label)
      .small()
      .on_click(move || state.update(|editing| drafted::remove(&mut editing.edit, &listing, &path)))
      .render(ctx),
  )
}

/// A source listed elsewhere, read-only: its path and where it is from.
fn inherited_row(id: &str, path: &SourcePath, hint: Arc<str>) -> GroupRow {
  GroupRow::new(id, path.as_str()).mono().hint(hint)
}

fn add(ctx: &mut Ctx, props: &Props, (place, failure): Adding, (id, layout, words): (&str, Layout, &str)) -> Element {
  match failure {
    Some(failure) => failure,
    None => ctx.mount_keyed::<AddSource>(
      &format!("{id}-{}", props.step.as_ref().map_or("pipeline", |(key, _)| key.as_str())),
      AddSourceProps {
        id: id.into(),
        place,
        layout,
        add: ctx.t(words),
        locked: props.locked,
        on_add: adder(props),
      },
    ),
  }
}

fn settings(ctx: &mut Ctx, props: &Props, project: &[Listed], adding: Adding) -> Element {
  let edit = props.state.get().shown();
  let mut group = RowGroup::new("pipeline-settings-sources", ctx.t("pipelines.sourcesSection.label"))
    .lead(ctx.t("pipelines.sourcesSection.description"));
  for (index, listed) in project.iter().enumerate() {
    group = group.row(inherited_row(
      &format!("pipeline-project-source-{index}"),
      &listed.path,
      ctx.t("pipelines.sourcesSection.projectMeta"),
    ));
  }
  for (index, path) in edit.sources().pipeline().iter().enumerate() {
    let hint = ctx.t("pipelines.sourcesSection.pipelineMeta");
    group = group.row(removable(ctx, props, (&format!("pipeline-source-{index}"), path), hint));
  }
  group = group.after(add(
    ctx,
    props,
    adding,
    ("pipeline-sources-add", Layout::Picker, "pipelines.sourcesSection.addFolder"),
  ));
  if let Some(read_back) = drafted::steps_read_back(&edit) {
    group = group.after(
      Text::new(&message_text(ctx, &read_back))
        .id("pipeline-sources-steps")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    );
  }
  group.into()
}

fn step(ctx: &mut Ctx, props: &Props, (key, back): (&StepKey, &str), project: &[Listed], adding: Adding) -> Element {
  let editing = props.state.get();
  let edit = editing.shown();
  let own = edit.sources().step(key).to_vec();
  let pipeline = edit.sources().pipeline().to_vec();
  // What it inherits is counted only once the project's sources are read: not while they are, nor when they could
  // not be.
  let meta = adding.0.is_some().then(|| {
    ctx
      .t_args(
        "pipelines.sourcesSection.counts",
        [
          ("own", own.len().to_string()),
          ("inherited", (project.len() + pipeline.len()).to_string()),
        ],
      )
      .to_string()
  });
  let (views, step) = (editing.views.clone(), key.clone());
  let back = parts::back_link(
    "step-page-back",
    back,
    Press::run(move || views.choose(View::Step(step.clone()))),
  );
  let mut mine = RowGroup::new("step-sources-own", ctx.t("pipelines.sourcesSection.thisStep"));
  for (index, path) in own.iter().enumerate() {
    let hint = ctx.t("pipelines.sourcesSection.thisStep");
    mine = mine.row(removable(ctx, props, (&format!("step-source-{index}"), path), hint));
  }
  let mine = mine.after(add(
    ctx,
    props,
    adding,
    ("step-sources-add", Layout::Stacked, "pipelines.sourcesSection.add"),
  ));
  let version = next_version(&editing.opened).map_or_else(String::new, |version| version.get().to_string());
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Lg)
    .child(parts::sub_head("step-sources-title", back, &ctx.t("pipelines.sources"), meta.as_deref()))
    .child(parts::lead("step-sources-detail", &ctx.t("pipelines.sourcesSection.stepDetail")))
    .child(mine)
    .with_children(inherited(ctx, project, &pipeline))
    .child(note("step-sources-tasks", &ctx.t("pipelines.sourcesSection.tasksToo")))
    .child(note(
      "step-sources-draft",
      &ctx.t_args("pipelines.sourcesSection.draftOnly", [("version", version)]),
    ))
    .into()
}

/// What the step inherits: the project's sources and the pipeline's own, each in its group.
fn inherited(ctx: &Ctx, project: &[Listed], pipeline: &[SourcePath]) -> Vec<Element> {
  let groups = [
    (
      "project",
      "pipelines.sourcesSection.fromProject",
      "pipelines.sourcesSection.projectMeta",
      project.iter().map(|listed| listed.path.clone()).collect::<Vec<_>>(),
    ),
    (
      "pipeline",
      "pipelines.sourcesSection.fromPipeline",
      "pipelines.sourcesSection.pipelineMeta",
      pipeline.to_vec(),
    ),
  ];
  groups
    .into_iter()
    .filter(|(.., paths)| !paths.is_empty())
    .map(|(name, heading, meta, paths)| {
      let rows = paths
        .iter()
        .enumerate()
        .map(|(index, path)| inherited_row(&format!("step-sources-{name}-{index}"), path, ctx.t(meta)));
      RowGroup::new(format!("step-sources-{name}"), ctx.t(heading))
        .rows(rows)
        .into()
    })
    .collect()
}
