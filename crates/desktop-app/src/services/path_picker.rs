//! The system's folder and file pickers for All projects and Add project (*Open project folder…*, *Choose
//! folder…*, *Choose program…*). A test or a QA window provides its own [`PathPicker`] in the context, so no
//! native dialog opens there; the window uses the system's.

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use lurq::app::ctx::Ctx;

/// What a picker chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
  Folder,
  File,
}

impl lurq::app::component::DevtoolsInspectable for PathKind {}

/// What the picker gave; `None` when it was closed without a choice.
pub type Selection = Pin<Box<dyn Future<Output = Option<PathBuf>> + Send>>;

#[derive(Clone)]
pub struct PathPicker(Arc<dyn Fn(PathKind, Arc<str>) -> Selection + Send + Sync>);

impl PathPicker {
  pub fn new(pick: impl Fn(PathKind, Arc<str>) -> Selection + Send + Sync + 'static) -> Self {
    Self(Arc::new(pick))
  }

  /// The system's pickers, titled as asked.
  pub fn system() -> Self {
    Self::new(|kind, title| {
      Box::pin(async move {
        let dialog = rfd::AsyncFileDialog::new().set_title(title.as_ref());
        let picked = match kind {
          PathKind::Folder => dialog.pick_folder().await,
          PathKind::File => dialog.pick_file().await,
        };
        picked.map(|handle| handle.path().to_path_buf())
      })
    })
  }

  /// The picker the context provides, else the system's.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx.use_context::<Self>().unwrap_or_else(Self::system)
  }

  pub fn pick(&self, kind: PathKind, title: Arc<str>) -> Selection {
    (self.0)(kind, title)
  }
}
