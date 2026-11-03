//! The system's save dialog for *Export conversation…* (ORC-327): it opens on a file name and takes Markdown. A test
//! or a QA window provides its own [`SavePicker`] in the context, so no native dialog opens there; the window uses the
//! system's (`rfd`, on Windows and macOS alike).

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use lurq::app::ctx::Ctx;

/// Where the person chose to save; `None` when the dialog was closed without a choice.
pub type Chosen = Pin<Box<dyn Future<Output = Option<PathBuf>> + Send>>;

/// What the dialog is asked: its title, the file name it opens on, and the name of the Markdown files it lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveAsk {
  pub title: Arc<str>,
  pub file_name: String,
  pub kind: Arc<str>,
}

#[derive(Clone)]
pub struct SavePicker(Arc<dyn Fn(SaveAsk) -> Chosen + Send + Sync>);

impl SavePicker {
  pub fn new(pick: impl Fn(SaveAsk) -> Chosen + Send + Sync + 'static) -> Self {
    Self(Arc::new(pick))
  }

  /// The system's save dialog.
  pub fn system() -> Self {
    Self::new(|ask| {
      Box::pin(async move {
        rfd::AsyncFileDialog::new()
          .set_title(ask.title.as_ref())
          .set_file_name(&ask.file_name)
          .add_filter(ask.kind.as_ref(), &["md"])
          .save_file()
          .await
          .map(|handle| handle.path().to_path_buf())
      })
    })
  }

  /// The dialog the context provides, else the system's.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx.use_context::<Self>().unwrap_or_else(Self::system)
  }

  pub fn pick(&self, ask: SaveAsk) -> Chosen {
    (self.0)(ask)
  }
}
