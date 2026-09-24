//! Pipelines: the library (03.04 `LRMQW`, empty 03.A `MqRT7`) and, once a pipeline is opened,
//! its editor (03.01 `Q21HOA` and its states).

mod editor;
mod library;
mod library_row;
mod new_pipeline;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
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
use crate::{pipelines::library::StarterText, theme::ExtraColor, workspace::OpenProject};

/// A pipeline to open, as signals and futures carry it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineId(pub ResourceId);

impl lurq::app::component::DevtoolsInspectable for PipelineId {}

/// Provided by [`PipelinesScreen`]: opening a pipeline, going back to the library, and saying
/// that pipelines changed so the library reads them again.
#[derive(Clone)]
pub struct PipelinesNav {
  open: Signal<Option<PipelineId>>,
  revision: Signal<u64>,
}

impl PipelinesNav {
  pub fn open(&self, id: ResourceId) {
    self.changed();
    self.open.set(Some(PipelineId(id)));
  }

  pub fn library(&self) {
    self.open.set(None);
  }

  pub fn changed(&self) {
    self.revision.update(|revision| *revision += 1);
  }
}

pub struct PipelinesScreen {
  nav: PipelinesNav,
}

impl Component for PipelinesScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: PipelinesNav {
        open: ctx.signal(None),
        revision: ctx.signal(0),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    ctx.provide(self.nav.clone());
    let project = ctx.props::<OpenProject>().clone();
    match self.nav.open.get() {
      None => ctx.mount_keyed::<Library>(
        "pipelines-library",
        LibraryProps {
          project,
          revision: self.nav.revision.get(),
        },
      ),
      Some(PipelineId(id)) => {
        ctx.mount_keyed::<PipelineEditor>(&format!("pipeline-editor-{id}"), EditorProps { project, id })
      }
    }
  }
}

/// A dialog centered on the backdrop that dims the window behind it (`a8U756`, `nqqYL`).
fn on_backdrop(dialog: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(ExtraColor::Scrim.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(dialog)
    .into()
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
