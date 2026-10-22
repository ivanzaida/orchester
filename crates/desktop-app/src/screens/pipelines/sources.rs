//! The sources of a pipeline draft: Pipeline settings › Sources (`Section · Sources` `Q10UQ` in `x4IIjM`), the
//! pipeline's own beside the project's, read-only, with *Add folder* and what the steps add; and the step inspector's
//! Sources (`Section · Sources` `O6piA3` in `pwoBo`), the step's own with *Add* over what it inherits. They change the
//! draft only (`orchester_workspace::pipelines::sources`); publishing a version that reads more asks a person
//! (`super::editor`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{resource::StepKey, source::SourcePath};
use orchester_workspace::{
  background, messages,
  pipelines::{
    sources::{self as drafted, Listing},
    write::next_version,
  },
  sources::{check::Kind, listed::Listed, project},
  workspace::OpenProject,
};

use super::state::State;
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    add_source::{AddSource, AddSourceProps, Layout},
    chip::TagChip,
    form_section::{self, Label, Place},
    icon::{Icon, icon},
    inspector, list_entry,
    message::message_text,
    source_row::SourceRow,
  },
};

/// The control column of a settings row (`Control` `zhPTP`).
const CONTROL_WIDTH: f32 = 300.0;
/// A project's source in Pipeline settings (`Source · H:/projects/kontur-docs` `XVFzy`).
const ENTRY_HEIGHT: f32 = 32.0;

/// The project the editor edits a pipeline of, which its Sources read the project's sources from.
#[derive(Clone)]
pub struct EditorProject(pub OpenProject);

impl DevtoolsInspectable for EditorProject {}

#[derive(Clone)]
struct Props {
  state: State,
  /// The step whose sources show; `None` for the pipeline's own.
  step: Option<StepKey>,
  locked: bool,
}

/// The props are equal when they name the same list: the draft is read from the state each render.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.step, self.locked) == (&other.step, other.locked)
  }
}

impl DevtoolsInspectable for Props {}

/// The project's sources as they are on disk now.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectSources(Vec<Listed>);

impl DevtoolsInspectable for ProjectSources {}

/// Pipeline settings › Sources.
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

/// The step inspector's Sources, for an agent's step `key`.
pub fn step_section(ctx: &mut Ctx, state: &State, key: &StepKey, locked: bool) -> Element {
  ctx.mount_keyed::<Sources>(
    &format!("step-sources-{key}"),
    Props {
      state: state.clone(),
      step: Some(key.clone()),
      locked,
    },
  )
}

struct Sources;

impl Component for Sources {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let open = ctx.use_context::<EditorProject>().map(|project| project.0);
    let root = open.as_ref().map(|open| open.repo_root.clone());
    let read = ctx
      .future(root, |root| async move {
        let Some(root) = root else {
          return Ok(None);
        };
        background::run(move || project::load_in(&root).map(|listed| Some(ProjectSources(listed))))
          .await?
          .map_err(|error| messages::store_error(&error))
      })
      .state()
      .get();
    let project = read.data.flatten().map(|read| read.0);
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
      None => settings(ctx, &props, &project, place),
      Some(key) => step(ctx, &props, key, &project, place),
    }
  }
}

fn listing(props: &Props) -> Listing {
  props.step.clone().map_or(Listing::Pipeline, Listing::Step)
}

/// Adds a path that passed the check to the list `props` names.
fn adder(props: &Props) -> Arc<dyn Fn(SourcePath) + Send + Sync> {
  let (state, listing) = (props.state.clone(), listing(props));
  Arc::new(move |path| state.update(|editing| drafted::add(&mut editing.edit, &listing, path)))
}

fn remover(props: &Props, path: &SourcePath) -> impl Fn() + Send + Sync + 'static {
  let (state, listing, path) = (props.state.clone(), listing(props), path.clone());
  move || state.update(|editing| drafted::remove(&mut editing.edit, &listing, &path))
}

fn settings(
  ctx: &mut Ctx,
  props: &Props,
  project: &[Listed],
  place: Option<orchester_workspace::sources::check::Place>,
) -> Element {
  let edit = props.state.get().shown();
  let mut control = Column::new()
    .id("pipeline-sources-control")
    .width(Dimension::Px(CONTROL_WIDTH))
    .spacing(ExtraSpacing::Snug.role());
  for (index, listed) in project.iter().enumerate() {
    control = control.child(project_entry(ctx, index, listed));
  }
  for (index, path) in edit.sources().pipeline().iter().enumerate() {
    let remove = (!props.locked).then(|| remover(props, path));
    control = control.child(list_entry::removable(
      ctx,
      &format!("pipeline-source-{index}"),
      path.as_str(),
      &ctx.t_args("ui.source.remove", [("path", path.as_str())]),
      remove,
    ));
  }
  control = control.child(ctx.mount_keyed::<AddSource>(
    "pipeline-sources-add",
    AddSourceProps {
      id: "pipeline-sources-add".into(),
      place,
      layout: Layout::Picker,
      add: ctx.t("pipelines.sourcesSection.addFolder"),
      locked: props.locked,
      on_add: adder(props),
    },
  ));
  if let Some(read_back) = drafted::steps_read_back(&edit) {
    control = control.child(
      Text::new(&message_text(ctx, &read_back))
        .id("pipeline-sources-steps")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    );
  }
  form_section::section(
    "pipeline-settings-sources",
    &ctx.t("pipelines.sources"),
    Some(&ctx.t("pipelines.sourcesSection.note")),
    vec![form_section::side_row(
      Label::new("pipeline-sources", ctx.t("pipelines.sourcesSection.label"))
        .description(ctx.t("pipelines.sourcesSection.description")),
      control,
      Place::Last,
      true,
    )],
  )
  .into()
}

