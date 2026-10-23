//! Import resources (`Dzzks` in `orchester2.pen`): a page in place of the one it was opened from (`G69AHh`). Choose
//! the project to import from and its resources (Select resources `DsqQJ`), how everything their copies refer to is
//! resolved in this project (Resolve dependencies `xp4dr`, `jsMUX`), then what the import creates (Create drafts
//! `A5Av4`). Nothing is saved before Import resources: the copies are written at once, as local drafts, and the
//! first one opens, a pipeline with what its import did (`z3vhzo`, [`imported`]). What each step says and allows
//! comes from `orchester_workspace::import`.

mod create;
mod drafts_view;
mod frame;
pub(crate) mod imported;
mod mapping_row;
mod resolve;
mod resource_row;
mod select;
mod words;
mod work;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId},
  import::{Choices, Pick, Plan},
};
use orchester_store::RegisteredProject;
use orchester_workspace::{
  background,
  import::data::{self, Catalogs, LoadNotice, Sources},
  messages::{self, Message},
};

use crate::{navigation::Navigation, project::ShownProject, routes::ImportTab, services::Services};

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  tab: ImportTab,
}

impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, project: &ShownProject, tab: ImportTab) -> Element {
  ctx.mount_keyed::<ImportScreen>(
    &format!("import-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      tab,
    },
  )
}

/// Where the import stands (`Stepper` `s4SpV4`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
  Select,
  Resolve,
  Create,
}

impl DevtoolsInspectable for Step {}

/// The project resources are imported from; `None` until the projects were read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceChoice(Option<ProjectId>);

impl DevtoolsInspectable for SourceChoice {}

/// The selected resources and the content each copy takes, in the order they were selected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Picked(Vec<(ResourceId, Pick)>);

impl DevtoolsInspectable for Picked {}

/// What the person chose for what the selection refers to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ChoiceState(Choices);

impl DevtoolsInspectable for ChoiceState {}

/// What the page's parts share. Nothing of it is saved: leaving the page drops the import.
#[derive(Clone)]
struct State {
  source: Signal<SourceChoice>,
  step: Signal<Step>,
  search: Signal<String>,
  picked: Signal<Picked>,
  choices: Signal<ChoiceState>,
  /// Raised to read both projects again.
  revision: Signal<u64>,
  /// The last failure to write the copies was answered by Check the mappings: Create drafts no longer shows it.
  dismissed: Signal<bool>,
  /// Identifies this import, so trying again never creates a copy twice.
  key: IdempotencyKey,
  work: work::Work,
  navigation: Navigation,
  project: ProjectId,
}

impl State {
  fn plan(&self, catalogs: &Catalogs) -> Plan {
    let Picked(picked) = self.picked.get();
    let ChoiceState(choices) = self.choices.get();
    Plan::new(&catalogs.source, &catalogs.target, &picked, &choices)
  }
}

/// Where the project to import from stands.
enum Source {
  /// The workspace's projects are being read.
  Listing,
  /// The workspace's projects could not be read.
  Unlisted(Message),
  /// The workspace has no other project.
  NoProjects,
  /// The chosen project is being read.
  Reading(RegisteredProject),
  /// The chosen project, or this one, could not be read.
  Failed(LoadNotice),
  Ready(Box<Catalogs>),
}

/// What both projects hold, once read; `None` while no project is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loaded(Option<Box<Catalogs>>);

impl DevtoolsInspectable for Loaded {}

struct ImportScreen {
  state: State,
}

