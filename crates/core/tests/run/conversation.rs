//! The task conversation: a person's messages move from queued to delivered to acknowledged and
//! reach the agent they are for; an agent's question makes its step wait for the answer; a run that
//! succeeded is followed up in an attempt of its own and is succeeded again after it, whether the
//! follow-up passes or fails; messages reach prompts introduced by why they are read now. A message
//! sent now is for the agent at work; one its CLI did not take is kept and may be sent after the turn.

use orchester_core::{
  conversation::{
    AgentMessage, Arrival, Delivery, Message, MessageBody, MessageError, Recipient, Sender, Timing, Undelivered,
    follow_up_step, latest_question,
  },
  id::{InvocationId, ResourceId, RunId, StepExecutionId, TaskId},
  resource::InjectionEvent,
  run::{
    AttemptReason, AttemptStatus, CONTINUE_PROMPT, DeliveredMessages, MessageOccasion, PromptInput, Run, RunError,
    RunStatus, StepFailure, StepOutcome, StepResult, StepStatus, WaitReason,
  },
};

use crate::{
  fixture::{Logged, at, key, task},
  prompt::{Prompt, builder, step_with},
};

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("a message")
}

fn from_person(run: Option<RunId>, recipient: Recipient, text: &str) -> Message {
  Message::from_person(TaskId::generate(), run, recipient, body(text), at(40))
}

/// Every step of Feature delivery passed: the run succeeded.
fn succeeded() -> Logged {
  let mut logged = Logged::started();
  logged.through_plan();
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  logged
}

fn profile_of(logged: &Logged, step: &str) -> ResourceId {
  match logged.run.snapshot().step(&key(step)).map(|step| step.executor()) {
    Some(orchester_core::run::StepExecutor::Agent { profile, .. }) => profile.resource,
    _ => panic!("{step} is an agent step"),
  }
}

#[test]
fn conversation_a_message_says_something() {
  assert_eq!(MessageBody::new("  \n "), Err(MessageError::Blank));
  assert_eq!(body("  Keep email sign-in. ").as_str(), "Keep email sign-in.");
}

#[test]
fn conversation_a_message_reaches_the_agent_it_is_for_in_its_run() {
  let run = RunId::generate();
  let builder = ResourceId::generate();
  let reviewer = ResourceId::generate();
  let routed = from_person(Some(run), Recipient::Orchestrator, "Also cover cancel.");
  assert!(routed.reaches(run, builder) && routed.reaches(run, reviewer));
  assert!(!routed.reaches(RunId::generate(), builder), "written for another run");
  let direct = from_person(Some(run), Recipient::Agent(reviewer), "Check the tests.");
  assert!(direct.reaches(run, reviewer) && !direct.reaches(run, builder));
  // Written before the task had a run: its next run takes it.
  let early = from_person(None, Recipient::Orchestrator, "Keep email sign-in.");
  assert!(early.reaches(run, builder));
}

#[test]
fn conversation_delivery_moves_from_queued_to_delivered_to_acknowledged() {
  let run = RunId::generate();
  let (execution, invocation) = (StepExecutionId::generate(), InvocationId::generate());
  let mut message = from_person(None, Recipient::Orchestrator, "Keep email sign-in.");
  assert_eq!(message.delivery(), Some(Delivery::Queued));
  assert_eq!(message.acknowledge(at(41)), Err(MessageError::NotDelivered));
  message
    .deliver(run, (execution, invocation), at(41), Arrival::Prompt)
    .expect("delivered");
  assert_eq!(
    message.run(),
    Some(run),
    "a message written before the run belongs to the run it went to"
  );
  assert_eq!(
    message.delivery(),
    Some(Delivery::Delivered {
      execution,
      invocation,
      at: at(41),
      arrival: Arrival::Prompt,
    })
  );
  assert!(
    !message.reaches(run, ResourceId::generate()),
    "a delivered message is not delivered again"
  );
  assert_eq!(
    message.deliver(run, (execution, invocation), at(42), Arrival::Prompt),
    Err(MessageError::AlreadyDelivered)
  );
  message.acknowledge(at(43)).expect("acknowledged");
  assert_eq!(
    message.delivery(),
    Some(Delivery::Acknowledged {
      execution,
      invocation,
      delivered_at: at(41),
      at: at(43),
      arrival: Arrival::Prompt,
    })
  );
  let mut other = from_person(Some(RunId::generate()), Recipient::Orchestrator, "Hi.");
  assert_eq!(
    other.deliver(run, (execution, invocation), at(41), Arrival::Prompt),
    Err(MessageError::OtherRun)
  );
  let mut reply = Message::from_agent(
    TaskId::generate(),
    run,
    (execution, invocation),
    AgentMessage::Reply,
    body("Kept."),
    at(44),
  );
  assert_eq!(reply.delivery(), None);
  assert_eq!(reply.acknowledge(at(45)), Err(MessageError::NotFromAPerson));
}

