//! What a module's page last wrote of its module, so a second change made before the page reads the module again goes
//! over that write and is not refused as changed meanwhile: a change is saved over the details it was made from
//! (`TaskWriter::update_module`), and the page still shows the read the first write replaced.

use std::sync::Arc;

use orchester_core::module::Module;

/// The module as the page last wrote it, and the read the page showed then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastWrite {
  shown: Arc<Module>,
  written: Arc<Module>,
}

impl LastWrite {
  /// What a change to `shown`, the module as the page read it, goes over: `last`'s write while the page still shows
  /// the read that write replaced; else `shown` itself, which then is the newer.
  pub fn base(last: Option<&Self>, shown: &Arc<Module>) -> Arc<Module> {
    match last {
      Some(write) if write.shown == *shown => write.written.clone(),
      _ => shown.clone(),
    }
  }

  /// `last` after `written` was written over `over`: a write over the last one keeps the read that one replaced.
  #[must_use]
  pub fn after(last: Option<Self>, over: Arc<Module>, written: Arc<Module>) -> Self {
    let shown = match last {
      Some(write) if write.written == over => write.shown,
      _ => over,
    };
    Self { shown, written }
  }
}
