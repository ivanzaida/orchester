//! Copy in the Orchestrator chat (ORC-323, 07 `GgXmd`, 07b `Iogxc`): the Orchestrator's answer shows Copy at the end of
//! its name line while the pointer is on it, which puts what it wrote on the clipboard and says *Copied*; the code
//! block in it has Copy in its head, which puts its code without the fence; the person's own message has Copy beside
//! its bubble, which puts their words as they wrote them.

use orchester_app::{
  routes::OrchestratorChat,
  ui::chat::copy::{COPIED_CLASS, SHOWN_CLASS},
};

use super::{
  harness::{Orchestrator, Setup},
  support::clipboard::{Copied, own_clipboard},
};

/// The code of the message's block.
const CODE: &str = "[webhooks.retry]\nmax_attempts = 5";

/// What the person sends: a line, then a TOML block, which the stand-in agent writes back after *Noted:*.
fn message() -> String {
  format!("Include the config shape.\n\n```toml retry.toml\n{CODE}\n```")
}

/// The Orchestrator answered [`message`], the window copying into a clipboard of its own: the answer's id, the
/// person's message's id, and what was copied.
fn answered() -> (Orchestrator, (String, String), Copied) {
  let mut screen = Orchestrator::new(Setup::Ready);
  let copied = own_clipboard(&mut screen.window, true);
  screen.go(OrchestratorChat::New);
  screen.send(&message());
  screen.until("the answer", |screen| {
    !screen.exists("orchestrator-thinking") && !with_suffix(screen, "orchestrator-answer-", "-actions").is_empty()
  });
  for _ in 0..5 {
    screen.window.pass();
  }
  let answer = with_suffix(&mut screen, "orchestrator-answer-", "-actions")
    .pop()
    .expect("the answer's Copy");
  let mine = with_suffix(&mut screen, "orchestrator-message-", "-actions")
    .pop()
    .expect("the message's Copy");
  (screen, (answer, mine), copied)
}

/// The ids under the chat that start with `prefix` and end with `suffix`, without the suffix.
fn with_suffix(screen: &mut Orchestrator, prefix: &str, suffix: &str) -> Vec<String> {
  let mut found = Vec::new();
  if let Some(chat) = screen.window.tree.get_element_by_id("orchestrator-conversation") {
    collect(chat, prefix, suffix, &mut found);
  }
  found
}

fn collect(element: lurq::node::element::ElementRef<'_>, prefix: &str, suffix: &str, into: &mut Vec<String>) {
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
    into.extend(id.strip_suffix(suffix).map(str::to_owned));
  }
  for child in element.children() {
    collect(child, prefix, suffix, into);
  }
}

fn has_class(screen: &mut Orchestrator, id: &str, class: &str) -> bool {
  screen
    .window
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| element.has_class(class))
}

#[test]
fn orchestrator_copy_an_answer_puts_what_it_wrote() {
  let (mut screen, (answer, _), copied) = answered();
  let actions = format!("{answer}-actions");
  assert!(!has_class(&mut screen, &actions, SHOWN_CLASS), "no Copy at rest");
  screen.window.hover(&answer);
  assert!(
    has_class(&mut screen, &actions, SHOWN_CLASS),
    "the pointer on the answer shows Copy"
  );
  screen.press(&format!("{answer}-copy"));
  screen.window.pass();
  let copied = copied.lock().clone();
  assert_eq!(copied.len(), 1, "{copied:?}");
  assert!(
    copied[0].starts_with("Noted: "),
    "what the answer wrote: {:?}",
    copied[0]
  );
  assert!(
    copied[0].contains(&format!("```toml retry.toml\n{CODE}\n```")),
    "its Markdown: {:?}",
    copied[0]
  );
  assert!(has_class(&mut screen, &actions, COPIED_CLASS));
  assert_eq!(screen.text(&format!("{answer}-copy-tooltip")), "Copied");
}

#[test]
fn orchestrator_copy_a_code_block_puts_its_code() {
  let (mut screen, _, copied) = answered();
  let block = with_suffix(&mut screen, "orchestrator-reply-", "-copy")
    .into_iter()
    .find(|id| id.contains("-code-"))
    .expect("the code block's Copy");
  assert_eq!(screen.text(&format!("{block}-language")), "toml · retry.toml");
  screen.press(&format!("{block}-copy"));
  screen.window.pass();
  assert_eq!(*copied.lock(), vec![CODE.to_owned()]);
  assert_eq!(screen.text(&format!("{block}-copy")), "Copied");
}

#[test]
fn orchestrator_copy_the_persons_message_puts_their_words() {
  let (mut screen, (_, mine), copied) = answered();
  screen.window.hover(&format!("{mine}-bubble"));
  assert!(has_class(&mut screen, &format!("{mine}-actions"), SHOWN_CLASS));
  screen.press(&format!("{mine}-copy"));
  screen.window.pass();
  assert_eq!(*copied.lock(), vec![message()]);
}
