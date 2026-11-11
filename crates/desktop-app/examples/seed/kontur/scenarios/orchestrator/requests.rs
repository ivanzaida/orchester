//! `orchestrator-requests`: the `orchestrator` scenario with a conversation, *Tool requests of Webhooks v2* (open it
//! from History: the `orchestrator` scenario's own conversations come later), of
//! agents' tool requests in each state the chat shows (2026-11-11; `Orchestrator WIP` `mPw5l`, states `ICzi3`): a
//! the Orchestrator reads one (its wait keeps no turn yet, and *Let the Orchestrator drive* stays off, so none
//! starts); b it allowed one once, with its reason; c it denied one, with the words it told the agent; d it was not
//! sure, so the request waits for the person with its recommendation and the card; e the person answered one in the
//! chat. Each request is a real one of a run of Feature delivery, kept under the follower's own key, so the
//! conversation reads where each stands.

use orchester_core::{
  id::ChatId,
  orchestrator::{ActionOutcome, AgentWait, Chat, ChatAction, ToolAnswer, TurnState, WaitKind},
  run::{DecidedBy, DenialNote, PendingToolApproval, ToolApprovalDecision},
  time::Timestamp,
};

use super::{
  Part, act, keep, later,
  live::{self, Kept, Ran},
  set_up, turn, wait_turn,
};
use crate::kontur::{Kontur, SeedError, catalogue::Catalogue, shapes::Shape};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = set_up(kontur)?;
  // After the `orchestrator` scenario's own conversations, so that this one is the latest.
  let start = kontur.ago(100);
  let chat = Chat::named(ChatId::generate(), "Tool requests of Webhooks v2", start);
  turn(
    kontur,
    &chat,
    (
      "Look after the agents' tool requests: allow what plainly serves the task, ask me otherwise.",
      start,
    ),
    vec![Part::Reply(
      "I'll read each request: allow it once when it plainly serves the task and is safe, deny it when it is plainly \
       wrong, and ask you otherwise.",
    )],
    &TurnState::Answered,
  )?;
  let at = |minutes: i64| later(start, minutes * 60);
  let reading = request(kontur, &catalogue, "Retry failed deliveries", 90)?;
  let allowed = request(kontur, &catalogue, "Backoff schedule for retries", 85)?;
  let denied = request(kontur, &catalogue, "Keep the webhook payloads small", 80)?;
  let unsure = request(kontur, &catalogue, "Parse Retry-After headers", 75)?;
  let by_you = request(kontur, &catalogue, "Delivery log", 70)?;

  answered_itself(kontur, &chat, (&allowed, &denied), start)?;

  let (seq, _) = waits(kontur, &chat, &unsure, at(30))?;
  wait_turn(
    kontur,
    (chat.id(), seq),
    at(31),
    vec![Part::Reply(
      "Not sure: it adds a dependency the brief does not name, and it uses the network.\n\nRecommends Deny: the \
       retry schedule can be computed with what is already in Cargo.lock.",
    )],
  )?;

  let (seq, asked) = waits(kontur, &chat, &by_you, at(40))?;
  wait_turn(
    kontur,
    (chat.id(), seq),
    at(41),
    vec![Part::Reply(
      "Not sure: the brief names the delivery log, not this dependency.\n\nRecommends Allow once: it is a small, \
       well-known crate.",
    )],
  )?;
  decide(
    kontur,
    &asked,
    (DecidedBy::Person, ToolApprovalDecision::Approved, None),
    at(43),
  )?;

  // Kept last, without a turn: the Orchestrator reads it next.
  waits(kontur, &chat, &reading, at(50))?;
  Ok(())
}

