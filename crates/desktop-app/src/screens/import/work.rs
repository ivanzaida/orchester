//! Import resources' one write: the batch of copies into this project, at once, as the status bar reports it
//! (`kjn8H`). Once the copies exist the first one opens in place of the page (`z3vhzo`); when they could not be
//! written nothing was, and the page keeps every choice.

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  import::{Content, CopiedResource, ImportBatch},
  resource::InstructionScope,
};
use orchester_workspace::{
  activity::operations,
  background,
  import::data::{self, ImportFailure},
  workspace::OpenProject,
};

use crate::{
  routes::{InstructionsTab, InstructionsView, ProjectPage, Route},
  services::Services,
};

/// The page the first copy opens on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Imported(pub Route);

impl DevtoolsInspectable for Imported {}

/// This project, the name of the project imported from, and the copies.
pub(super) type Args = (OpenProject, String, ImportBatch);
pub(super) type Work = FutureAction<Args, Imported, ImportFailure>;

pub(super) fn work(ctx: &mut Ctx) -> Work {
  let activity = Services::of(ctx).activity;
  ctx.future_action(move |(open, source, batch): Args| {
    let activity = activity.clone();
    async move {
      let first = batch
        .copies()
        .first()
        .map(|copy| Route::Project(open.project.id(), page_of(copy)))
        .expect("a batch is built only from a plan that copies something");
      let spec = operations::import_resources(&open, &source, batch.copies().len());
      let mut failure = None;
      let written = activity
        .track(spec, async {
          let root = open.repo_root.clone();
          match background::run(move || data::import_into(&root, &batch)).await? {
            Ok(created) => Ok(created),
            Err(error) => {
              tracing::error!(%error, "cannot import the resources");
              failure = Some(data::import_failure(&error));
              Err(data::import_error(&error))
            }
          }
        })
        .await;
      match written {
        Ok(_) => Ok(Imported(first)),
        Err(reason) => Err(failure.unwrap_or(ImportFailure::Failed(reason))),
      }
    }
  })
}

/// The page that shows `copy` once it exists: a pipeline's editor, an agent profile, an instruction in its scope's
/// tab, or a tool.
fn page_of(copy: &CopiedResource) -> ProjectPage {
  let id = copy.resource().id();
  match copy.content() {
    Content::Pipeline(_) => ProjectPage::Pipeline(id, Default::default()),
    Content::AgentProfile(_) => ProjectPage::Agent(id, Default::default()),
    Content::Instruction(instruction) => {
      let tab = match instruction.scope() {
        InstructionScope::Project => InstructionsTab::Project,
        InstructionScope::AgentRole { .. } => InstructionsTab::Roles,
        InstructionScope::Pipeline { .. } => InstructionsTab::Pipelines,
        InstructionScope::Step { .. } => InstructionsTab::Steps,
      };
      ProjectPage::Instructions(InstructionsView::rule(tab, Some(id)))
    }
    Content::Tool(_) => ProjectPage::Tool(id, Default::default()),
  }
}
