//! `orchestrator-leading` and `orchestrator-held-budget`, `-held-plan`, `-held-plan-unnamed`: the `orchestrator`
//! scenario with a conversation, the one the Orchestrator opens on, that leads the module *Webhooks v2* to the end and
//! follows *Search index*, so a verifier reaches what the board `Orchestrator autonomy · frames to approve` draws
//! (`L01` `QMwUB`, `Parts · Lines` `e6nsBS`, `Parts · Before you leave` `k0jNa`, `Parts · An event whose turn failed
//! or is held` `tkjdw`). *Let the Orchestrator drive* is on, and the Orchestrator runs on the project's Codex
//! connection, which `tools/qa-app.ps1 -FakeAgents` points at its fake: the turns the follower starts there are
//! answered by the fake, never by a CLI of the person's. The person's `/watch-module` whose order begins with
//! *Lead to the end* and its line are kept, with the subscriptions they made; *Before you leave* reads the copy as
//! it is, and the app's follower raises *Work can start in Webhooks v2* for the module's drafts by itself.
//!
//! Three of the module's tasks have an agent that asked a question, each an event of the conversation:
//!
//! - `orchestrator-leading`: their turns failed, each after ten minutes: once (*trying again* in a minute, *2 of 4*,
//!   which the follower then starts: *Orchestrator is thinking · 2 of 4*), three times (*4 of 4* in a quarter of an
//!   hour) and four times (*Left to you after 4 tries*, with **Try again**).
//! - the held ones: they have no turn yet, and nothing starts one: today's budget is used, or the account the
//!   Orchestrator runs on is at its plan limit, with the reset its CLI named or with none.
//!
//! `orchestrator-ceilings` is `orchestrator-leading` as `L02 · /watching` `HpcQJ` and `Parts · Ceilings` draw it
//! (OA-10): Webhooks v2 is led within a budget of 40M tokens of which its conversation's turns spent 12M, one more of
//! its tasks runs for the second time (*run again 2 of 3*), and the conversation ends with the person's `/watching`,
//! so its answer shows as the conversation opens: the mode control and *Stop leading* on Webhooks v2's row, its
//! ceilings row with *Change…*, and Search index followed beside it.

use orchester_core::{
  connection::AgentConnection,
  id::{AgentAccountId, ChatId, ChatMessageId},
  limits::ProjectLimits,
  orchestrator::{
    Ceilings, Chat, ChatEvent, OrchestratorSettings, PlanHold, Subscription, TurnFailure, TurnState, WatchMode,
    watch::{NamedTarget, WatchCommand, asked_to_lead},
  },
  run::TokenUsage,
  store::{ConnectionStore, SettingsStore},
  task::organization::ChangeActor,
  tokens::TokenBudget,
};

use super::{
  super::{
    CodexPlan,
    accounts::CODEX_MODELS,
    keep, later,
    live::{self, Ran},
    model, project,
  },
  module, watched,
};
use crate::kontur::{
  Kontur, SeedError,
  catalogue::Catalogue,
  shapes::{self, Shape},
};

/// The conversation that leads Webhooks v2 to the end.
pub const LEADING: &str = "Webhooks v2 and Search index";
/// The order of the command that led Webhooks v2 to the end, as the person wrote it.
const ORDER: &str = "Lead to the end: answer agents from the brief and the docs. Ask me before anything outside the \
                     module is touched.";
/// The day's budget of the scenario whose budget is used, and what its turns spent today.
const BUDGET: u64 = 20_000_000;
const SPENT: TokenUsage = TokenUsage {
  input: 19_400_000,
  cached_input: 0,
  output: 700_000,
};

/// The budget of the subscription `orchestrator-ceilings` leads Webhooks v2 within, and what its conversation's
/// turns spent of it.
const LEAD_BUDGET: u64 = 40_000_000;
const LEAD_SPENT: TokenUsage = TokenUsage {
  input: 11_600_000,
  cached_input: 0,
  output: 400_000,
};

/// What holds the turns Orchester starts by itself.
#[derive(Clone, Copy)]
enum Holds {
  Nothing,
  Budget,
  /// The account's plan limit, its reset named by its CLI or not.
  PlanLimit {
    named: bool,
  },
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  leading(kontur, Holds::Nothing).map(|_| ())
}

pub fn held_budget(kontur: &mut Kontur) -> Result<(), SeedError> {
  leading(kontur, Holds::Budget).map(|_| ())
}

