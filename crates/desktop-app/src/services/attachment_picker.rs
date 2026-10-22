//! The system's file picker for *Attach* in the chats' composers: any number of files at once. A test or a QA window
//! provides its own [`AttachmentPicker`] in the context, so no native dialog opens there; the window uses the
//! system's (`rfd`, on Windows and macOS alike).

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use lurq::app::ctx::Ctx;

/// The files the picker gave; empty when it was closed without a choice.
pub type Selection = Pin<Box<dyn Future<Output = Vec<PathBuf>> + Send>>;

#[derive(Clone)]
pub struct AttachmentPicker(Arc<dyn Fn(Arc<str>) -> Selection + Send + Sync>);

impl AttachmentPicker {
  pub fn new(pick: impl Fn(Arc<str>) -> Selection + Send + Sync + 'static) -> Self {
    Self(Arc::new(pick))
  }

  /// The system's picker, titled as asked.
  pub fn system() -> Self {
    Self::new(|title| {
      Box::pin(async move {
        rfd::AsyncFileDialog::new()
          .set_title(title.as_ref())
          .pick_files()
          .await
          .unwrap_or_default()
          .into_iter()
          .map(|handle| handle.path().to_path_buf())
          .collect()
      })
    })
  }

  /// The picker the context provides, else the system's.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx.use_context::<Self>().unwrap_or_else(Self::system)
  }

  pub fn pick(&self, title: Arc<str>) -> Selection {
    (self.0)(title)
  }
}
