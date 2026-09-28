//! Pipelines: the library (03.04 `LRMQW`, empty 03.A `MqRT7`) and, once a pipeline is opened,
//! its editor (03.01 `Q21HOA` and its states) on the draft or on a published version.

mod editor;
mod library;
mod library_row;
mod new_pipeline;

use lurq::{
  app::{component::Component, ctx::Ctx},
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{PromptText, StarterStep, StarterStepText},
};

use self::{
  editor::{EditorProps, PipelineEditor},
  library::{Library, LibraryProps},
};
use super::ProjectNav;
use crate::{
  navigation::{
    import::ImportTab,
    route::{Page, PipelineView},
  },
  pipelines::library::StarterText,
  ui::link::Link,
  workspace::OpenProject,
};

/// A pipeline to open, as signals and futures carry it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineId(pub ResourceId);

impl lurq::app::component::DevtoolsInspectable for PipelineId {}

/// Provided by [`PipelinesScreen`]: opening a pipeline, going between its draft and its published
/// versions, going back to the library, and saying that pipelines changed so the library reads
/// them again.
#[derive(Clone)]
pub struct PipelinesNav {
  project: ProjectNav,
  revision: Signal<u64>,
}

impl PipelinesNav {
  pub fn open(&self, id: ResourceId) {
    self.changed();
    self.show(id, PipelineView::Draft);
  }

  /// Shows `view` of pipeline `id` as a new history entry.
  pub fn show(&self, id: ResourceId, view: PipelineView) {
    self.project.show(Page::Pipeline(id, view));
  }

  /// A link to `view` of pipeline `id` (see `ui::link`).
  pub fn link(&self, id: ResourceId, view: PipelineView) -> Link {
    self.project.link(Page::Pipeline(id, view))
  }

  /// A link from the library to pipeline `id`, as [`Self::open`] opens it.
  pub fn open_link(&self, id: ResourceId) -> Link {
    let nav = self.clone();
    Link::new(self.project.tabs(), move |open| {
      nav.changed();
      nav.project.open(Page::Pipeline(id, PipelineView::Draft), open);
    })
  }

  /// Shows `view` of pipeline `id` in place of the current entry: after a version was restored
  /// as the draft, going back does not return to the version it replaced the draft with.
  pub fn replace(&self, id: ResourceId, view: PipelineView) {
    self.project.replace(Page::Pipeline(id, view));
  }

  /// A link to the library (see `ui::link`).
  pub fn library_link(&self) -> Link {
    self.project.link(Page::Pipelines)
  }

  pub fn changed(&self) {
    self.revision.update(|revision| *revision += 1);
  }

  /// Opens Import resources on its Pipelines tab.
  pub fn import(&self) -> impl Fn() + Send + Sync + 'static {
    let project = self.project.clone();
    move || project.show_import(ImportTab::Pipelines)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelinesProps {
  pub project: OpenProject,
  /// The pipeline shown in its editor, and what of it; `None` shows the library.
  pub open: Option<(ResourceId, PipelineView)>,
}

impl lurq::app::component::DevtoolsInspectable for PipelinesProps {}

pub struct PipelinesScreen {
  nav: PipelinesNav,
}

impl Component for PipelinesScreen {
  type Props = PipelinesProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: PipelinesNav {
        project: ctx
          .use_context::<ProjectNav>()
          .expect("the project shell provides ProjectNav"),
        revision: ctx.signal(0),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(self.nav.clone());
    let PipelinesProps { project, open } = ctx.props::<PipelinesProps>().clone();
    match open {
      None => ctx.mount_keyed::<Library>(
        "pipelines-library",
        LibraryProps {
          project,
          revision: self.nav.revision.get(),
        },
      ),
      Some((id, view)) => {
        ctx.mount_keyed::<PipelineEditor>(&format!("pipeline-editor-{id}"), EditorProps { project, id, view })
      }
    }
  }
}

/// The starter's step names and instructions and the profile and tool names it looks for,
/// in the current language.
fn starter_text(ctx: &Ctx) -> StarterText {
  StarterText::new(|step| {
    let key = step_text_key(step);
    let name = ctx.t(&format!("pipelines.starter.{key}.name"));
    let instructions = ctx.t(&format!("pipelines.starter.{key}.instructions"));
    let text = StarterStepText {
      name: Name::new(&name).expect("starter step names in the locale files are not blank"),
      instructions: PromptText::new(&instructions)
        .expect("starter instructions in the locale files have no placeholders"),
    };
    (text, ctx.t(&format!("pipelines.starter.{key}.uses")).to_string())
  })
}

const fn step_text_key(step: StarterStep) -> &'static str {
  match step {
    StarterStep::Research => "research",
    StarterStep::Plan => "plan",
    StarterStep::Implement => "implement",
    StarterStep::RunChecks => "runChecks",
    StarterStep::Review => "review",
    StarterStep::CreatePullRequest => "createPullRequest",
  }
}
