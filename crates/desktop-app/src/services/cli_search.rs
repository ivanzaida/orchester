//! Where the start page looks for an agent CLI (`screens::all_projects::agent_cli`): the app's search path, and what it
//! can say it looked at. A test or a QA window provides its own [`CliSearch`] in the context, since a test cannot
//! change its process's `PATH` for itself alone and the macOS wording must be reachable on Windows; the window uses the
//! system's.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_exec::search_path::{self, SearchBasis, SearchPath};

#[derive(Clone)]
pub struct CliSearch(Arc<dyn Fn(bool) -> (SearchPath, SearchBasis) + Send + Sync>);

impl CliSearch {
  pub fn new(search: impl Fn(bool) -> (SearchPath, SearchBasis) + Send + Sync + 'static) -> Self {
    Self(Arc::new(search))
  }

  /// This process's search path, the login shell read again first when asked.
  pub fn system() -> Self {
    Self::new(|read_again| {
      if read_again {
        search_path::read_login_shell_again();
      }
      (SearchPath::current(), search_path::search_basis())
    })
  }

  /// The search the context provides, else the system's.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx.use_context::<Self>().unwrap_or_else(Self::system)
  }

  /// Searches now, the login shell read again first when `read_again` says so. Blocks for as long as that read takes.
  pub fn search(&self, read_again: bool) -> (SearchPath, SearchBasis) {
    (self.0)(read_again)
  }
}
