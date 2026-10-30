//! The window's shortcuts (`docs/design/browser.md`, "Tabs"): Ctrl+T (⌘T on macOS) opens a tab of the person's, Ctrl+W
//! (⌘W) closes the tab shown, Ctrl+Tab and Ctrl+Shift+Tab show the next and the previous tab, Ctrl+L (⌘L) puts the
//! focus in the address line and Ctrl+R (⌘R) reloads the tab shown. The window takes them before the page, as a browser
//! does, also while the page or the address line has the focus; every other key goes on to them. Without a browser,
//! with it in a window of its own, or while it restarts, the window's requests are not made. Ctrl+C, X and V (⌘C, X, V)
//! are the window's only while the page has the focus (`page::input`), so the address line keeps its own.

use lurq::app::events::KeyboardEvent;
use orchester_ipc::browser::{BrowserCommand, BrowserReply, Navigation};

use super::{model, state::Showing};
use crate::navigation::shortcuts::Keys;

/// What a key press asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserShortcut {
  NewTab,
  CloseTab,
  NextTab,
  PreviousTab,
  Address,
  Reload,
}

/// The shortcut `keys` are, if any.
pub fn shortcut(keys: Keys<'_>) -> Option<BrowserShortcut> {
  if keys.only_ctrl() && keys.is("Tab", "Tab") {
    return Some(if keys.shift {
      BrowserShortcut::PreviousTab
    } else {
      BrowserShortcut::NextTab
    });
  }
  if !keys.only_command() {
    return None;
  }
  [
    ("t", "KeyT", BrowserShortcut::NewTab),
    ("w", "KeyW", BrowserShortcut::CloseTab),
    ("l", "KeyL", BrowserShortcut::Address),
    ("r", "KeyR", BrowserShortcut::Reload),
  ]
  .into_iter()
  .find(|(key, code, _)| keys.is(key, code))
  .map(|(_, _, shortcut)| shortcut)
}

/// The window's key handler on `showing`; `address` puts the focus in the address line.
pub fn handler(showing: Showing, address: impl Fn() + Send + Sync + 'static) -> impl Fn(KeyboardEvent) + Send + Sync {
  move |event: KeyboardEvent| {
    let Some(shortcut) = shortcut(Keys::of(&event)) else {
      return;
    };
    event.prevent_default();
    if showing.usable() {
      run(&showing, shortcut, &address);
    }
  }
}

/// What a key press on the page asks of the system clipboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardShortcut {
  Copy,
  Cut,
  Paste,
}

/// The clipboard's shortcut `keys` are, if any: Ctrl (⌘ on macOS) and C, X or V, whichever the page has the focus for.
pub fn clipboard_shortcut(keys: Keys<'_>) -> Option<ClipboardShortcut> {
  if !keys.only_command() {
    return None;
  }
  [
    ("c", "KeyC", ClipboardShortcut::Copy),
    ("x", "KeyX", ClipboardShortcut::Cut),
    ("v", "KeyV", ClipboardShortcut::Paste),
  ]
  .into_iter()
  .find(|(key, code, _)| keys.is(key, code))
  .map(|(_, _, shortcut)| shortcut)
}

/// Takes a key up of a shortcut away from the page, which did not get its key down.
pub fn swallow_up(event: &KeyboardEvent) {
  if shortcut(Keys::of(event)).is_some() {
    event.prevent_default();
  }
}

fn run(showing: &Showing, shortcut: BrowserShortcut, address: &dyn Fn()) {
  let shown = showing.shown().cloned();
  match shortcut {
    BrowserShortcut::Address => address(),
    BrowserShortcut::NewTab => {
      let hooks = showing.person.clone();
      showing
        .commands
        .send_then(BrowserCommand::OpenTab { url: None }, move |reply| {
          if let BrowserReply::TabOpened { tab } = reply {
            hooks.pick(&tab);
          }
        });
    }
    BrowserShortcut::CloseTab => {
      if let Some(tab) = shown {
        showing.commands.send(BrowserCommand::CloseTab { tab });
      }
    }
    BrowserShortcut::NextTab | BrowserShortcut::PreviousTab => {
      let forward = shortcut == BrowserShortcut::NextTab;
      if let Some(tab) = model::cycled(&showing.strip, shown.as_ref(), forward) {
        showing.person.pick(&tab);
      }
    }
    BrowserShortcut::Reload => {
      if let Some(tab) = shown {
        showing.commands.send(BrowserCommand::Navigate {
          tab,
          to: Navigation::Reload,
        });
      }
    }
  }
}
