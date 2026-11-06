//! `/watch-task` and `/watch-module`: each target is claimed for the conversation (`Subscriptions::claim`); a new one
//! is kept at once, one the conversation leads already takes the command's order, and one another conversation leads
//! is kept nowhere and returned for the card. A conversation whose less specific subscription took in a new target
//! (a module whose task is watched here now) is told so in a line of its own, with no card.

use std::path::Path;

use orchester_core::{
  id::ChatId,
  orchestrator::{
    Chat, ChatEvent, Claim, Subscription, Subscriptions,
    watch::{LedElsewhere, NamedTarget, Told, WatchCommand, WatchLine},
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
  if targets.is_empty() {
    return Err(WatchError::NoTarget);
  }
  let mut db = ProjectDb::open(root)?;
  let now = Timestamp::now();
  let mut applying = Applying {
    chat: chat.id(),
    order,
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
  let command = WatchCommand::Watch {
    targets: named,
    order: order.trim().to_owned(),
  };
  let line = WatchLine::Watching {
    started: watched.started.clone(),
    already_here: watched.already_here.clone(),
    led_elsewhere: watched.led_elsewhere.clone(),
    gone: watched.gone.clone(),
  };
  db.keep_watch_entries(
    chat,
    &[ChatEvent::WatchCommand { command }, ChatEvent::Watch { line }],
    now,
  )?;
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
    let subscription = Subscription::new(self.chat, named.target, self.order, self.now);
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
        let renewed = self
          .set
          .of(named.target)
          .is_some_and(|led| led.order() != subscription.order());
        if renewed {
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
