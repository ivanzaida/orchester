//! *Sources for every step* in the panel's *Pipeline* (`Sources for every step` `WFH1F` in `Pipelines · v2` P1), and
//! *Sources* of an open step (P2 `u2rYEH`): the sources one list holds as path pills, each with a cross that takes it
//! off, and *Add*, which asks for a folder and checks it against what the project and the pipeline already list. Both
//! change only the edit the panel holds ([`super::edit`]); the pipeline's list and each step's are told apart by
//! [`Listing`].

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Row,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{resource::PipelineEdit, source::SourcePath};
use orchester_workspace::{
  background,
  messages::{self, Message},
  pipelines::sources::{self as drafted, Listing},
  sources::{listed::Listed, project},
};

use super::edit::Edit;
use crate::{
  screens::hub::agent_row::tag_chip,
  ui::{
    add_source::{AddSource, AddSourceProps, Layout},
    field::field_error,
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
  },
};

#[derive(Clone)]
pub(super) struct Props {
  pub root: std::path::PathBuf,
  /// The project's name, which a path is checked against.
  pub project: Arc<str>,
  /// The list whose sources show.
  pub listing: Listing,
  /// The edit whose sources show.
  pub current: PipelineEdit,
  pub edit: Edit,
  /// A save is under way: nothing is changed now.
  pub locked: bool,
}

/// The props are equal when they show the same: the edit's handle is the panel's own.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.root, &self.project, &self.listing, &self.current, self.locked)
      == (
        &other.root,
        &other.project,
        &other.listing,
        &other.current,
        other.locked,
      )
  }
}
impl DevtoolsInspectable for Props {}

/// The project's sources as they are on disk now.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectSources(Vec<Listed>);
impl DevtoolsInspectable for ProjectSources {}

/// The row's value: the pills and *Add*.
pub(super) fn view(ctx: &mut Ctx, props: Props) -> Element {
  let key = match &props.listing {
    Listing::Pipeline => "pipeline-hub-sources".to_owned(),
    Listing::Step(step) => format!("pipeline-hub-sources-{step}"),
  };
  ctx.mount_keyed::<Sources>(&key, props)
}

struct Sources;

impl Component for Sources {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let root = props.root.clone();
    let read = ctx
      .future(root, |root| async move {
        background::run(move || project::load_in(&root).map(ProjectSources))
          .await?
          .map_err(|error| messages::store_error(&error))
      })
      .state()
      .get();
    // Without the project's sources a path cannot be checked against them, so *Add* waits for them, and says so when
    // they could not be read.
    let failure = read.error.clone().filter(|_| read.is_rejected());
    let project = read.data.filter(|_| failure.is_none()).map(|read| read.0);
    let place = project.as_ref().map(|listed| {
      let paths: Vec<SourcePath> = listed.iter().map(|listed| listed.path.clone()).collect();
      drafted::place(&props.root, &props.project, &paths, &props.current, &props.listing)
    });
    let pills: Vec<Element> = listed(&props.current, &props.listing)
      .iter()
      .enumerate()
      .map(|(index, path)| pill(ctx, &props, (index, path)))
      .collect();
    let add = adding(ctx, &props, (place, failure.as_ref()));
    Row::new()
      .id("pipeline-hub-sources")
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(pills)
      .child(add)
  }
}

/// The sources the list `listing` holds in `edit`.
pub(super) fn listed<'a>(edit: &'a PipelineEdit, listing: &Listing) -> &'a [SourcePath] {
  match listing {
    Listing::Pipeline => edit.sources().pipeline(),
    Listing::Step(key) => edit.sources().step(key),
  }
}

/// A source: its path as a tag, and the cross that takes it off the edit.
fn pill(ctx: &mut Ctx, props: &Props, (index, path): (usize, &SourcePath)) -> Element {
  let id = format!("pipeline-hub-source-{index}");
  let tag = tag_chip(&id, path.as_str());
  let mut pill = Row::new()
    .id(format!("{id}-pill"))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(tag);
  if !props.locked {
    let (edit, path, listing) = (props.edit.clone(), path.clone(), props.listing.clone());
    let label = ctx.t_args("ui.source.remove", [("path", path.as_str())]);
    pill = pill.child(
      IconButton::new(format!("{id}-remove"), Icon::X, label)
        .small()
        .on_click(move || edit.change(|current| drafted::remove(current, &listing, &path)))
        .render(ctx),
    );
  }
  pill.into()
}

/// *Add*, or why the project's sources could not be read.
fn adding(
  ctx: &mut Ctx,
  props: &Props,
  (place, failure): (Option<orchester_workspace::sources::check::Place>, Option<&Message>),
) -> Element {
  if let Some(error) = failure {
    return field_error("pipeline-hub-sources-error", &message_text(ctx, error)).into();
  }
  let (edit, listing) = (props.edit.clone(), props.listing.clone());
  ctx.mount_keyed::<AddSource>(
    "pipeline-hub-sources-add",
    AddSourceProps {
      id: "pipeline-hub-sources-add".into(),
      place,
      layout: Layout::Picker,
      add: ctx.t("pipeline_hub.settings.addSource"),
      locked: props.locked,
      on_add: Arc::new(move |path| edit.change(|current| drafted::add(current, &listing, path))),
    },
  )
}