impl Component for ImportScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let navigation = Navigation::of(ctx);
    let work = work::work(ctx);
    let opened = navigation.clone();
    ctx.watch(&work.state(), move |done| {
      if let Some(work::Imported(route)) = done.data.as_ref().filter(|_| !done.is_pending()) {
        opened.replace(route.clone());
      }
    });
    let state = State {
      source: ctx.signal(SourceChoice(None)),
      step: ctx.signal(Step::Select),
      search: ctx.signal(String::new()),
      picked: ctx.signal(Picked::default()),
      choices: ctx.signal(ChoiceState::default()),
      revision: ctx.signal(0),
      dismissed: ctx.signal(false),
      key: IdempotencyKey::generate(),
      work,
      navigation,
      project: ctx.props::<Props>().project.open.project.id(),
    };
    // Resources and choices belong to the project they were read from.
    let (picked, choices, step) = (state.picked.clone(), state.choices.clone(), state.step.clone());
    ctx.watch(&state.source, move |_| {
      picked.set(Picked::default());
      choices.set(ChoiceState::default());
      step.set(Step::Select);
    });
    Self { state }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, tab } = ctx.props::<Props>().clone();
    // Futures are matched by call order: the work comes first, made in `create`, then both reads, every render.
    let sources = self.sources(ctx, &project);
    let (projects, source) = self.source(ctx, &project, sources);
    let name = project.open.project.name().as_str().to_owned();
    let step = match (&source, self.state.step.get()) {
      (Source::Ready(_), step) => step,
      _ => Step::Select,
    };
    let header = frame::header(ctx, &self.state, &name, step);
    let (body, footer) = match (&source, step) {
      (Source::Ready(catalogs), Step::Resolve) => resolve::step(ctx, &self.state, catalogs),
      (Source::Ready(catalogs), Step::Create) => create::step(ctx, &self.state, (&project, catalogs)),
      _ => select::step(ctx, &self.state, (&project, tab), (&projects, &source)),
    };
    frame::page(header, body, footer)
  }
}

impl ImportScreen {
  /// The projects to import from: every other project of the workspace, most recently opened first.
  fn sources(&self, ctx: &mut Ctx, project: &ShownProject) -> Result<Option<Sources>, Message> {
    let workspace = Services::of(ctx).workspace;
    let current = project.open.project.id();
    let read = ctx.future((current, self.state.revision.get()), move |(current, _)| {
      let workspace = workspace.clone();
      async move {
        background::run(move || data::sources(&workspace, current))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot read the workspace's projects");
            messages::store_error(&error)
          })
      }
    });
    let state = read.state().get();
    let pending = state.is_pending();
    match (state.data, state.error) {
      (_, Some(error)) if !pending => Err(error),
      (data, _) => Ok(data),
    }
  }

  /// The project chosen among `projects`: the person's, or the most recently opened one until they pick.
  fn chosen(&self, projects: &[RegisteredProject]) -> Option<RegisteredProject> {
    let SourceChoice(choice) = self.state.source.get();
    if let Some(project) = choice.and_then(|id| projects.iter().find(|project| project.id == id)) {
      return Some(project.clone());
    }
    let first = projects.first()?;
    self.state.source.set(SourceChoice(Some(first.id)));
    Some(first.clone())
  }

  /// The projects to import from, and where the chosen one stands: what it and this project hold once read. What
  /// a project chosen before holds is not shown while the chosen one is read.
  fn source(
    &self,
    ctx: &mut Ctx,
    project: &ShownProject,
    sources: Result<Option<Sources>, Message>,
  ) -> (Vec<RegisteredProject>, Source) {
    let (projects, unlisted) = match sources {
      Ok(Some(Sources(projects))) => (Some(projects), None),
      Ok(None) => (None, None),
      Err(error) => (None, Some(error)),
    };
    let chosen = projects.as_deref().and_then(|projects| self.chosen(projects));
    let deps = (
      chosen.clone(),
      project.open.repo_root.clone(),
      self.state.revision.get(),
    );
    let read = ctx.future(deps, |(chosen, target, _)| async move {
      let Some(chosen) = chosen else {
        return Ok(Loaded(None));
      };
      let (root, name) = (chosen.repo_root.clone(), chosen.name.as_str().to_owned());
      let read = background::run(move || data::load(&root, &target))
        .await
        .map_err(|interrupted| LoadNotice {
          title: interrupted.clone(),
          body: interrupted,
          refused: false,
        })?;
      read.map(|catalogs| Loaded(Some(Box::new(catalogs)))).map_err(|error| {
        tracing::error!(%error, source = %chosen.repo_root.display(), "cannot read the projects to import");
        data::load_notice(&error, &name)
      })
    });
    let state = read.state().get();
    let pending = state.is_pending();
    let source = match (unlisted, &projects, chosen) {
      (Some(error), ..) => Source::Unlisted(error),
      (None, None, _) => Source::Listing,
      (None, Some(_), None) => Source::NoProjects,
      (None, Some(_), Some(chosen)) => match (state.data, state.error) {
        (_, Some(notice)) if !pending => Source::Failed(notice),
        (Some(Loaded(Some(catalogs))), _) if catalogs.source.project().id() == chosen.id => Source::Ready(catalogs),
        _ => Source::Reading(chosen),
      },
    };
    (projects.unwrap_or_default(), source)
  }
}
