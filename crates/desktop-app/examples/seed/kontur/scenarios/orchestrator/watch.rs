//! `orchestrator-watch`: the `orchestrator` scenario with the conversations' watch entries the app writes itself
//! (ORC-777; `04 · Leading two modules` `OKtX2` and `Parts · Lines` `avDZO` a–d in `Orchestrator watch · redesign`).
//! *Webhooks v2 and Search index*, the one the Orchestrator opens on, holds the person's `/watch-module` of both
//! modules with its order and the subscription line (04). *Billing*, in History, holds its own command, then *Billing
//! is now led in «…»* (a), *KON-… is now led in «Release checklist»* (b), *Stopped watching Search index* with two
//! tasks left open (c), and *Every task of Mobile release is Done* with the summary turn it started (d). *Release
//! checklist*, in History, holds a `/watch-task` without an order. *Search index, asked in words*, in History, holds
//! the lines the Orchestrator's watch tools kept at the person's request (ORC-893). The modules hold drafts, so the
//! subscription line counts their tasks.

use orchester_core::{
  id::{ChatId, ModuleId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat, ChatEvent, Subscription, WatchTarget,
    watch::{AtRequest, EndReason, NamedTarget, OpenTask, OpenWork, WatchCommand, WatchLine, WatchSummary},
  },
  task::{Task, organization::ChangeActor},
  time::Timestamp,
};

use super::{Part, later, live, set_up, wait_turn};
use crate::kontur::{Kontur, SeedError, catalogue::Catalogue};

/// The order typed after the chips of the command of 04 (`oUZqo`).
const ORDER: &str = "Answer agents from the brief and the docs. New tasks inside these modules are fine. Ask me before merging into the \
   module branch.";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = set_up(kontur)?;
  let modules = Modules::seed(kontur, &catalogue)?;
  let leading = Chat::named(ChatId::generate(), "Webhooks v2 and Search index", kontur.ago(20));
  let checklist = Chat::named(ChatId::generate(), "Release checklist", kontur.ago(90));
  let task = NamedTarget {
    target: WatchTarget::Task(modules.billing_tasks[0].id()),
    name: Some(modules.billing_tasks[0].key().to_string()),
  };
  watched(kontur, &checklist, (vec![task.clone()], ""), checklist.started())?;
  asked(kontur, &modules.search)?;
  billing(kontur, &modules, (&leading, &checklist, task))?;
  let both = vec![modules.webhooks, modules.search];
  watched(kontur, &leading, (both, ORDER), leading.started())
}

/// The modules the conversations watch, with drafts in them, and the drafts of *Billing*.
struct Modules {
  webhooks: NamedTarget,
  search: NamedTarget,
  billing: NamedTarget,
  billing_tasks: Vec<Task>,
}

impl Modules {
  fn seed(kontur: &mut Kontur, catalogue: &Catalogue) -> Result<Self, SeedError> {
    let (webhooks, _) = module(
      kontur,
      catalogue,
      "Webhooks v2",
      &["Retry failed deliveries", "Shared rate limiter", "Sign every attempt"],
    )?;
    let (search, _) = module(kontur, catalogue, "Search index", &["Index task briefs", "Rank by key"])?;
    let (billing, billing_tasks) = module(kontur, catalogue, "Billing", &["Invoice numbering", "Refund receipts"])?;
    Ok(Self {
      webhooks,
      search,
      billing,
      billing_tasks,
    })
  }
}

/// Module `name`, open, with a draft of each of `titles`.
fn module(
  kontur: &mut Kontur,
  catalogue: &Catalogue,
  name: &str,
  titles: &[&str],
) -> Result<(NamedTarget, Vec<Task>), SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let at = kontur.ago(6 * 60);
  let details = ModuleDetails::new(
    Name::new(name).map_err(|error| invalid(&error))?,
    "The module's tasks, watched by a conversation.",
    (None, None),
    ModuleState::Open,
  )
  .map_err(|error| invalid(&error))?;
  let module = kontur.db.create_module(details, at)?;
  let mut tasks = Vec::new();
  for title in titles {
    let task = live::draft(kontur, catalogue, (title, "Part of the module's work."), at)?;
    kontur
      .db
      .set_task_module(task.id(), Some(module.id()), ChangeActor::Person, at)?;
    tasks.push(task);
  }
  let named = NamedTarget {
    target: WatchTarget::Module(module.id()),
    name: Some(name.to_owned()),
  };
  Ok((named, tasks))
}

/// The person's /watch of `targets` with `order` in `chat` at `at`, and the subscription line it wrote.
fn watched(
  kontur: &mut Kontur,
  chat: &Chat,
  (targets, order): (Vec<NamedTarget>, &str),
  at: Timestamp,
) -> Result<(), SeedError> {
  let command = WatchCommand::Watch {
    targets: targets.clone(),
    order: order.to_owned(),
  };
  let line = WatchLine::Watching {
    started: targets,
    already_here: Vec::new(),
    led_elsewhere: Vec::new(),
    gone: Vec::new(),
    at_request: None,
  };
  let entries = [ChatEvent::WatchCommand { command }, ChatEvent::Watch { line }];
  kontur.db.keep_watch_entries(chat, &entries, at)?;
  Ok(())
}

