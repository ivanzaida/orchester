//! *Create v1*: stores the pipeline the panel made as its first version (`pipelines::create::create_made_in`, through
//! `pipelines::library::create_with`), the only thing that stores it (D-032). A refusal stores nothing and keeps
//! everything typed; a pipeline created is recorded for the screens that read the pipelines and New task's picker, and
//! the hub opens it once it lists it.

use std::path::PathBuf;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::Pipeline,
};
use orchester_workspace::{background, messages::Message, pipelines::create::create_made_in, tasks::outside::Outside};

use super::NewPipeline;
use crate::{
  screens::pipeline_hub::leave::{UnsavedSignal, mark_unsaved},
  services::Services,
};

/// What *Create v1* stores: the name typed, and the steps and description made.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Ask {
  pub name: String,
  pub body: Pipeline,
}

impl DevtoolsInspectable for Ask {}

/// The pipeline *Create v1* stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Created(pub ResourceId);
impl DevtoolsInspectable for Created {}

pub(super) type Create = FutureAction<Ask, Created, Message>;

/// Creates the pipeline in the project at `root`.
pub(super) fn action(ctx: &mut Ctx, root: PathBuf, project: ProjectId) -> Create {
  let services = Services::of(ctx);
  let (changes, servers) = (services.changes, services.servers);
  ctx.future_action(move |ask: Ask| {
    let (root, changes, servers) = (root.clone(), changes.clone(), servers.clone());
    async move {
      let created = background::run(move || create_made_in((&root, &servers), &ask.name, &ask.body)).await??;
      changes.record(project, Outside::Pipelines);
      changes.record(project, Outside::Tasks);
      Ok(Created(created))
    }
  })
}

/// Once the pipeline is created nothing is left unsaved, and the hub opens it as soon as it lists it.
pub(super) fn follow(ctx: &mut Ctx, create: &Create, (new, unsaved): (&NewPipeline, &UnsavedSignal)) {
  let (new, unsaved) = (new.clone(), unsaved.clone());
  ctx.watch(&create.state(), move |state| {
    if state.is_pending() || !state.is_fulfilled() {
      return;
    }
    if let Some(Created(created)) = state.data {
      mark_unsaved(&unsaved, None);
      new.created(created);
    }
  });
}
