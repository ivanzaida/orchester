//! `/watch-task` and `/watch-module`: each target is claimed for the conversation (`Subscriptions::claim`); a new one
//! is kept at once, one the conversation leads already takes the command's order, and one another conversation leads
//! is kept nowhere and returned for the card. A conversation whose less specific subscription took in a new target
//! (a module whose task is watched here now) is told so in a line of its own, with no card. The person's own command
//! has its targets led to the end when they chose *Lead to the end* in its picker ([`lead_in`], OA-10), or when its
//! order begins with those words (`orchester_core::orchestrator::watch::asked_to_lead`), within the ceilings the
//! project gives a new leading subscription (Project settings › Orchestrator › *Leading to the end*). The command
//! keeps that it did. The watch the agent applies at the person's request never chooses it.

use std::{collections::HashMap, path::Path};

use orchester_core::{
  id::ChatId,
  orchestrator::{
    Chat, ChatEntry, ChatEvent, Claim, EntryKind, Subscription, Subscriptions, WatchMode, WatchTarget,
    watch::{AtRequest, LedElsewhere, NamedTarget, Told, WatchCommand, WatchLine, asked_to_lead},
  },
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError, WatchStart};

use super::{WatchError, kept, read, tell};

/// What a /watch applied: what the conversation leads from now on, what it led already, what other conversations
/// lead (the card's), and what was closed or deleted meanwhile.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watched {
  pub started: Vec<NamedTarget>,
  pub already_here: Vec<NamedTarget>,
  pub led_elsewhere: Vec<LedElsewhere>,
  pub gone: Vec<NamedTarget>,
}

impl Watched {
  fn applied_nothing(&self) -> bool {
    self.started.is_empty() && self.already_here.is_empty() && self.led_elsewhere.is_empty()
  }
}

/// `chat` watches `targets`, as the composer's chips name them, following `order` (blank: the project's default),
/// in the project at `root`. The command and its line are kept in `chat`, which is kept with them when it is new.
/// Refused, with nothing kept, when it names nothing or every target is gone. Blocks on SQLite.
pub fn watch_in(root: &Path, chat: &Chat, targets: &[NamedTarget], order: &str) -> Result<Watched, WatchError> {
  apply(root, chat, targets, (order, By::Command))
}

/// [`watch_in`] with *Lead to the end* chosen in the command's picker: `chat` leads `targets` to the end, within the
/// ceilings the project gives a new leading subscription. Only the person chooses it; a target the conversation
/// followed is led from now on.
pub fn lead_in(root: &Path, chat: &Chat, targets: &[NamedTarget], order: &str) -> Result<Watched, WatchError> {
  apply(root, chat, targets, (order, By::Lead))
}

/// The watch the person asked the Orchestrator for in words, which its `watch` tool applies (ORC-893): as
/// [`watch_in`], by the same rules, but no command of theirs is kept, only the line, at their request with `order`.
pub fn watch_asked_in(root: &Path, chat: &Chat, targets: &[NamedTarget], order: &str) -> Result<Watched, WatchError> {
  apply(root, chat, targets, (order, By::Request))
}

/// Whose a /watch is, and what it chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum By {
  /// The person's own command: it follows its targets, unless its order begins with the words for leading them.
  Command,
  /// The person's own command, with *Lead to the end* chosen in its picker.
  Lead,
  /// The watch the agent applies at the person's request: it always follows.
  Request,
}

impl By {
  /// Whether a /watch with `order` leads its targets to the end. Only the person's own command chooses it.
  fn leads(self, order: &str) -> bool {
    match self {
      Self::Lead => true,
      Self::Command => asked_to_lead(order).is_some(),
      Self::Request => false,
    }
  }
}

/// [`watch_in`], [`lead_in`] or [`watch_asked_in`], as `by` says.
fn apply(root: &Path, chat: &Chat, targets: &[NamedTarget], (order, by): (&str, By)) -> Result<Watched, WatchError> {
  if targets.is_empty() {
    return Err(WatchError::NoTarget);
  }
  let asked = by == By::Request;
  let mut db = ProjectDb::open(root)?;
  let now = Timestamp::now();
  // A leading command's order is what follows the words for it, when it began with them.
  let (to_end, command_order) = (by.leads(order), order);
  let (mode, order) = if to_end {
    let ceilings = db.lead_defaults()?;
    (WatchMode::Lead { ceilings }, asked_to_lead(order).unwrap_or(order))
  } else {
    (WatchMode::Follow, order)
  };
  let mut applying = Applying {
    chat: chat.id(),
    order,
    mode,
    now,
    set: db.active_subscriptions()?,
    watched: Watched::default(),
    told: Vec::new(),
  };
  let mut named = Vec::new();
  for given in targets {
    if named.iter().any(|kept: &NamedTarget| kept.target == given.target) {
      continue;
    }
    named.push(applying.claim(&mut db, given)?);
  }
  let Applying { watched, told, .. } = applying;
  if watched.applied_nothing() {
    return Err(WatchError::Gone(watched.gone));
  }
  // The command is kept as the person wrote it; the lines name the order that applies.
  let command_order = command_order.trim().to_owned();
  let order = order.trim().to_owned();
  let line = ChatEvent::Watch {
    line: WatchLine::Watching {
      started: watched.started.clone(),
      already_here: watched.already_here.clone(),
      led_elsewhere: watched.led_elsewhere.clone(),
      gone: watched.gone.clone(),
      at_request: asked.then(|| AtRequest { order: order.clone() }),
    },
  };
  let entries = if asked {
    vec![line]
  } else {
    let command = WatchCommand::Watch {
      targets: named,
      order: command_order,
      to_end,
    };
    vec![ChatEvent::WatchCommand { command }, line]
  };
  db.keep_watch_entries(chat, &entries, now)?;
  let here = kept(&db, chat)?;
  for owed in told {
    tell(&mut db, owed, &here, now)?;
  }
  Ok(watched)
}