/// *Search index, asked in words*, in History (ORC-893): the person asked the Orchestrator to follow Search index and
/// later to stop, and its `watch` and `unwatch` tools kept only the lines, each *at your request*.
fn asked(kontur: &mut Kontur, search: &NamedTarget) -> Result<(), SeedError> {
  let chat = Chat::named(ChatId::generate(), "Search index, asked in words", kontur.ago(80));
  let at_request = || Some(AtRequest::default());
  let leading = WatchLine::Watching {
    started: vec![search.clone()],
    already_here: Vec::new(),
    led_elsewhere: Vec::new(),
    gone: Vec::new(),
    at_request: at_request(),
  };
  kontur
    .db
    .keep_watch_entries(&chat, &[ChatEvent::Watch { line: leading }], chat.started())?;
  let stopped = WatchLine::Stopped {
    targets: vec![search.clone()],
    left_open: Vec::new(),
    at_request: at_request(),
  };
  let at = kontur.ago(70);
  kontur
    .db
    .keep_watch_entries(&chat, &[ChatEvent::Watch { line: stopped }], at)?;
  Ok(())
}

/// *Billing*: its /watch of Billing and Search index, then the lines a–d of `avDZO`.
fn billing(
  kontur: &mut Kontur,
  modules: &Modules,
  (leading, checklist, task): (&Chat, &Chat, NamedTarget),
) -> Result<(), SeedError> {
  let chat = Chat::named(ChatId::generate(), "Billing", kontur.ago(120));
  let both = vec![modules.billing.clone(), modules.search.clone()];
  watched(kontur, &chat, (both, "Keep the invoices job green."), chat.started())?;
  let line = |line| [ChatEvent::Watch { line }];
  let taken = WatchLine::NowLedIn {
    target: modules.billing.clone(),
    chat: leading.id(),
    title: leading.title().to_owned(),
    taken_over: true,
  };
  let at = kontur.ago(110);
  kontur.db.keep_watch_entries(&chat, &line(taken), at)?;
  let task_taken = WatchLine::NowLedIn {
    target: task,
    chat: checklist.id(),
    title: checklist.title().to_owned(),
    taken_over: false,
  };
  let at = kontur.ago(90);
  kontur.db.keep_watch_entries(&chat, &line(task_taken), at)?;
  let left_open = |task: &Task, work| OpenTask {
    task: task.key().to_string(),
    work,
  };
  let stopped = WatchLine::Stopped {
    targets: vec![modules.search.clone()],
    left_open: vec![
      left_open(&modules.billing_tasks[1], OpenWork::StoppedAtLimit),
      left_open(&modules.billing_tasks[0], OpenWork::Queued),
    ],
    at_request: None,
  };
  let stop = [
    ChatEvent::WatchCommand {
      command: WatchCommand::Unwatch {
        targets: vec![modules.search.clone()],
      },
    },
    ChatEvent::Watch { line: stopped },
  ];
  let at = kontur.ago(60);
  kontur.db.keep_watch_entries(&chat, &stop, at)?;
  let at = kontur.ago(30);
  ended(kontur, &chat, at)
}

/// *Mobile release*, a module `chat` watched, ended by itself at `at` with every task Done, and the turn that wrote its
/// summary.
fn ended(kontur: &mut Kontur, chat: &Chat, at: Timestamp) -> Result<(), SeedError> {
  let module = ModuleId::generate();
  let since = kontur.ago(100);
  let subscription = Subscription::new(chat.id(), WatchTarget::Module(module), "", since);
  kontur.db.start_subscription(&subscription)?;
  let summary = WatchSummary {
    target: subscription.target(),
    name: "Mobile release".to_owned(),
    title: None,
    reason: EndReason::ModuleClosed { done: 4, canceled: 0 },
    order: None,
    since: subscription.since(),
    link: "/modules".to_owned(),
    automatic: true,
  };
  if !kontur.db.end_subscription_by_itself(&subscription, &summary, at)? {
    return Err(SeedError::Invalid(
      "Mobile release's subscription did not end".to_owned(),
    ));
  }
  let seq = kontur
    .db
    .chat_entries(chat.id())?
    .into_iter()
    .map(|entry| entry.seq)
    .max()
    .ok_or_else(|| SeedError::Invalid("the summary's entry".to_owned()))?;
  wait_turn(
    kontur,
    (chat.id(), seq),
    later(at, 10),
    vec![Part::Reply(
      "Mobile release is done: its four tasks merged into the release branch, and nothing of it waits for anyone.",
    )],
  )
}
