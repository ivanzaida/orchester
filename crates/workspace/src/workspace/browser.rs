//! Settings › Browser (`docs/design/browser.md`, "Finding the browser"): which Chromium-based browser agents' pages
//! open in (the program the person chose, else the one installed, else none, saying what was looked for), the folder
//! of the projects' browser profiles beside the application database, and whether the Browser window opens when an
//! agent starts browsing. The runner starts a project's browser by the same rule (`orchester_runner::browser`), so
//! what Settings shows is what agents get.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_exec::browser::{self, BrowserProgram, BrowserUnavailable, NotABrowser};
use orchester_store::{StoreError, location::browser_profiles_beside};
use parking_lot::Mutex;
use thiserror::Error;

use super::Workspace;

/// Where this workspace looks for an installed browser: the computer's own places, or only the folders a test or a QA
/// copy gave ([`Workspace::look_for_browsers_in`]).
#[derive(Clone, Default)]
pub(crate) struct BrowserFolders(Arc<Mutex<Option<Arc<[PathBuf]>>>>);

/// Which browser agents' pages open in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentsBrowser {
  /// The one installed on this computer, as no program is chosen.
  Found(BrowserProgram),
  /// The program the person chose.
  Chosen(BrowserProgram),
  /// The program the person chose, which is no Chromium-based browser's any more (moved, removed): agents' browser
  /// tools answer that the browser is unavailable until another is chosen.
  ChosenUnusable(NotABrowser),
  /// None is installed, and none is chosen: what was looked for and where.
  NotFound(BrowserUnavailable),
}

/// What Settings › Browser shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserSettings {
  pub browser: AgentsBrowser,
  /// The folder holding one browser profile per project; `None` for a database only in memory.
  pub profiles: Option<PathBuf>,
  /// *Open the Browser window when an agent starts browsing*; on until a person turns it off.
  pub opens_window: bool,
}

/// Why a program the person chose was not taken.
#[derive(Debug, Error)]
pub enum ChooseBrowserError {
  #[error(transparent)]
  NotABrowser(#[from] NotABrowser),
  #[error(transparent)]
  Store(#[from] StoreError),
}

impl Workspace {
  /// What Settings › Browser shows. Blocks on SQLite and, finding the browser on Windows, on the registry.
  pub fn browser_settings(&self) -> Result<BrowserSettings, StoreError> {
    let (chosen, opens_window) = {
      let db = self.db.lock();
      (db.browser_program()?, db.browser_window_opens_itself()?)
    };
    let browser = match chosen {
      Some(program) => browser::chosen(&program).map_or_else(AgentsBrowser::ChosenUnusable, AgentsBrowser::Chosen),
      None => self
        .find_browser()
        .map_or_else(AgentsBrowser::NotFound, AgentsBrowser::Found),
    };
    Ok(BrowserSettings {
      browser,
      profiles: self.database_path().as_deref().and_then(browser_profiles_beside),
      opens_window,
    })
  }

  /// Keeps `program`, which the person chose, as agents' browser once it is a Chromium-based browser's; refused, with
  /// the reason, and nothing kept, otherwise.
  pub fn choose_browser_program(&self, program: &Path) -> Result<BrowserProgram, ChooseBrowserError> {
    let taken = browser::chosen(program)?;
    self.db.lock().set_browser_program(Some(taken.path()))?;
    Ok(taken)
  }

  /// Makes this workspace and its clones look for an installed browser in `folders` only, never in the computer's own
  /// places: what a test or a QA copy shows instead of this computer's browsers.
  pub fn look_for_browsers_in(&self, folders: &[PathBuf]) {
    *self.browser_folders.0.lock() = Some(Arc::from(folders));
  }

  fn find_browser(&self) -> Result<BrowserProgram, BrowserUnavailable> {
    let folders = self.browser_folders.0.lock().clone();
    match folders {
      Some(folders) => browser::find_in(&folders),
      None => browser::find(),
    }
  }
}