/// A project's source, read-only here: its path beside a `project` chip.
fn project_entry(ctx: &Ctx, index: usize, listed: &Listed) -> Element {
  let glyph = match listed.kind {
    Some(Kind::File) => Icon::FileText,
    _ => Icon::Folder,
  };
  Row::new()
    .id(format!("pipeline-project-source-{index}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ENTRY_HEIGHT))
    .padding(Padding::new().left(ExtraSpacing::Loose.role()).right(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .rounded(RadiusSize::Md)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(icon(glyph, crate::theme::IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(listed.path.as_str())
        .id(format!("pipeline-project-source-{index}-path"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(TagChip::new(
      format!("pipeline-project-source-{index}-chip"),
      ctx.t("pipelines.sourcesSection.project"),
    ))
    .into()
}

fn step(
  ctx: &mut Ctx,
  props: &Props,
  key: &StepKey,
  project: &[Listed],
  place: Option<orchester_workspace::sources::check::Place>,
) -> Element {
  let editing = props.state.get();
  let edit = editing.shown();
  let own = edit.sources().step(key).to_vec();
  let pipeline = edit.sources().pipeline().to_vec();
  let meta = ctx.t_args(
    "pipelines.sourcesSection.counts",
    [
      ("own", own.len().to_string()),
      ("inherited", (project.len() + pipeline.len()).to_string()),
    ],
  );
  let mut section = inspector::section("step-sources", SpacingSize::Xs)
    .child(inspector::section_title(
      "step-sources-title",
      &ctx.t("pipelines.sources"),
      Some(&meta),
    ))
    .child(
      Text::new(&ctx.t("pipelines.sourcesSection.stepDetail"))
        .id("step-sources-detail")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .width(Dimension::Pct(100.0)),
    );
  let mut own_rows = Column::new().id("step-sources-own").width(Dimension::Pct(100.0));
  for (index, path) in own.iter().enumerate() {
    let row = SourceRow::new(
      format!("step-source-{index}"),
      path.as_str(),
      None,
      ctx.t("pipelines.sourcesSection.thisStep"),
    )
    .compact();
    let row = if props.locked {
      row
    } else {
      row.removable(
        ctx.t_args("ui.source.remove", [("path", path.as_str())]),
        remover(props, path),
      )
    };
    own_rows = own_rows.child(row.render(ctx));
  }
  section = section.child(own_rows).child(ctx.mount_keyed::<AddSource>(
    &format!("step-sources-add-{key}"),
    AddSourceProps {
      id: "step-sources-add".into(),
      place,
      layout: Layout::Stacked,
      add: ctx.t("pipelines.sourcesSection.add"),
      locked: props.locked,
      on_add: adder(props),
    },
  ));
  section = inherited(ctx, section, project, &pipeline);
  let version = next_version(&editing.opened).map_or_else(String::new, |version| version.get().to_string());
  section
    .child(inspector::note(
      "step-sources-tasks",
      Icon::Info,
      &ctx.t("pipelines.sourcesSection.tasksToo"),
    ))
    .child(inspector::note(
      "step-sources-draft",
      Icon::Info,
      &ctx.t_args("pipelines.sourcesSection.draftOnly", [("version", version)]),
    ))
    .into()
}

/// What the step inherits: the project's sources and the pipeline's own, each under its heading.
fn inherited(ctx: &mut Ctx, mut section: Column, project: &[Listed], pipeline: &[SourcePath]) -> Column {
  let groups = [
    (
      "project",
      "pipelines.sourcesSection.fromProject",
      "pipelines.sourcesSection.projectMeta",
      project
        .iter()
        .map(|listed| (listed.path.clone(), listed.kind))
        .collect::<Vec<_>>(),
    ),
    (
      "pipeline",
      "pipelines.sourcesSection.fromPipeline",
      "pipelines.sourcesSection.pipelineMeta",
      pipeline.iter().map(|path| (path.clone(), None)).collect(),
    ),
  ];
  for (name, heading, meta, paths) in groups {
    if paths.is_empty() {
      continue;
    }
    let mut group = Column::new()
      .id(format!("step-sources-{name}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(ExtraSpacing::Snug.role()))
      .child(Text::new(&ctx.t(heading)).variant(TypographyStyle::Caption));
    for (index, (path, kind)) in paths.iter().enumerate() {
      group = group.child(
        SourceRow::new(
          format!("step-sources-{name}-{index}"),
          path.as_str(),
          *kind,
          ctx.t(meta),
        )
        .compact()
        .render(ctx),
      );
    }
    section = section.child(group);
  }
  section
}