pub fn held_plan(kontur: &mut Kontur) -> Result<(), SeedError> {
  leading(kontur, Holds::PlanLimit { named: true }).map(|_| ())
}

pub fn held_plan_unnamed(kontur: &mut Kontur) -> Result<(), SeedError> {
  leading(kontur, Holds::PlanLimit { named: false }).map(|_| ())
}

/// `orchestrator-ceilings`: [`seed`], Webhooks v2 led within a budget part of which is spent, a task of it run again,
/// and the person's `/watching` last in the conversation.
pub fn ceilings(kontur: &mut Kontur) -> Result<(), SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let led = leading(kontur, Holds::Nothing)?;
  let budget = TokenBudget::new(LEAD_BUDGET).map_err(|error| invalid(&error))?;
  let default = Ceilings::default();
  let within = Ceilings::new(
    (default.runs_of_task(), default.returns_of_run()),
    (default.raises_of_run(), default.raise_percent()),
    Some(budget),
  )
  .map_err(|error| invalid(&error))?;
  let lead = WatchMode::Lead { ceilings: within };
  if kontur
    .db
    .set_subscription_mode(led.chat.id(), led.webhooks.target, lead)?
    .is_none()
  {
    return Err(SeedError::Invalid("Webhooks v2 is not led".to_owned()));
  }
  let account = account(kontur)?;
  let turn = (led.chat.id(), ChatMessageId::generate());
  kontur
    .db
    .record_chat_usage(turn, (account, LEAD_SPENT), kontur.ago(25))?;
  run_again(kontur, &led)?;
  let asked = ChatEvent::WatchCommand {
    command: WatchCommand::Watching,
  };
  let at = kontur.ago(1);
  kontur.db.keep_watch_entries(&led.chat, &[asked], at)?;
  Ok(())
}

/// A task of the module `led` leads whose first run failed and whose second is at work: *run again 2 of 3*.
fn run_again(kontur: &mut Kontur, led: &Led) -> Result<(), SeedError> {
  let orchester_core::orchestrator::WatchTarget::Module(module) = led.webhooks.target else {
    return Err(SeedError::Invalid("Webhooks v2 is a module".to_owned()));
  };
  let pipeline = Some(&led.catalogue.feature_delivery);
  let described = (
    "Replay dead letters",
    "Deliver a dead-lettered webhook again on request.",
    &["A replayed delivery is signed again."][..],
  );
  let (made, failed, again) = (kontur.ago(4 * 60), kontur.ago(3 * 60), kontur.ago(35));
  let mut task = kontur.task(described, pipeline, made)?;
  shapes::apply(kontur, &mut task, pipeline, Shape::Failed("implement"), failed)?;
  let mut task = kontur.find(&task.key().to_string())?;
  shapes::apply(kontur, &mut task, pipeline, Shape::Running("implement"), again)?;
  kontur
    .db
    .set_task_module(task.id(), Some(module), ChangeActor::Person, made)?;
  Ok(())
}

/// The conversation that leads Webhooks v2 to the end, the module, and the project's published resources.
struct Led {
  chat: Chat,
  webhooks: NamedTarget,
  catalogue: Catalogue,
}

fn leading(kontur: &mut Kontur, holds: Holds) -> Result<Led, SeedError> {
  let (sign_ins, catalogue) = project(kontur, CodexPlan::Used)?;
  let on_codex = OrchestratorSettings::new(sign_ins.codex, model(CODEX_MODELS[0])?, None);
  kontur.db.publish_orchestrator(&on_codex, kontur.ago(3 * 24 * 60))?;
  kontur.db.set_orchestrator_drives(true)?;
  let (webhooks, _) = module(
    kontur,
    &catalogue,
    "Webhooks v2",
    &["Dead-letter queue", "Delivery metrics"],
  )?;
  let (search, _) = module(kontur, &catalogue, "Search index", &["Index task briefs"])?;
  let asking = asking(kontur, &catalogue, &webhooks)?;
  let start = kontur.ago(40);
  let chat = Chat::named(ChatId::generate(), LEADING, start);
  watched(kontur, &chat, (vec![webhooks.clone()], ORDER), start)?;
  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  let order = asked_to_lead(ORDER).unwrap_or(ORDER);
  let led = Subscription::new(chat.id(), webhooks.target, order, start).in_mode(lead);
  kontur.db.start_subscription(&led)?;
  let then = later(start, 30);
  watched(kontur, &chat, (vec![search.clone()], ""), then)?;
  kontur
    .db
    .start_subscription(&Subscription::new(chat.id(), search.target, "", then))?;
  let mut events = Vec::new();
  for (ran, minutes) in asking.iter().zip([30, 20, 10]) {
    let mut kept = live::asked(kontur, ran)?;
    kept.wait.automatic = true;
    events.push(keep(kontur, &chat, &kept, kontur.ago(minutes))?);
  }
  match holds {
    Holds::Nothing => {
      // Oldest first: left to the person, the last try to come, the second try to come.
      for (seq, failures) in events.into_iter().zip([4, 3, 1]) {
        fail(kontur, (chat.id(), seq), failures)?;
      }
    }
    Holds::Budget => use_up_the_budget(kontur)?,
    Holds::PlanLimit { named } => {
      let now = kontur.ago(0);
      let limit = PlanHold {
        since: now,
        resets: named.then(|| later(now, 160 * 60)),
      };
      let account = account(kontur)?;
      kontur.db.hold_orchestrator(account, limit)?;
    }
  }
  Ok(Led {
    chat,
    webhooks,
    catalogue,
  })
}

