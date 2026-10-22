//! Import resources' one write: the batch of copies into this project, at once, as the status bar reports it
//! (`kjn8H`). Once the copies exist the first one opens in place of the page (`z3vhzo`); when they could not be
//! written nothing was, and the page keeps every choice.

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  import::{Content, CopiedResource, ImportBatch},
  resource::{InstructionScope, Resource, ResourceKind},
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
      let copies = batch.copies().to_vec();
      let spec = operations::import_resources(&open, &source, copies.len());
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
        Ok(created) => {
          let page = first_page(&created, &copies).unwrap_or(ProjectPage::Overview(Default::default()));
          Ok(Imported(Route::Project(open.project.id(), page)))
        }
        Err(reason) => Err(failure.unwrap_or(ImportFailure::Failed(reason))),
      }
    }
  })
}

/// The page of the first resource the store says the import created. After Try again of a write that did commit,
/// those are the earlier import's resources, whose ids this batch does not have: their copy is found by kind and
/// slug, which the same choices give again.
fn first_page(created: &[Resource], copies: &[CopiedResource]) -> Option<ProjectPage> {
  let resource = created.first()?;
  let content = copies
    .iter()
    .find(|copy| copy.resource().kind() == resource.kind() && copy.resource().slug() == resource.slug())
    .map(CopiedResource::content);
  Some(page_of(resource, content))
}

/// The page that shows `resource`: a pipeline's editor, an agent profile, an instruction in its scope's tab
/// (Project when its content is not known), a tool, or a skill.
fn page_of(resource: &Resource, content: Option<&Content>) -> ProjectPage {
  let id = resource.id();
  match resource.kind() {
    ResourceKind::Pipeline => ProjectPage::Pipeline(id, Default::default()),
    ResourceKind::AgentProfile => ProjectPage::Agent(id, Default::default()),
    ResourceKind::Instruction => {
      let tab = match content {
        Some(Content::Instruction(instruction)) => match instruction.scope() {
          InstructionScope::Project => InstructionsTab::Project,
          InstructionScope::AgentRole { .. } => InstructionsTab::Roles,
          InstructionScope::Pipeline { .. } => InstructionsTab::Pipelines,
          InstructionScope::Step { .. } => InstructionsTab::Steps,
        },
        _ => InstructionsTab::Project,
      };
      ProjectPage::Instructions(InstructionsView::rule(tab, Some(id)))
    }
    ResourceKind::Tool => ProjectPage::Tool(id, Default::default()),
    ResourceKind::Skill => ProjectPage::Skill(id),
  }
}
