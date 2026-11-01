//! Ctrl+C, X and V (⌘ on macOS) with the page's focus (`docs/design/browser.md`, "What frames do not show"): what the
//! window asks of the tab shown, instead of a key press for the page, and what it does with the answer. The clipboard
//! is the test's own, for what is copied and for what is pasted: the system's is the person's.

use std::{path::PathBuf, sync::Arc};

use lurq::app::{
  ClipboardReader,
  events::{ClipboardContent, MouseButton as Button, with_synthetic_input},
};
use orchester_app::ui::clipboard::Clipboard;
use orchester_ipc::browser::{
  BrowserCommand, BrowserMode, BrowserReply, BrowserTabId, KeyAction, MAX_CLIPBOARD_TEXT, Modifiers, PersonInput,
};
use parking_lot::Mutex;

use super::support::{Browser, browser_installed, state};

/// What the test's clipboard holds to paste.
struct Held(ClipboardContent);

impl ClipboardReader for Held {
  fn read_text(&self) -> Option<String> {
    self.0.text_content().map(str::to_owned)
  }

  fn read(&self) -> Option<ClipboardContent> {
    Some(self.0.clone())
  }
}

type Copied = Arc<Mutex<Vec<String>>>;

/// The window with `tab` shown and the page's focus; what it copies goes to the second value.
fn on_page(tab: &str) -> Option<(Browser, Copied)> {
  if !browser_installed() {
    return None;
  }
  let copied = Copied::default();
  let sink = copied.clone();
  let mut browser = Browser::open_with_clipboard(Clipboard::new(move |text| {
    sink.lock().push(text.to_owned());
    true
  }));
  browser.state.record_commands_for_test();
  browser.feed(state(tab, BrowserMode::Headless).0);
  browser.until_shown("browser-page");
  let (x, y) = browser.bounds("browser-page").center();
  browser.tree.mouse_down(x, y, Button::Left);
  browser.tree.mouse_up(x, y, Button::Left);
  browser.pass();
  assert_eq!(browser.focused().as_deref(), Some("browser-page-text"));
  asked(&browser);
  Some((browser, copied))
}

/// What the window asked of the tabs since the last look, without which tab it screencasts.
fn asked(browser: &Browser) -> Vec<BrowserCommand> {
  browser
    .state
    .recorded_commands_for_test()
    .into_iter()
    .map(|(_, command)| command)
    .filter(|command| !matches!(command, BrowserCommand::Show { .. } | BrowserCommand::StopShowing))
    .collect()
}

fn holds(browser: &mut Browser, content: ClipboardContent) {
  browser.tree.set_clipboard_reader(Some(Arc::new(Held(content))));
}

fn tab(id: &str) -> BrowserTabId {
  BrowserTabId::new(id)
}

/// On the person's tab and on an agent's tab shown alike, and with the keys' release kept from the page too.
#[test]
fn browser_clipboard_copy_and_cut_ask_the_tab_shown_and_are_no_keys_for_it() {
  for shown in ["P1", "T1"] {
    let Some((mut browser, _)) = on_page(shown) else {
      return;
    };
    browser.command("c");
    assert_eq!(asked(&browser), [BrowserCommand::CopySelection { tab: tab(shown) }]);
    browser.command("x");
    assert_eq!(asked(&browser), [BrowserCommand::CutSelection { tab: tab(shown) }]);
  }
}

#[test]
fn browser_clipboard_paste_sends_the_clipboards_text_as_text_and_no_key() {
  let Some((mut browser, _)) = on_page("P1") else {
    return;
  };
  holds(&mut browser, ClipboardContent::text("from elsewhere"));
  browser.command("v");
  assert_eq!(
    asked(&browser),
    [BrowserCommand::Input {
      tab: tab("P1"),
      input: PersonInput::Text {
        text: "from elsewhere".to_owned()
      },
    }]
  );
  let hidden = browser
    .tree
    .get_element_by_id_mut("browser-page-text")
    .and_then(|element| element.as_text_input())
    .expect("the page has its hidden input")
    .value();
  assert_eq!(hidden, "", "the hidden input keeps nothing");
}

/// Text only: files on the clipboard paste nothing, and files with a text paste the text.
#[test]
fn browser_clipboard_paste_takes_only_text() {
  let Some((mut browser, _)) = on_page("P1") else {
    return;
  };
  let files = ClipboardContent::files([PathBuf::from("shot.png")]);
  holds(&mut browser, files.clone());
  browser.command("v");
  assert_eq!(asked(&browser), []);
  holds(&mut browser, files.with_text("named"));
  browser.command("v");
  assert_eq!(
    asked(&browser),
    [BrowserCommand::Input {
      tab: tab("P1"),
      input: PersonInput::Text {
        text: "named".to_owned()
      },
    }]
  );
}