/// The Orchestrator's own answers (b, c), in `chat` from `start` on: it allowed `allowed`'s request once, with its
/// reason, and denied `denied`'s, with the words it told the agent.
fn answered_itself(
  kontur: &mut Kontur,
  chat: &Chat,
  (allowed, denied): (&Ran, &Ran),
  start: Timestamp,
) -> Result<(), SeedError> {
  let at = |minutes: i64| later(start, minutes * 60);
  let (seq, asked) = waits(kontur, chat, allowed, at(10))?;
  decide(
    kontur,
    &asked,
    (DecidedBy::Automation, ToolApprovalDecision::Approved, None),
    at(11),
  )?;
  let answer = ToolAnswer {
    request: asked.id.clone(),
    asked: asked.action.clone(),
    allowed: true,
    reason: "the brief asks for the retry schedule, and it runs inside the run's worktree.".to_owned(),
    told: None,
  };
  wait_turn(kontur, (chat.id(), seq), at(11), vec![decided(allowed, answer)])?;

  let (seq, asked) = waits(kontur, chat, denied, at(20))?;
  let words = "Do not add serde: the brief keeps the dependencies as they are; use the JSON helpers the crate has.";
  decide(
    kontur,
    &asked,
    (DecidedBy::AutomationDeny, ToolApprovalDecision::Denied, Some(words)),
    at(21),
  )?;
  let answer = ToolAnswer {
    request: asked.id.clone(),
    asked: asked.action.clone(),
    allowed: false,
    reason: "it adds a dependency the brief does not name.".to_owned(),
    told: Some(words.to_owned()),
  };
  wait_turn(kontur, (chat.id(), seq), at(21), vec![decided(denied, answer)])?;

  Ok(())
}

/// A task `title` on Feature delivery whose Implement agent asks to run a command, its run started `minutes` ago.
fn request(kontur: &mut Kontur, catalogue: &Catalogue, title: &str, minutes: i64) -> Result<Ran, SeedError> {
  let at = kontur.ago(minutes);
  let brief = "Keep webhook deliveries going when a receiver fails for a while.";
  live::task_in(kontur, catalogue, (title, brief), (Shape::Approval("implement"), at))
}

/// The wait of `ran`'s request, kept in `chat` at `at` under the follower's key, to start a turn by itself: its entry
/// and the request.
fn waits(kontur: &mut Kontur, chat: &Chat, ran: &Ran, at: Timestamp) -> Result<(i64, PendingToolApproval), SeedError> {
  let asked = kontur
    .db
    .pending_tool_approvals(ran.run.id())?
    .into_iter()
    .next()
    .ok_or(SeedError::NotAtWork)?;
  let task = ran.task.key().to_string();
  let kept = Kept {
    wait: AgentWait {
      kind: WaitKind::Approval,
      title: ran.task.details().title.as_str().to_owned(),
      run: ran.run.id(),
      number: ran.run.number().get(),
      step: Some("Implement".to_owned()),
      place: None,
      others: 0,
      agent: Some(asked.agent.clone()),
      link: format!("/tasks/{task}/conversation"),
      task,
      automatic: true,
      continuing: None,
      merging: None,
    },
    key: format!("approval:{}:{}", ran.run.id(), asked.id),
  };
  Ok((keep(kontur, chat, &kept, at)?, asked))
}

/// `asked` answered by `by` as `decision` at `at`, with a denial's `words` for the agent.
fn decide(
  kontur: &mut Kontur,
  asked: &PendingToolApproval,
  (by, decision, words): (DecidedBy, ToolApprovalDecision, Option<&str>),
  at: Timestamp,
) -> Result<(), SeedError> {
  kontur.db.claim_tool_approval(asked.run, &asked.id, by, at)?;
  // A denial's words are kept with its claim, before it reaches the agent, as the tool keeps them.
  if let Some(words) = words {
    let note = DenialNote::new(words).map_err(|error| SeedError::Invalid(error.to_string()))?;
    kontur.db.record_denial_note(asked.run, &asked.id, &note)?;
  }
  kontur.db.delivered_tool_approval(asked.run, &asked.id, decision)?;
  Ok(())
}

/// The Orchestrator's own `decide_approval` of `ran`'s request, kept as its action.
fn decided(ran: &Ran, answer: ToolAnswer) -> Part {
  act(ChatAction {
    tool_answer: Some(answer),
    ..ChatAction::new("decide_approval", Some(ran.task.key().to_string()), ActionOutcome::Done)
  })
}
