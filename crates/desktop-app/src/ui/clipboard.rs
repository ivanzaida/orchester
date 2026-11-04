//! Clipboard I/O boundary; headless windows can own a private clipboard instead of the OS one.
use std::sync::Arc;

#[derive(Clone)]
pub struct Clipboard(Arc<dyn Fn(&str) -> bool + Send + Sync>);
impl Default for Clipboard {
  /// The app's clipboard ([`crate::clipboard::copy`]).
  fn default() -> Self {
    Self::new(crate::clipboard::copy)
  }
}
/// Compared by where it copies to: the window keeps one.
impl PartialEq for Clipboard {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}
impl Clipboard {
  pub fn new(copy: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
    Self(Arc::new(copy))
  }
  /// The clipboard the context provides (a headless window's), else the app's.
  pub fn of(ctx: &mut lurq::app::ctx::Ctx) -> Self {
    ctx.use_context::<Self>().unwrap_or_default()
  }
  pub fn copy(&self, text: &str) -> bool {
    (self.0)(text)
  }
}
