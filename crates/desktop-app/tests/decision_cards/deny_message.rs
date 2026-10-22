//! *Deny with a message* on the card that waits (`D2fdTQ`), headless on the `cards` seed without a runner: the editor
//! takes the place of the answers and the focus, says what the agent is told as the words are written, waits for words
//! that fit, goes back with *Back* or Esc, sends with its button or Ctrl+Enter (⌘ Enter on macOS) and says why the
//! answer did not go; a denial sent with a message reads, decided, the words it told the agent.

use orchester_app::ui::icon::Icon;

use super::{card_of, click, glyphs, text};
use crate::support::Window;

const WAITING: &str = "Add serde to the webhook payloads";
const WORDS: &str = "Use the workspace's serde_json instead.";

/// Opens the editor of the waiting card; the card's id.
fn editing() -> (Window, String) {
  let (mut window, card) = card_of(WAITING, "cards-approval-");
  window.until("the request's choices", |window| {
    window.tree.get_element_by_id(&format!("{card}-similar")).is_some()
  });
  click(&mut window, &format!("{card}-deny-message"));
  window.until("the editor", |window| {
    window.tree.get_element_by_id(&format!("{card}-message")).is_some()
  });
  (window, card)
}

/// Writes `words` into the message, as a paste or a tool sets the value.
fn write(window: &mut Window, card: &str, words: &str) {
  window
    .tree
    .get_element_by_id_mut(&format!("{card}-message"))
    .and_then(|element| element.as_text_input())
    .expect("the message field")
    .set_value(words);
  window.pass();
  window.pass();
}

fn shown(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

fn focused(window: &Window) -> Option<String> {
  window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
}

#[test]
fn card_deny_with_a_message_opens_its_editor_in_place_of_the_answers() {
  let (mut window, card) = editing();
  assert_eq!(
    text(&mut window, &format!("{card}-message-label")),
    "Message to Builder"
  );
  assert_eq!(focused(&window).as_deref(), Some(format!("{card}-message").as_str()));
  assert_eq!(
    text(&mut window, &format!("{card}-message-told")),
    "Builder is told: “Denied by the person reviewing this run. Continue without it.” The step goes on; Deny never \
     fails it.",
    "what a plain denial tells, until words are written"
  );
  assert_eq!(text(&mut window, &format!("{card}-message-back")), "Back");
  assert_eq!(
    text(&mut window, &format!("{card}-message-send")),
    "Deny and send message"
  );
  let keys = if cfg!(target_os = "macos") {
    "⌘ Enter"
  } else {
    "Ctrl+Enter"
  };
  assert_eq!(
    text(&mut window, &format!("{card}-message-keys")),
    format!("{keys} denies and sends · Esc goes back to the answers")
  );
  for gone in ["-deny-tells", "-answers", "-similar", "-deny-message-reason"] {
    assert!(
      !shown(&mut window, &format!("{card}{gone}")),
      "{gone} is not shown while editing"
    );
  }
  // Without words there is nothing to send.
  click(&mut window, &format!("{card}-message-send"));
  for _ in 0..5 {
    window.tree.tick_futures();
    window.pass();
  }
  assert!(!shown(&mut window, &format!("{card}-refused")), "nothing was sent");
}

#[test]
fn card_deny_with_a_message_says_what_the_agent_is_told_and_what_fits() {
  let (mut window, card) = editing();
  write(&mut window, &card, WORDS);
  assert_eq!(
    text(&mut window, &format!("{card}-message-told")),
    format!(
      "Builder is told: “Denied by the person reviewing this run: {WORDS}” The step goes on; Deny never fails it."
    )
  );
  write(&mut window, &card, &"x".repeat(2001));
  assert_eq!(
    text(&mut window, &format!("{card}-message-note")),
    "At most 2000 characters: this message has 2001."
  );
  click(&mut window, &format!("{card}-message-send"));
  for _ in 0..5 {
    window.tree.tick_futures();
    window.pass();
  }
  assert!(
    !shown(&mut window, &format!("{card}-refused")),
    "too many words are not sent"
  );
}

#[test]
fn card_deny_with_a_message_goes_back_with_back_or_esc() {
  let (mut window, card) = editing();
  write(&mut window, &card, WORDS);
  click(&mut window, &format!("{card}-message-back"));
  assert!(!shown(&mut window, &format!("{card}-message")));
  assert!(shown(&mut window, &format!("{card}-deny-tells")));
  assert!(shown(&mut window, &format!("{card}-similar")));
  click(&mut window, &format!("{card}-deny-message"));
  window.until("the editor again", |window| {
    window.tree.get_element_by_id(&format!("{card}-message")).is_some()
  });
  assert_eq!(
    window.texts(&format!("{card}-message")).unwrap_or_default().join(""),
    WORDS,
    "the words are kept"
  );
  window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  window.pass();
  window.pass();
  assert!(
    !shown(&mut window, &format!("{card}-message")),
    "Esc goes back to the answers"
  );
  assert!(shown(&mut window, &format!("{card}-deny-message")));
}

#[test]
fn card_deny_with_a_message_sends_and_says_why_it_did_not_go() {
  let (mut window, card) = editing();
  write(&mut window, &card, WORDS);
  let send = format!("{card}-message-send");
  click(&mut window, &send);
  // No runner here: the answer reaches none, and the card says so under the editor, which keeps the words and lets
  // them be sent again. (Without a runner the refusal comes within the press's frame, so the loader cannot be caught.)
  window.until("why the answer did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
  assert!(shown(&mut window, &format!("{card}-message")));
  assert!(!glyphs(&mut window, &send).contains(&Icon::LoaderCircle.glyph().to_string()));
  assert_eq!(
    window.texts(&format!("{card}-message")).unwrap_or_default().join(""),
    WORDS
  );
}

#[test]
fn card_deny_with_a_message_sends_with_ctrl_enter() {
  let (mut window, card) = editing();
  write(&mut window, &card, WORDS);
  let (ctrl, meta) = (!cfg!(target_os = "macos"), cfg!(target_os = "macos"));
  window
    .tree
    .key_down_with_meta("Enter".to_owned(), "Enter".to_owned(), false, ctrl, false, meta);
  window.pass();
  window.until("why the answer did not go", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
}

#[test]
fn card_denied_with_a_message_reads_the_words() {
  let (mut window, card) = card_of("Publish the webhook crate", "cards-approval-");
  assert!(text(&mut window, &format!("{card}-decided")).starts_with("Denied · by you at "));
  assert_eq!(
    text(&mut window, &format!("{card}-note")),
    "Your message: “Publish it from CI once the version is bumped.”"
  );
}