/// More than a frame carries is not sent, which would lose the runner's connection; the window says so.
#[test]
fn browser_clipboard_paste_too_long_is_said_and_not_sent() {
  let Some((mut browser, _)) = on_page("P1") else {
    return;
  };
  holds(&mut browser, ClipboardContent::text("x".repeat(MAX_CLIPBOARD_TEXT + 1)));
  browser.command("v");
  assert_eq!(asked(&browser), []);
  browser.until_shown("browser-failed");
}

/// The answer goes on the clipboard; no selection leaves the clipboard as it is.
#[test]
fn browser_clipboard_copy_puts_the_selection_on_the_clipboard() {
  let Some((mut browser, copied)) = on_page("P1") else {
    return;
  };
  browser.state.hold_answers_for_test();
  for (shortcut, text) in [("c", "picked"), ("c", ""), ("x", "taken")] {
    browser.command(shortcut);
    browser
      .state
      .answer_held_for_test(&Ok(BrowserReply::Selection { text: text.to_owned() }));
    let wanted = if text.is_empty() { None } else { Some(text) };
    browser.until("the answer to be handled", |_| {
      wanted.is_none_or(|text| copied.lock().last().is_some_and(|last| last == text))
    });
  }
  assert_eq!(*copied.lock(), ["picked", "taken"]);
}

/// An MCP client driving the window gets nothing: no request, so no selection on the clipboard and nothing cut, and no
/// key for the page either, with the release of the shortcut kept from it too.
#[test]
fn browser_clipboard_shortcuts_refuse_synthetic_input() {
  let Some((mut browser, copied)) = on_page("P1") else {
    return;
  };
  holds(&mut browser, ClipboardContent::text("the person's"));
  for shortcut in ["c", "x", "v"] {
    with_synthetic_input(|| browser.command(shortcut));
    assert_eq!(asked(&browser), [], "{shortcut}");
  }
  assert_eq!(*copied.lock(), [] as [String; 0]);

  browser.command("c");
  assert_eq!(
    asked(&browser),
    [BrowserCommand::CopySelection { tab: tab("P1") }],
    "the person's own press still works"
  );
}

/// A synthetic paste carries its own content, past lurq's refusal to read the system clipboard; it is no person's input
/// either, and the hidden input keeps none of it.
#[test]
fn browser_clipboard_paste_refuses_a_synthetic_paste_with_content() {
  let Some((mut browser, _)) = on_page("P1") else {
    return;
  };
  with_synthetic_input(|| browser.tree.paste(ClipboardContent::text("from an agent")));
  browser.pass();
  assert_eq!(asked(&browser), []);
  let hidden = browser
    .tree
    .get_element_by_id_mut("browser-page-text")
    .and_then(|element| element.as_text_input())
    .expect("the page has its hidden input")
    .value();
  assert_eq!(hidden, "", "the hidden input keeps nothing");

  browser.tree.paste(ClipboardContent::text("from the person"));
  browser.pass();
  assert_eq!(
    asked(&browser),
    [BrowserCommand::Input {
      tab: tab("P1"),
      input: PersonInput::Text {
        text: "from the person".to_owned()
      },
    }],
    "the person's own paste still works"
  );
}

/// Only the page's focus makes them the window's: the address line pastes for itself, and Shift makes it a key of the
/// page's.
#[test]
fn browser_clipboard_shortcuts_need_the_page_focus_and_only_the_command_key() {
  let Some((mut browser, _)) = on_page("P1") else {
    return;
  };
  let (ctrl, meta) = (!cfg!(target_os = "macos"), cfg!(target_os = "macos"));
  browser
    .tree
    .key_down_with_meta("C".to_owned(), "KeyC".to_owned(), true, ctrl, false, meta);
  assert_eq!(
    asked(&browser),
    [BrowserCommand::Input {
      tab: tab("P1"),
      input: PersonInput::Key {
        action: KeyAction::Down,
        key: "C".to_owned(),
        code: "KeyC".to_owned(),
        text: None,
        modifiers: Modifiers {
          ctrl,
          meta,
          shift: true,
          alt: false,
        },
      },
    }],
    "with Shift it is the page's"
  );

  holds(&mut browser, ClipboardContent::text("elsewhere.dev"));
  browser.command("l");
  browser.pass();
  assert_eq!(browser.focused().as_deref(), Some("browser-address"));
  asked(&browser);
  browser.command("v");
  assert_eq!(asked(&browser), [], "the address line takes it");
  let address = browser
    .tree
    .get_element_by_id_mut("browser-address")
    .and_then(|element| element.as_text_input())
    .expect("the address line is a text input")
    .value();
  assert!(address.contains("elsewhere.dev"), "{address}");
}