/// One /watch being applied: the subscriptions as they stand after each target, and who is owed a line.
struct Applying<'a> {
  chat: ChatId,
  order: &'a str,
  /// How the command takes its targets: following them, or leading them to the end.
  mode: WatchMode,
  now: Timestamp,
  set: Subscriptions,
  watched: Watched,
  told: Vec<(Told, NamedTarget)>,
}

impl Applying<'_> {
  /// Claims `given` and keeps what it means; returns the target as the command names it.
  fn claim(&mut self, db: &mut ProjectDb, given: &NamedTarget) -> Result<NamedTarget, StoreError> {
    let Some(found) = read::find(db, given.target)? else {
      self.watched.gone.push(given.clone());
      return Ok(given.clone());
    };
    let named = found.named;
    let subscription = Subscription::new(self.chat, named.target, self.order, self.now).in_mode(self.mode);
    match self.set.claim(self.chat, named.target, found.module) {
      Claim::New { .. } => match db.start_subscription(&subscription)? {
        WatchStart::Started => {
          let taken = self.set.take_over(subscription, found.module);
          if let Some(told) = taken.told() {
            self.told.push((told, named.clone()));
          }
          self.set = taken.into_set();
          self.watched.started.push(named.clone());
        }
        WatchStart::AlreadyHere => self.watched.already_here.push(named.clone()),
        WatchStart::LedElsewhere(led) => self.led_elsewhere(db, named.clone(), (led.chat(), led.since()))?,
      },
      Claim::AlreadyHere => {
        let led = self.set.of(named.target);
        let renewed = led.is_some_and(|led| led.order() != subscription.order());
        // A target the conversation only follows is led to the end from now on: a renewal keeps how a target is
        // taken, so the following ends and the leading starts.
        let leads_now = self.mode.ceilings().is_some() && led.is_some_and(|led| led.mode().ceilings().is_none());
        if leads_now {
          db.stop_subscription(self.chat, named.target, self.now)?;
          db.start_subscription(&subscription)?;
          self.set = db.active_subscriptions()?;
        } else if renewed {
          db.take_over_subscription(&subscription)?;
          self.set = self.set.take_over(subscription, found.module).into_set();
        }
        self.watched.already_here.push(named.clone());
      }
      Claim::LedElsewhere { chat, since } => self.led_elsewhere(db, named.clone(), (chat, since))?,
    }
    Ok(named)
  }

  /// `named`, which conversation `chat` leads since `since`, goes to the card.
  fn led_elsewhere(
    &mut self,
    db: &ProjectDb,
    named: NamedTarget,
    (chat, since): (ChatId, Timestamp),
  ) -> Result<(), StoreError> {
    self.watched.led_elsewhere.push(LedElsewhere {
      target: named,
      chat,
      title: read::title(db, chat)?,
      since,
    });
    Ok(())
  }
}

/// The entries among `entries`, a conversation's, that are the line of a /watch which led its targets to the end: the
/// line kept right after the person's own command that did, as the command keeps it; one kept before commands kept it
/// is told by the words its order begins with. The conversation tells such a line as *Leading … to the end*.
///
/// Each with the targets it is the latest such line of: what would stop one of them while the person is away is told
/// under that line, once per target however often the command was repeated.
pub(in crate::orchestrator) fn led_to_end(entries: &[ChatEntry]) -> HashMap<i64, Vec<WatchTarget>> {
  let mut lines: HashMap<i64, Vec<WatchTarget>> = HashMap::new();
  let mut told: Vec<WatchTarget> = Vec::new();
  for pair in entries.windows(2).rev() {
    let (
      EntryKind::Event(ChatEvent::WatchCommand {
        command: WatchCommand::Watch { order, to_end, .. },
      }),
      EntryKind::Event(ChatEvent::Watch {
        line:
          WatchLine::Watching {
            started,
            already_here,
            at_request: None,
            ..
          },
      }),
    ) = (&pair[0].kind, &pair[1].kind)
    else {
      continue;
    };
    if !(*to_end || asked_to_lead(order).is_some()) {
      continue;
    }
    let mut latest_of = Vec::new();
    for target in started.iter().chain(already_here).map(|named| named.target) {
      if !told.contains(&target) {
        told.push(target);
        latest_of.push(target);
      }
    }
    lines.insert(pair[1].seq, latest_of);
  }
  lines
}
