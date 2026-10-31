//! The status bar's Browser item (`Browser 3 tabs` `PwuEd` in `10 · Run · Browser line` `RGeKO`;
//! `docs/design/browser.md`, "The *Browser* window"): *Browser 3 tabs*, muted, only while the shown project's browser
//! has tabs. A press opens the Browser window on the project, or switches an open window to it. On a narrow bar its
//! words give way, its globe stays.

use std::path::{Path, PathBuf};

use lurq::{
  app::{
    ctx::{Ctx, StreamEmitter},
    theme::PaletteColor,
  },
  layout::layout_kind::ShrinkLimit,
  node::Element,
};
use orchester_workspace::browser::BrowserWindow;

use super::{
  facts::{BarAction, Dispatch},
  item::{dropping, give_way, glyph, pressed},
};
use crate::ui::{icon::Icon, pressable::Press};

/// How many tabs the browser of the project at `root` has; none without a project or before its runner reported it.
fn tabs_of(window: &BrowserWindow, root: Option<&Path>) -> usize {
  root
    .and_then(|root| window.view(root))
    .map_or(0, |view| view.tabs().len())
}

/// How many tabs the browser of the project at `root` has, rendering again when that number changes: the window's
/// changes also count the frames of the tab it shows, which the bar need not follow.
pub fn open_tabs(ctx: &mut Ctx, window: &BrowserWindow, root: Option<PathBuf>) -> usize {
  let follow = window.clone();
  let now = tabs_of(window, root.as_deref());
  ctx
    .stream(root, move |root, emitter: StreamEmitter<usize, ()>| {
      let window = follow.clone();
      async move {
        let mut changes = window.changes();
        let mut told = None;
        loop {
          changes.borrow_and_update();
          let count = tabs_of(&window, root.as_deref());
          if told != Some(count) {
            if !emitter.emit(count) {
              return;
            }
            told = Some(count);
          }
          if changes.changed().await.is_err() {
            return;
          }
        }
      }
    })
    .state()
    .get();
  now
}

/// The item, or nothing without tabs.
pub fn item(ctx: &Ctx, tabs: usize, dispatch: &Dispatch) -> Option<Element> {
  if tabs == 0 {
    return None;
  }
  let key = if tabs == 1 {
    "shell.status.browser.one"
  } else {
    "shell.status.browser.other"
  };
  let words = ctx.t_args(key, [("count", tabs.to_string())]).to_string();
  let dispatch = dispatch.clone();
  Some(
    pressed("status-browser", Press::run(move || dispatch(BarAction::ShowBrowser)))
      .describe("tabs", tabs.to_string())
      .describe("a11y_name", words.clone())
      .flex_shrink(1.0)
      .shrink_order(give_way::BROWSER)
      .shrink_limit(ShrinkLimit::Content)
      .child(glyph(Icon::Globe, PaletteColor::TextMuted))
      .child(dropping(&words, PaletteColor::TextMuted).id("status-browser-text"))
      .into(),
  )
}