#[test]
fn conversation_a_message_sent_now_is_for_the_agent_at_work() {
  let run = RunId::generate();
  let (builder, reviewer) = (ResourceId::generate(), ResourceId::generate());
  let (execution, invocation) = (StepExecutionId::generate(), InvocationId::generate());
  let after_turn = from_person(Some(run), Recipient::Agent(builder), "Also cover cancel.");
  assert_eq!(after_turn.timing(), Timing::AfterTurn);
  assert!(
    !after_turn.steers(run, builder),
    "sent after the turn, it waits for the turn's end"
  );
  let mut now = from_person(Some(run), Recipient::Agent(builder), "Stop editing the parser.")
    .sent(Timing::Now)
    .expect("a person's message");
  assert!(now.steers(run, builder) && !now.steers(run, reviewer));
  now
    .deliver(run, (execution, invocation), at(41), Arrival::MidTurn)
    .expect("delivered mid-turn");
  assert!(!now.steers(run, builder), "taken once");
  now.acknowledge(at(42)).expect("acknowledged");
  assert_eq!(now.delivery().and_then(Delivery::arrival), Some(Arrival::MidTurn));
  let reply = Message::from_agent(
    TaskId::generate(),
    run,
    (execution, invocation),
    AgentMessage::Reply,
    body("Kept."),
    at(44),
  );
  assert_eq!(reply.sent(Timing::Now).err(), Some(MessageError::NotFromAPerson));
}

#[test]
fn conversation_a_message_the_cli_did_not_take_is_kept_and_sent_after_the_turn() {
  let run = RunId::generate();
  let builder = ResourceId::generate();
  let execution = StepExecutionId::generate();
  let mut message = from_person(None, Recipient::Agent(builder), "Use the new API.")
    .sent(Timing::Now)
    .expect("a person's message");
  assert_eq!(message.send_after_turn(), Err(MessageError::NotUndelivered));
  message
    .not_delivered(run, execution, at(41), Undelivered::Refused)
    .expect("not delivered");
  assert_eq!(message.run(), Some(run));
  assert_eq!(
    message.delivery(),
    Some(Delivery::NotDelivered {
      execution,
      at: at(41),
      reason: Undelivered::Refused
    })
  );
  assert!(!message.reaches(run, builder), "it goes no further on its own");
  assert_eq!(
    message.not_delivered(run, execution, at(42), Undelivered::Unwritable),
    Err(MessageError::AlreadyDelivered)
  );
  message.send_after_turn().expect("sent after the turn");
  assert_eq!(message.timing(), Timing::AfterTurn);
  assert!(message.reaches(run, builder) && !message.steers(run, builder));
}

#[test]
fn conversation_a_stored_message_is_checked() {
  let sender = Sender::Agent {
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    kind: AgentMessage::Question,
  };
  let restored = Message::restore(
    orchester_core::id::MessageId::generate(),
    TaskId::generate(),
    None,
    sender,
    body("Which provider?"),
    at(1),
  );
  assert_eq!(restored, Err(MessageError::WithoutRun));
}

