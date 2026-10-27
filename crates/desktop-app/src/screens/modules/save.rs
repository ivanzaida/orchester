//! Writing a module through the project's task writer off the UI thread, for New module, Edit module, the Summary's
//! properties and the page's *Close module*: a new one, or a change over the module as it was read, refused when it
//! changed since.

use std::{path::PathBuf, sync::Arc};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::module::{Module, ModuleDetails};
use orchester_workspace::{background, messages::Message, sharing::Servers, tasks::writer::TaskWriter};

use crate::services::Services;

/// What a save writes, and where: over the module as it was read, or as a new one.
#[derive(Clone)]
pub struct SaveModule {
  pub root: PathBuf,
  pub module: Option<Arc<Module>>,
  pub details: ModuleDetails,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub Arc<Module>);

impl DevtoolsInspectable for Saved {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused(pub Message);

impl DevtoolsInspectable for Refused {}

pub type SaveAction = FutureAction<SaveModule, Saved, Refused>;

/// The component's save of a module.
pub fn action(ctx: &mut Ctx) -> SaveAction {
  let servers = Services::of(ctx).servers;
  ctx.future_action(move |args: SaveModule| {
    let servers = servers.clone();
    async move { background::run(move || write(&args, &servers)).await.map_err(Refused)? }
  })
}

/// Creates or changes the module through the project's task writer; a change is refused when the module changed
/// since it was read.
fn write(args: &SaveModule, servers: &Servers) -> Result<Saved, Refused> {
  let refused = |error: &orchester_workspace::tasks::writer::WriteError| {
    tracing::warn!(%error, "the module was not saved");
    Refused(error.message())
  };
  let mut writer = TaskWriter::open(&args.root, servers).map_err(|error| refused(&error))?;
  let module = match &args.module {
    Some(base) => writer.update_module(base.id(), base.details(), args.details.clone()),
    None => writer.create_module(args.details.clone()),
  }
  .map_err(|error| refused(&error))?;
  Ok(Saved(Arc::new(module)))
}
