//! The native reference-file chooser, separate from eager repository discovery.
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

pub type Selection = Pin<Box<dyn Future<Output = Option<PathBuf>> + Send>>;
#[derive(Clone)]
pub struct ReferencePicker(Arc<dyn Fn(Arc<str>) -> Selection + Send + Sync>);
impl ReferencePicker {
  pub fn new(pick: impl Fn(Arc<str>) -> Selection + Send + Sync + 'static) -> Self {
    Self(Arc::new(pick))
  }
  pub fn system() -> Self {
    Self::new(|title| {
      Box::pin(async move {
        rfd::AsyncFileDialog::new()
          .set_title(title.as_ref())
          .pick_file()
          .await
          .map(|file| file.path().to_path_buf())
      })
    })
  }
  pub fn pick(&self, title: Arc<str>) -> Selection {
    (self.0)(title)
  }
}
