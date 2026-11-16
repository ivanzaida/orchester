//! Copy in a run's Conversation (ORC-323, ORC-706, `06 · Message actions` `XPDAX`): an agent's message shows Copy in a
//! line under its text while the pointer is on it, which puts its Markdown on the clipboard as it was kept and says
//! *Copied*; the person's message has Copy under its bubble, before where it stands; a fenced code block has Copy in
//! its head, which puts its code without the fence. A secret the runner redacted stays redacted in what is copied. How
//! a message is split at its code blocks is `fences::pieces`'. Both Copies are the pointer's only: a press leaves the
//! focus where it was and Tab skips them.

use std::sync::Arc;

use orchester_app::ui::chat::{
  copy::{COPIED_CLASS, SHOWN_CLASS},
  fences::{CodeBlock, Piece, pieces},
};
use orchester_core::{
  command::{SecretVariable, redact_secrets},
  secret::SecretValue,
};

use super::{
  Window,
  agent_text::agent_says,
  ids, key_of, open, seeded,
  support::clipboard::{Copied, own_clipboard},
};

/// The code of the message's block, which ends the message.
const CODE: &str = "let wait = backoff.next_delay(attempt);\ntokio::time::sleep(wait).await;";

/// What the Builder of KON-12 says: a paragraph, a list, then a Rust block from `sender.rs`.
fn message() -> String {
  format!("Criterion **02** is not covered yet.\n\n- 429\n- 503\n\n```rust sender.rs\n{CODE}\n```")
}

/// KON-12's Conversation with its agent's newest message `text`, which starts with *Criterion*, the window copying
/// into a clipboard of its own that takes what is copied when `takes`: the message's row id, and what was copied.
fn says(text: &str, takes: bool) -> (Window, String, Copied) {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  agent_says(&seeded, &key, text);
  let copied = own_clipboard(&mut window, takes);
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the message", |window| row(window).is_some());
  for _ in 0..5 {
    window.pass();
  }
  let row = row(&mut window).expect("the message's row");
  (window, row, copied)
}

/// The row of the message that starts with *Criterion*, found by its Copy's toolbar.
fn row(window: &mut Window) -> Option<String> {
  ids(window, "conversation-agent-")
    .into_iter()
    .filter_map(|id| id.strip_suffix("-actions").map(str::to_owned))
    .rfind(|row| {
      window
        .texts(row)
        .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Criterion")))
    })
}

fn has_class(window: &mut Window, id: &str, class: &str) -> bool {
  window
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| element.has_class(class))
}

#[test]
fn conversation_copy_shows_on_hover_and_puts_the_messages_markdown() {
  let (mut window, row, copied) = says(&message(), true);
  let actions = format!("{row}-actions");
  assert!(!has_class(&mut window, &actions, SHOWN_CLASS), "no Copy at rest");
  window.hover(&row);
  assert!(
    has_class(&mut window, &actions, SHOWN_CLASS),
    "the pointer on the message shows Copy"
  );
  window.press(&format!("{row}-copy"));
  window.pass();
  assert_eq!(
    *copied.lock(),
    vec![message()],
    "the message's Markdown, as it was kept"
  );
  assert!(has_class(&mut window, &actions, COPIED_CLASS));
  assert_eq!(window.text(&format!("{row}-copy-tooltip")), "Copied");
}

fn bounds(window: &mut Window, id: &str) -> lurq::core::ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

/// Whether `id` is laid out inside the chat's viewport, where the pointer reaches it.
fn in_view(window: &mut Window, id: &str) -> bool {
  let chat = bounds(window, "conversation-chat");
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .is_some_and(|element| element.y >= chat.y && element.y + element.height <= chat.y + chat.height)
}

#[test]
fn conversation_copy_of_an_agents_message_stands_under_its_text() {
  let (mut window, row, _) = says(&message(), true);
  window.hover(&row);
  let message = bounds(&mut window, &format!("{row}-message"));
  let copy = bounds(&mut window, &format!("{row}-copy"));
  assert!(
    copy.y >= message.y + message.height,
    "under its text: {copy:?} {message:?}"
  );
  assert!(copy.x < message.x + message.width / 2.0, "at its start: {copy:?}");
}

#[test]
fn conversation_copy_of_your_message_comes_before_where_it_stands() {
  let (mut window, _) = super::retry_webhooks();
  let message = super::with_text(
    &mut window,
    "conversation-message-",
    "Please keep the retry budget configurable",
  );
  let (bubble, line) = (format!("{message}-bubble"), format!("{message}-meta"));
  for _ in 0..200 {
    if in_view(&mut window, &bubble) && in_view(&mut window, &line) {
      break;
    }
    super::wheel(&mut window, 120.0);
  }
  window.hover(&bubble);
  assert!(has_class(&mut window, &format!("{message}-actions"), SHOWN_CLASS));
  for gone in ["edit", "rewind", "fork", "more"] {
    assert!(
      window.tree.get_element_by_id(&format!("{message}-{gone}")).is_none(),
      "a run's Conversation offers no {gone}"
    );
  }
  let bubble = bounds(&mut window, &format!("{message}-bubble"));
  let line = bounds(&mut window, &format!("{message}-meta"));
  let copy = bounds(&mut window, &format!("{message}-copy"));
  assert!(copy.y >= bubble.y + bubble.height, "under the bubble");
  assert!(
    copy.x + copy.width < line.x + line.width - copy.width,
    "where it stands comes after Copy"
  );
  assert!(
    window
      .texts(&format!("{message}-meta"))
      .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Delivered"))),
    "the line ends with where it stands"
  );
}

