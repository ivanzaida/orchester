//! Where the window's copies go: the app's clipboard ([`crate::clipboard::copy`]), or one of a QA copy's, a test's or
//! the gallery's that leaves the person's alone. [`Clipboard::install`] gives it to lurq once for the whole app, so
//! every copy lurq makes (a field's Ctrl+C, a Markdown or `ui::CodeBlock` Copy) goes there too; the window's root
//! provides the same one to the kit's own Copy buttons, which say whether it took the text.
use std::sync::Arc;

use lurq::app::{App, ClipboardWriter};

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
impl std::fmt::Debug for Clipboard {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("Clipboard")
  }
}
impl ClipboardWriter for Clipboard {
  fn write_text(&self, text: &str) -> bool {
    self.copy(text)
  }
}
impl Clipboard {
  /// Copies go to `copy`, which says whether it took the text.
  pub fn new(copy: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
    Self(Arc::new(copy))
  }
  /// Every copy `app` makes, in every window, goes here in place of the system's clipboard.
  pub fn install(&self, app: &App) {
    app.set_clipboard_writer(Some(Arc::new(self.clone())));
  }
  /// The clipboard the window's root provides (a QA copy's, a test's), else the app's.
  pub fn of(ctx: &mut lurq::app::ctx::Ctx) -> Self {
    ctx.use_context::<Self>().unwrap_or_default()
  }
  /// Puts `text` there; `true` once it is.
  pub fn copy(&self, text: &str) -> bool {
    (self.0)(text)
  }
}
