//! The person's report: at the end of a conversation they send a message and wait, and the Orchestrator's answer, a
//! long Markdown reply much taller than the view, lands with only its heading in view. The window draws only when
//! something asks for a frame, so these tests lay it out only while it asks (`wait`), as the event loop does: the
//! chat must reach the reply's end within those frames, whether the reply comes at once or grows in parts, and also
//! after a wheel over the opened text of a file sent with the message, which scrolls that text and not the chat.

use std::time::{Duration, Instant};

use orchester_core::{resource::ModelId, time::Timestamp};

use super::{
  harness::Orchestrator,
  paste::{paste_long_text, sent_message, wheel_over},
  scroll::{long_conversation, newest_foot_gap, turns},
};

/// A Markdown reply as the Orchestrator writes a long one, tagged `tag`: a heading, paragraphs, a list, a link, a
/// code block and a table, many times taller than the view.
fn markdown(tag: &str) -> String {
  let paragraph = |n: usize| {
    format!(
      "{tag} paragraph {n}: the queue holds three tasks, two of them wait for a review and one for its runner; \
       each review names what it checked and what it left for the person to decide."
    )
  };
  let code: Vec<String> = (1..=24)
    .map(|line| format!("let step_{line} = queue.next({line});"))
    .collect();
  let rows: Vec<String> = (1..=8).map(|row| format!("| KON-{row} | waiting | {row}m |")).collect();
  format!(
    "## {tag} heading\n\n{}\n\n{}\n\n- first item of {tag}\n- second item\n- [the task](https://example.com/kon-1)\n\n\
     ```rust\n{}\n```\n\n| Task | State | Waited |\n| --- | --- | --- |\n{}\n\n{}\n\n{tag} last line.",
    paragraph(1),
    paragraph(2),
    code.join("\n"),
    rows.join("\n"),
    (3..=8).map(paragraph).collect::<Vec<_>>().join("\n\n"),
  )
}

/// Runs the window as its event loop does until `done` holds and it asks for no more frames: what its futures
/// finished is delivered, and it is laid out only while it asks for a frame (`Tree::needs_redraw`); panics with
/// `what` after 30 seconds.
fn wait(screen: &mut Orchestrator, what: &str, mut done: impl FnMut(&mut Orchestrator) -> bool) {
  let deadline = Instant::now() + Duration::from_secs(30);
  loop {
    screen.window.tree.tick_futures();
    if screen.window.tree.needs_redraw() {
      screen.window.pass();
    } else if done(screen) {
      return;
    } else {
      assert!(Instant::now() < deadline, "timed out waiting for {what}");
      std::thread::sleep(Duration::from_millis(5));
    }
  }
}

/// Asserts the chat shows the very end of its newest turn, the one that shows `newest`, with `last` laid out in
/// it, and follows it.
fn at_end_of(screen: &mut Orchestrator, (newest, last): (&str, &str), when: &str) {
  let turn = turns(screen).last().expect("a turn is shown").clone();
  let shown = screen.text(&turn);
  assert!(
    shown.contains(newest),
    "{when}: the newest turn is built at the chat's end"
  );
  assert!(shown.contains(last), "{when}: the newest turn's last line is built");
  let gap = newest_foot_gap(screen);
  assert!(
    gap.abs() < 1.0,
    "{when}: the chat is at the newest turn's end, {gap} under it"
  );
  assert!(!screen.exists("conversation-jump"), "{when}: the chat follows its end");
}

/// Keeps a long Markdown reply tagged `part` in the newest conversation, as its agent does while it answers, and
/// checks the chat shows its end once it is drawn.
fn reply(screen: &mut Orchestrator, part: &str) {
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let model = ModelId::new("haiku").expect("model");
  screen
    .db()
    .add_chat_reply(chat, (&markdown(part), &model), Timestamp::now())
    .expect("kept");
  let root = screen.root.clone();
  screen
    .window
    .launch
    .services()
    .expect("the session started")
    .runners
    .chat_changed(&root);
  let heading = format!("{part} heading");
  wait(screen, "the part", |screen| {
    screen.text("orchestrator-conversation").contains(&heading)
  });
  let last = format!("{part} last line.");
  at_end_of(screen, (&heading, &last), &format!("the {part} part landed"));
}

#[test]
fn orchestrator_tall_reply_a_long_markdown_answer_lands_at_its_end() {
  let mut screen = long_conversation(2);
  screen.send(&markdown("Tall"));
  wait(&mut screen, "the answer", |screen| {
    !screen.exists("orchestrator-thinking") && screen.text("orchestrator-conversation").contains("Noted: ## Tall")
  });
  at_end_of(&mut screen, ("Noted: ## Tall", "Tall last line."), "the answer landed");
}

#[test]
fn orchestrator_tall_reply_an_answer_growing_in_parts_stays_at_its_end() {
  let mut screen = long_conversation(2);
  screen.send("Keep thinking about the queue.");
  wait(&mut screen, "the start of the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Let me look at the queue.")
  });
  at_end_of(
    &mut screen,
    ("Let me look at the queue.", "Orchestrator is thinking"),
    "the answer started",
  );
  for part in ["First", "Second"] {
    reply(&mut screen, part);
  }
}

/// The person's two reports together: a long text pasted and sent becomes *Pasted text.txt*; while the Orchestrator
/// thinks, the person opens the file's text and turns the wheel in it; then the answer comes, and the chat shows its
/// end, not its heading.
#[test]
fn orchestrator_tall_reply_after_a_wheel_in_the_opened_text_lands_at_its_end() {
  let mut screen = long_conversation(2);
  paste_long_text(&mut screen);
  screen.send("Keep thinking about the queue.");
  let mut sent = None;
  wait(&mut screen, "the answer to start", |screen| {
    sent = sent_message(screen);
    sent.is_some()
      && screen
        .text("orchestrator-conversation")
        .contains("Let me look at the queue.")
  });
  let (message, chip) = sent.expect("the sent message");
  screen.click(&chip);
  let text = format!("{message}-attachment-preview-text-box");
  wait(&mut screen, "the opened text", |screen| screen.exists(&text));
  wheel_over(&mut screen, &text, -120.0);
  wheel_over(&mut screen, &text, 60.0);
  reply(&mut screen, "Later");
}