#[test]
fn conversation_copy_a_code_block_puts_its_code_without_the_fence() {
  let (mut window, _, copied) = says(&message(), true);
  let copy = ids(&mut window, "conversation-text-")
    .into_iter()
    .find(|id| id.ends_with("-code-1-copy"))
    .expect("the code block's Copy");
  let block = copy.trim_end_matches("-copy").to_owned();
  assert_eq!(window.text(&format!("{block}-language")), "rust · sender.rs");
  assert_eq!(window.text(&copy), "Copy", "Copy is always shown in the head");
  window.press(&copy);
  window.pass();
  assert_eq!(*copied.lock(), vec![CODE.to_owned()], "only the code");
  assert_eq!(window.text(&copy), "Copied");
}

#[test]
fn conversation_copy_says_when_it_could_not_copy() {
  let (mut window, row, copied) = says(&message(), false);
  window.hover(&row);
  window.press(&format!("{row}-copy"));
  window.pass();
  assert!(copied.lock().is_empty());
  assert_eq!(window.text(&format!("{row}-copy-tooltip")), "Couldn't copy");
}

#[test]
fn conversation_copy_keeps_redacted_secrets_redacted() {
  const SECRET: &str = "sk-live-4f9a2b7c";
  let value = SecretValue::new(SECRET.to_owned()).expect("a secret value");
  let secrets = [SecretVariable::new("API_KEY", Arc::new(value))];
  // The runner redacts what an agent wrote before it keeps it.
  let kept = redact_secrets(
    &format!("Criterion: the key `{SECRET}` works.\n\n```sh\ncurl -H 'Authorization: Bearer {SECRET}'\n```"),
    &secrets,
  );
  let (mut window, row, copied) = says(&kept, true);
  window.hover(&row);
  window.press(&format!("{row}-copy"));
  let code = ids(&mut window, "conversation-text-")
    .into_iter()
    .find(|id| id.ends_with("-code-1-copy"))
    .expect("the code block's Copy");
  window.press(&code);
  let copied = copied.lock().clone();
  assert_eq!(copied.len(), 2, "{copied:?}");
  for text in &copied {
    assert!(!text.contains(SECRET), "no secret value is copied: {text}");
    assert!(
      text.contains("<redacted>"),
      "the redaction is copied as it shows: {text}"
    );
  }
  assert_eq!(copied[1], "curl -H 'Authorization: Bearer <redacted>'");
}

#[test]
fn conversation_copy_pieces_split_at_fenced_blocks_that_stand_alone() {
  let text = "Intro\n\n```toml title=\"retry.toml\"\nmax = 5\n```\n\n- a list\n\n  ```sh\n  nested\n  ```\n\nEnd";
  let found = pieces(text);
  assert_eq!(found.len(), 3, "{found:?}");
  assert_eq!(found[0], Piece::Text("Intro\n\n"));
  assert_eq!(
    found[1],
    Piece::Code(CodeBlock {
      language: Some("toml".to_owned()),
      file: Some("retry.toml".to_owned()),
      code: "max = 5".to_owned(),
    })
  );
  assert!(
    matches!(found[2], Piece::Text(rest) if rest.contains("nested") && rest.ends_with("End")),
    "a block in a list stays in the Markdown: {:?}",
    found[2]
  );
  let tilde = pieces("~~~~\nraw ``` text\n~~~~");
  assert_eq!(
    tilde,
    vec![Piece::Code(CodeBlock {
      language: None,
      file: None,
      code: "raw ``` text".to_owned(),
    })]
  );
  assert_eq!(
    pieces("```rust:src/lib.rs\nfn f() {}\n```"),
    vec![Piece::Code(CodeBlock {
      language: Some("rust".to_owned()),
      file: Some("src/lib.rs".to_owned()),
      code: "fn f() {}".to_owned(),
    })]
  );
}

/// The id of what has the focus.
fn focused(window: &mut Window) -> Option<String> {
  window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
}

#[test]
fn conversation_copy_buttons_take_no_focus_from_a_press_or_tab() {
  let (mut window, row, _) = says(&message(), true);
  let code_copy = ids(&mut window, "conversation-text-")
    .into_iter()
    .find(|id| id.ends_with("-code-1-copy"))
    .expect("the code block's Copy");
  window.hover(&row);
  for copy in [format!("{row}-copy"), code_copy] {
    window.press(&copy);
    window.pass();
    assert_ne!(
      focused(&mut window),
      Some(copy),
      "a press leaves the focus where it was"
    );
  }
  for _ in 0..80 {
    window
      .tree
      .key_down("Tab".to_owned(), "Tab".to_owned(), false, false, false);
    window
      .tree
      .key_up("Tab".to_owned(), "Tab".to_owned(), false, false, false);
    window.pass();
    let stop = focused(&mut window);
    assert!(
      !stop.as_deref().is_some_and(|id| id.ends_with("-copy")),
      "Tab skips a Copy: {stop:?}"
    );
  }
}