#[test]
fn conversation_a_question_is_a_step_result() {
  let result = StepResult::parse(r#"{"outcome":"question","summary":"Keep email sign-in?"}"#).expect("a result");
  assert_eq!(result.outcome, StepOutcome::Question);
  assert_eq!(result.summary, "Keep email sign-in?");
  assert!(orchester_core::run::STEP_RESULT_SCHEMA.contains(r#""question""#));
  // Prose that asks something is no structured result.
  assert_eq!(StepResult::parse("Should I keep email sign-in?"), None);
}

#[test]
fn conversation_a_question_makes_its_step_wait_for_the_answer() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(20)));
  logged.apply(|run| run.wait_step(research, WaitReason::Message, at(21)));
  assert_eq!(logged.run.awaiting_answer(), [research]);
  assert!(logged.run.waits_for_a_person());
  assert!(logged.run.running_executions().is_empty());
  let question = Message::from_agent(
    logged.run.task(),
    logged.run.id(),
    (research, InvocationId::generate()),
    AgentMessage::Question,
    body("Which OAuth provider?"),
    at(21),
  );
  let messages = vec![question.clone()];
  assert!(question.awaits_answer(&logged.run, &messages));
  assert_eq!(latest_question(&messages, research), Some(&question));
  logged.apply(|run| run.resume_step(research, at(30)));
  assert_eq!(logged.run.running_executions(), [research]);
  assert!(
    !question.awaits_answer(&logged.run, &messages),
    "answered once the step works again"
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn conversation_a_follow_up_asks_the_last_agent_step_and_the_run_is_ready_again() {
  let mut logged = succeeded();
  let passed = logged.exec("pr");
  assert_eq!(logged.run.follow_up_step(None), Some(key("pr")));
  let reviewer = profile_of(&logged, "review");
  assert_eq!(logged.run.follow_up_step(Some(reviewer)), Some(key("review")));
  logged.apply(|run| run.follow_up(&key("pr"), at(50)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  let attempt = logged.run.current_attempt().expect("an attempt");
  assert_eq!(attempt.reason(), &AttemptReason::FollowUp { step: key("pr") });
  assert_eq!(attempt.executions().len(), 1);
  // The attempt before it stays succeeded.
  let before = &logged.run.attempts()[logged.run.attempts().len() - 2];
  assert_eq!(before.status(), AttemptStatus::Succeeded);
  let follow_up = logged.exec("pr");
  assert_ne!(follow_up, passed);
  assert_eq!(logged.run.followed_up(follow_up), Some(passed));
  assert_eq!(logged.run.followed_up(passed), None);
  assert_eq!(logged.run.ready_executions(), [follow_up]);
  logged.pass("pr");
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(
    logged.run.current_attempt().map(|attempt| attempt.status()),
    Some(AttemptStatus::Succeeded)
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn conversation_a_failed_follow_up_leaves_the_run_as_it_was() {
  let mut logged = succeeded();
  let passed = logged.exec("implement");
  logged.apply(|run| run.follow_up(&key("implement"), at(50)));
  let follow_up = logged.exec("implement");
  logged.apply(|run| run.start_step(follow_up, at(51)));
  logged.apply(|run| run.fail_step(follow_up, StepFailure::AgentFailed, at(52)));
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(
    logged.run.current_attempt().map(|attempt| attempt.status()),
    Some(AttemptStatus::Failed)
  );
  // The step's result is still the one that passed.
  assert_eq!(logged.exec("implement"), passed);
  assert_eq!(
    logged
      .run
      .latest_execution(&key("implement"))
      .map(|execution| execution.status()),
    Some(StepStatus::Passed)
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  // It can be asked again.
  logged.apply(|run| run.follow_up(&key("implement"), at(60)));
  assert_eq!(logged.run.followed_up(logged.exec("implement")), Some(passed));
}

#[test]
fn conversation_a_follow_up_of_a_gated_step_passes_without_a_plan() {
  let mut logged = succeeded();
  logged.apply(|run| run.follow_up(&key("plan"), at(50)));
  let follow_up = logged.exec("plan");
  assert_eq!(logged.run.is_gated(follow_up), Ok(false));
  logged.pass("plan");
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
}

#[test]
fn conversation_only_a_succeeded_run_is_followed_up() {
  let mut logged = Logged::started();
  assert!(matches!(
    logged.run.follow_up(&key("research"), at(50)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  let mut succeeded = succeeded();
  assert!(matches!(
    succeeded.run.follow_up(&key("missing"), at(50)),
    Err(RunError::InvalidAttempt(_))
  ));
  logged.apply(|run| run.cancel(at(51)));
  assert!(matches!(
    logged.run.follow_up(&key("research"), at(52)),
    Err(RunError::InvalidRunTransition { .. })
  ));
}

#[test]
fn conversation_queued_messages_choose_the_step_a_follow_up_asks() {
  let logged = succeeded();
  let run = logged.run.id();
  let reviewer = profile_of(&logged, "review");
  let direct = from_person(Some(run), Recipient::Agent(reviewer), "Why this test?");
  assert_eq!(
    follow_up_step(&logged.run, std::slice::from_ref(&direct)),
    Some(key("review"))
  );
  let routed = from_person(Some(run), Recipient::Orchestrator, "Rename it.");
  assert_eq!(follow_up_step(&logged.run, &[routed, direct]), Some(key("pr")));
  let elsewhere = from_person(Some(RunId::generate()), Recipient::Orchestrator, "Hi.");
  assert_eq!(follow_up_step(&logged.run, &[elsewhere]), None);
}

fn resumed(prompt: &Prompt, delivered: DeliveredMessages<'_>) -> String {
  PromptInput {
    messages: Some(delivered),
    ..prompt.input(&[], &[], true)
  }
  .assemble(InjectionEvent::Resume)
  .expect("no variables")
  .text
}

#[test]
fn conversation_a_resumed_session_reads_messages_introduced_by_why() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let messages = [
    from_person(None, Recipient::Orchestrator, "Keep email sign-in."),
    from_person(None, Recipient::Orchestrator, "Also cover cancel."),
  ];
  let after_turn = resumed(
    &prompt,
    DeliveredMessages {
      occasion: MessageOccasion::AfterTurn,
      messages: &messages,
    },
  );
  assert!(
    after_turn.starts_with("While you worked on this step, the person wrote to you:"),
    "{after_turn}"
  );
  assert!(
    after_turn.contains("Keep email sign-in.\n\n---\n\nAlso cover cancel."),
    "{after_turn}"
  );
  assert!(!after_turn.contains(CONTINUE_PROMPT));
  let written = resumed(
    &prompt,
    DeliveredMessages {
      occasion: MessageOccasion::Written,
      messages: &messages[..1],
    },
  );
  assert!(written.starts_with(CONTINUE_PROMPT), "{written}");
  assert!(written.contains("While you were stopped, the person wrote to you:"));
  let answer = resumed(
    &prompt,
    DeliveredMessages {
      occasion: MessageOccasion::Answer {
        question: "Keep email sign-in?",
      },
      messages: &messages[..1],
    },
  );
  assert!(
    answer.starts_with("You asked the person:\n\nKeep email sign-in?\n\nThey answered:"),
    "{answer}"
  );
  assert!(answer.contains("Continue the step with their answer."));
  let follow_up = resumed(
    &prompt,
    DeliveredMessages {
      occasion: MessageOccasion::FollowUp,
      messages: &messages[..1],
    },
  );
  assert!(
    follow_up.starts_with("You completed this step and the run is ready for review."),
    "{follow_up}"
  );
  assert!(follow_up.contains("End with exactly one JSON object containing `outcome` and `summary`"));
}

#[test]
fn conversation_a_new_session_reads_messages_in_its_task_part() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let messages = [from_person(None, Recipient::Orchestrator, "Keep email sign-in.")];
  let text = PromptInput {
    messages: Some(DeliveredMessages {
      occasion: MessageOccasion::Written,
      messages: &messages,
    }),
    ..prompt.input(&[], &[], true)
  }
  .assemble(InjectionEvent::Start)
  .expect("no variables")
  .text;
  let heading = text.find("# Messages from the person").expect("the messages");
  assert!(text[heading..].contains("Keep email sign-in."));
  assert!(heading < text.find("# When you finish").expect("the result"), "{text}");
  assert!(
    text.contains("Use `question` when progress needs the person's decision"),
    "{text}"
  );
}

#[test]
fn conversation_messages_follow_the_changes_a_reviewer_requested() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let note = orchester_core::run::ReviewNote::new("Keep email sign-in.").expect("a note");
  let messages = [from_person(None, Recipient::Orchestrator, "And cover cancel.")];
  let text = PromptInput {
    revision: Some(orchester_core::run::PlanRevision {
      version: orchester_core::run::PlanVersion::FIRST,
      plan: "1. Replace sign-in.",
      note: &note,
    }),
    messages: Some(DeliveredMessages {
      occasion: MessageOccasion::AfterTurn,
      messages: &messages,
    }),
    ..prompt.input(&[], &[], true)
  }
  .assemble(InjectionEvent::Resume)
  .expect("no variables")
  .text;
  assert!(
    text.contains("What should change in plan v1:\n\nKeep email sign-in."),
    "{text}"
  );
  assert!(
    text.contains("The person also wrote to you:\n\nAnd cover cancel."),
    "{text}"
  );
}
