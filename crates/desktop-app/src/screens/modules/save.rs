//! Writing a module through the project's task writer off the UI thread, for New module, Edit module, the Summary's
//! properties and the page's *Close module*: a new one, or a change over the module as it was read, refused when it
//! changed since. A module's page keeps what it last wrote ([`Written`]), so a second change made before the page reads
//! again goes over that and not over the read it replaced.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::module::{Module, ModuleDetails};
use orchester_workspace::{
  background, messages::Message, modules::last_write::LastWrite, sharing::Servers, tasks::writer::TaskWriter,
};

use crate::services::Services;

/// What a save writes, and where: over the module as it was read, or as a new one.
#[derive(Clone)]
pub struct SaveModule {
  pub root: PathBuf,
  pub module: Option<Arc<Module>>,
  pub details: ModuleDetails,
}

/// The module as written, and the one it was written over (none for a new one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
  pub over: Option<Arc<Module>>,
  pub module: Arc<Module>,
}

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

/// The module as its page last wrote it ([`LastWrite`]), shared by the page's saves (*Close module* and the Summary's
/// properties).
#[derive(Clone)]
pub struct Written(Signal<Option<LastWrite>>);

impl Written {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self(ctx.signal(None))
  }

  /// What a change to `shown`, the module as the page read it, goes over.
  pub fn base(&self, shown: &Arc<Module>) -> Arc<Module> {
    LastWrite::base(self.0.get_untracked().as_ref(), shown)
  }

  /// Keeps each of `save`'s writes over a module, and has the project read again after every save.
  pub fn follow(&self, ctx: &mut Ctx, save: &SaveAction, feed: crate::project::ProjectFeed) {
    let written = self.0.clone();
    ctx.watch(&save.state(), move |state| {
      let Some(saved) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
        return;
      };
      if let Some(over) = saved.over.clone() {
        written.update(|last| *last = Some(LastWrite::after(last.take(), over, saved.module.clone())));
      }
      feed.saved();
    });
  }
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
  Ok(Saved {
    over: args.module.clone(),
    module: Arc::new(module),
  })
}
