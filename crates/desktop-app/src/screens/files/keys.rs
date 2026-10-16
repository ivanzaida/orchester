//! The page's shortcuts: Ctrl+P (Cmd+P on macOS) opens *Go to file*, Ctrl+F (Cmd+F) the search in the
//! file. Copying the selected lines (Ctrl+C) belongs to the file's pane, which holds them. The shortcuts'
//! words in the key caps are the platform's (`Kbd · Ctrl P` `ebqd2`).

use lurq::app::{ctx::Ctx, events::KeyboardEvent};

use super::Page;
use crate::navigation::shortcuts::Keys;

/// What a key press on the page opens.
pub fn handler(page: &Page<'_>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
  let goto = page.view.goto.clone();
  let find = page.view.find.clone();
  let has_text = page.file().is_some();
  move |event: KeyboardEvent| {
    let keys = Keys::of(&event);
    if !keys.only_command() {
      return;
    }
    if keys.is("p", "KeyP") {
      goto.show();
      event.prevent_default();
    } else if keys.is("f", "KeyF") && has_text {
      find.show();
      event.prevent_default();
    }
  }
}

/// The key cap of *Go to file* and of *Find* on this platform.
pub fn caps(ctx: &Ctx) -> (String, String) {
  if cfg!(target_os = "macos") {
    (
      ctx.t("files.keys.goToCmd").to_string(),
      ctx.t("files.keys.findCmd").to_string(),
    )
  } else {
    (
      ctx.t("files.keys.goToCtrl").to_string(),
      ctx.t("files.keys.findCtrl").to_string(),
    )
  }
}
