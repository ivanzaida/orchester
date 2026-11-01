//! The system's own file dialog that a file field of the person's tab in the Browser window opens
//! (`docs/design/browser.md`, "What frames do not show"): one file or many, as the field takes them, offering first
//! the extensions it accepts (`rfd`, on Windows and macOS alike). MIME types a field accepts (`image/*`) name no
//! extension the dialog can filter by, so they leave it to any file. A test or a QA copy stands in for the dialog
//! ([`PageFilePicker::stand_in`]), so no native dialog opens there.

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use parking_lot::Mutex;

/// What a page asks the dialog for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageFiles {
  pub multiple: bool,
  /// The field's `accept` entries, as the page lists them.
  pub accept: Vec<String>,
  /// The name of the filter that offers every file, in the person's language.
  pub all_files: Arc<str>,
}

/// The files chosen; `None` when the dialog was closed without a choice.
pub type Chosen = Pin<Box<dyn Future<Output = Option<Vec<PathBuf>>> + Send>>;

type Pick = Arc<dyn Fn(PageFiles) -> Chosen + Send + Sync>;

/// The session's file dialog for pages. Clones share it.
#[derive(Clone, Default)]
pub struct PageFilePicker {
  stand_in: Arc<Mutex<Option<Pick>>>,
}

impl PageFilePicker {
  /// Opens the dialog for `asked`: the stand-in's, else the system's.
  pub fn pick(&self, asked: PageFiles) -> Chosen {
    let stand_in = self.stand_in.lock().clone();
    match stand_in {
      Some(pick) => pick(asked),
      None => system(&asked),
    }
  }

  /// From now on `pick` answers instead of the system's dialog: for a test or a QA copy.
  pub fn stand_in(&self, pick: impl Fn(PageFiles) -> Chosen + Send + Sync + 'static) {
    *self.stand_in.lock() = Some(Arc::new(pick));
  }
}

/// The system's dialog for `asked`.
fn system(asked: &PageFiles) -> Chosen {
  let mut dialog = rfd::AsyncFileDialog::new();
  let extensions = extensions(&asked.accept);
  if !extensions.is_empty() {
    let named: Vec<String> = extensions.iter().map(|extension| format!(".{extension}")).collect();
    dialog = dialog.add_filter(named.join(", "), &extensions);
    // macOS merges every filter into the one set of types it allows, where `*` would be a type of its own.
    if !cfg!(target_os = "macos") {
      dialog = dialog.add_filter(asked.all_files.as_ref(), &["*"]);
    }
  }
  let multiple = asked.multiple;
  Box::pin(async move {
    if multiple {
      let files = dialog.pick_files().await?;
      Some(files.iter().map(|file| file.path().to_path_buf()).collect())
    } else {
      let file = dialog.pick_file().await?;
      Some(vec![file.path().to_path_buf()])
    }
  })
}

/// The extensions among a field's `accept` entries, without their dot: `.pem` is `pem`; a MIME type names none.
pub fn extensions(accept: &[String]) -> Vec<String> {
  accept
    .iter()
    .filter_map(|entry| entry.trim().strip_prefix('.'))
    .filter(|extension| !extension.is_empty())
    .map(str::to_lowercase)
    .collect()
}