/// Three tasks of `module` whose Implement agent asked a question and waits for the answer.
fn asking(kontur: &mut Kontur, catalogue: &Catalogue, module: &NamedTarget) -> Result<Vec<Ran>, SeedError> {
  let orchester_core::orchestrator::WatchTarget::Module(id) = module.target else {
    return Err(SeedError::Invalid("the events' tasks are in a module".to_owned()));
  };
  let titles = [
    (
      "Retry failed deliveries",
      "Retry a failed webhook delivery with backoff.",
    ),
    ("Sign every attempt", "Sign each attempt again, not only the first."),
    ("Delivery log export", "Export the delivery log as CSV."),
  ];
  let mut asking = Vec::new();
  for (index, described) in (1_i64..).zip(titles) {
    let at = kontur.ago(60 * index);
    let ran = live::task_in(kontur, catalogue, described, (Shape::Question("implement"), at))?;
    kontur
      .db
      .set_task_module(ran.task.id(), Some(id), ChangeActor::Person, at)?;
    asking.push(ran);
  }
  Ok(asking)
}

/// The turns the wait `seq` of `chat` started by itself, `failures` of them, each ended after ten minutes; the last
/// one just now, so its next try is still to come.
fn fail(kontur: &mut Kontur, (chat, seq): (ChatId, i64), failures: i64) -> Result<(), SeedError> {
  let failed = TurnState::Failed(TurnFailure::TimeLimit { minutes: 10 });
  for turn in 0..failures {
    let message = ChatMessageId::generate();
    let given = if turn == 0 {
      kontur.db.start_wait_turn((chat, seq), message)?
    } else {
      kontur.db.retry_wait_turn((chat, seq), message)?
    };
    if !given {
      return Err(SeedError::Invalid(format!("wait {seq} did not get its turn")));
    }
    let ended = kontur.ago((failures - 1 - turn) * 20);
    kontur.db.end_chat_turn(message, &failed, ended)?;
  }
  Ok(())
}

/// The project's daily budget is [`BUDGET`], and the Orchestrator's turns spent more than that today.
fn use_up_the_budget(kontur: &mut Kontur) -> Result<(), SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let budget = TokenBudget::new(BUDGET).map_err(|error| invalid(&error))?;
  let limits = ProjectLimits::new(None, budget, None).map_err(|error| invalid(&error))?;
  kontur.db.save_project_limits(&limits)?;
  let account = account(kontur)?;
  let turn = (ChatId::generate(), ChatMessageId::generate());
  kontur.db.record_chat_usage(turn, (account, SPENT), kontur.ago(0))?;
  Ok(())
}

/// The account the Orchestrator's published connection runs on.
fn account(kontur: &Kontur) -> Result<AgentAccountId, SeedError> {
  let missing = |what: &str| SeedError::Invalid(format!("the Orchestrator has no {what}"));
  let settings = kontur.db.orchestrator_settings()?.ok_or_else(|| missing("settings"))?;
  let connection = kontur
    .db
    .connection(settings.connection())?
    .ok_or_else(|| missing("connection"))?;
  let connection = AgentConnection::try_from(connection).map_err(|_| missing("agent connection"))?;
  connection
    .account()
    .id(connection.provider())
    .ok_or_else(|| missing("account"))
}
