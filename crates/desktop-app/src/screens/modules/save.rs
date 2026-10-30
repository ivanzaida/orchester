//! Writing a module through the project's task writer off the UI thread, for New module, Edit module, the Summary's
//! properties and the page's *Close module*: a new one, or a change over the module as it was read, refused when it
//! changed since. A module's page keeps what it last wrote ([`Written`]), so a second change made before the page reads
//! again goes over that and not over the read it replaced. The base branch a form chose is taken in the repository
//! before anything is written (a remote one creates its local branch; one that is gone refuses the save) and is set
//! once the details are, on this computer only.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::{
  integration::base::BaseChoice,
  module::{Module, ModuleDetails},
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  messages::Message,
  modules::{
    base::{ModuleBasePick, save_module_base_in, take_module_base_in},
    last_write::LastWrite,
  },
  sharing::Servers,
  tasks::writer::TaskWriter,
};

use crate::services::Services;

/// What a save writes, and where: over the module as it was read, or as a new one.
#[derive(Clone)]
pub struct SaveModule {
  pub root: PathBuf,
  pub module: Option<Arc<Module>>,
  pub details: ModuleDetails,
  /// The base branch a form chose in place of the module's; `None` leaves it.
  pub base: Option<ModuleBasePick>,
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
  let Services { servers, programs, .. } = Services::of(ctx);
  ctx.future_action(move |args: SaveModule| {
    let (servers, programs) = (servers.clone(), programs.clone());
    async move { background::run(move || write(&args, (&servers, &programs))).await.map_err(Refused)? }
  })
}

/// The module as its page last wrote it ([`LastWrite`]), shared by the page's saves (*Close module* and the Summary's
/// properties).
#[derive(Clone)]
pub struct Written(Signal<Last>);

/// The page's last write, none before its first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Last(Option<LastWrite>);

impl DevtoolsInspectable for Last {}

impl Written {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self(ctx.signal(Last::default()))
  }

  /// What a change to `shown`, the module as the page read it, goes over.
  pub fn base(&self, shown: &Arc<Module>) -> Arc<Module> {
    LastWrite::base(self.0.get_untracked().0.as_ref(), shown)
  }

  /// Keeps each of `save`'s writes over a module, and has the project read again after every save.
  pub fn follow(&self, ctx: &mut Ctx, save: &SaveAction, feed: crate::project::ProjectFeed) {
    let written = self.0.clone();
    ctx.watch(&save.state(), move |state| {
      let Some(saved) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
        return;
      };
      if let Some(over) = saved.over.clone() {
        written.update(|last| last.0 = Some(LastWrite::after(last.0.take(), over, saved.module.clone())));
      }
      feed.saved();
    });
  }
}

/// Creates or changes the module through the project's task writer; a change is refused when the module changed
/// since it was read. A base branch chosen is taken first and set last, over the one the module was read with.
fn write(args: &SaveModule, (servers, programs): (&Servers, &Programs)) -> Result<Saved, Refused> {
  let refused = |error: &orchester_workspace::tasks::writer::WriteError| {
    tracing::warn!(%error, "the module was not saved");
    Refused(error.message())
  };
  let base = match &args.base {
    Some(pick) => Some(take_module_base_in(&args.root, programs, pick).map_err(Refused)?),
    None => None,
  };
  let mut writer = TaskWriter::open(&args.root, servers).map_err(|error| refused(&error))?;
  let mut module = match &args.module {
    Some(base) => writer.update_module(base.id(), base.details(), args.details.clone()),
    None => writer.create_module(args.details.clone()),
  }
  .map_err(|error| refused(&error))?;
  drop(writer);
  if let Some(after) = base {
    let before = args
      .module
      .as_ref()
      .map_or(BaseChoice::CheckedOut, |read| read.base().clone());
    save_module_base_in(&args.root, module.id(), (&before, after.clone())).map_err(Refused)?;
    module = module.with_base(after);
  }
  Ok(Saved {
    over: args.module.clone(),
    module: Arc::new(module),
  })
}
