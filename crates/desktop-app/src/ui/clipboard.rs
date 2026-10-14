//! Clipboard I/O boundary; headless windows can own a private clipboard instead of the OS one.
use std::sync::Arc;

#[derive(Clone)]
pub struct Clipboard(Arc<dyn Fn(&str) -> bool + Send + Sync>);
impl Default for Clipboard {
  fn default() -> Self {
    Self::new(|text| lurq::clipboard::copy_to_clipboard(text))
  }
}
impl Clipboard {
  pub fn new(copy: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
    Self(Arc::new(copy))
  }
  pub fn copy(&self, text: &str) -> bool {
    (self.0)(text)
  }
}
