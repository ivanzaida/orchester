//! Taking a target and stopping a subscription: the new set of subscriptions and what the other conversation it
//! concerns is told, as data its line is written from.

use super::{Subscription, Subscriptions, WatchTarget};
use crate::id::{ChatId, ModuleId};

/// What another conversation is told when a subscription changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Told {
  chat: ChatId,
  news: News,
}

impl Told {
  /// The conversation told.
  pub const fn chat(&self) -> ChatId {
    self.chat
  }

  pub const fn news(&self) -> News {
    self.news
  }
}

/// What changed for the conversation told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum News {
  /// It led `target`, which `by` took over.
  TakenOver { target: WatchTarget, by: ChatId },
  /// `target` lies inside what it watches, and `to` leads it from now on (a task inside its module, say).
  Loses { target: WatchTarget, to: ChatId },
  /// `target` lies inside what it watches, and `from` stopped leading it: its events come here again.
  Regains { target: WatchTarget, from: ChatId },
}

/// A target taken: the new set and who is told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taken {
  set: Subscriptions,
  told: Option<Told>,
}

impl Taken {
  pub const fn set(&self) -> &Subscriptions {
    &self.set
  }

  pub fn into_set(self) -> Subscriptions {
    self.set
  }

  pub const fn told(&self) -> Option<Told> {
    self.told
  }
}

/// A subscription stopped: the new set, the subscription it ended and who is told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stopped {
  set: Subscriptions,
  stopped: Subscription,
  told: Option<Told>,
}

impl Stopped {
  pub const fn set(&self) -> &Subscriptions {
    &self.set
  }

  pub fn into_set(self) -> Subscriptions {
    self.set
  }

  pub const fn stopped(&self) -> &Subscription {
    &self.stopped
  }

  pub const fn told(&self) -> Option<Told> {
    self.told
  }
}

impl Subscriptions {
  /// `subscription`'s conversation leads its target from now on, whatever [`Subscriptions::claim`] said: a new
  /// target is added, the conversation's own is replaced (a new order), and another conversation's is taken over and
  /// that conversation told. A new target taken from inside another conversation's less specific one tells that
  /// conversation it loses it. `task_module` is the module of a task target (`None`: it has none) and says nothing
  /// for the other targets.
  pub fn take_over(&self, subscription: Subscription, task_module: Option<ModuleId>) -> Taken {
    let target = subscription.target();
    let chat = subscription.chat();
    let told = match self.of(target) {
      Some(led) => (led.chat() != chat).then(|| Told {
        chat: led.chat(),
        news: News::TakenOver { target, by: chat },
      }),
      None => self
        .outer_leader(target, task_module)
        .filter(|&other| other != chat)
        .map(|other| Told {
          chat: other,
          news: News::Loses { target, to: chat },
        }),
    };
    let mut kept = self.without(target);
    kept.push(subscription);
    Taken {
      set: Self { kept },
      told,
    }
  }

  /// `chat` stops leading `target`; the other conversation whose less specific subscription covers it now is told it
  /// regains it. `None` when `chat` does not lead `target`. `task_module` is as for [`Subscriptions::take_over`].
  pub fn stop(&self, chat: ChatId, target: WatchTarget, task_module: Option<ModuleId>) -> Option<Stopped> {
    let stopped = self.of(target).filter(|led| led.chat() == chat)?.clone();
    let told = self
      .outer_leader(target, task_module)
      .filter(|&other| other != chat)
      .map(|other| Told {
        chat: other,
        news: News::Regains { target, from: chat },
      });
    Some(Stopped {
      set: Self {
        kept: self.without(target),
      },
      stopped,
      told,
    })
  }

  fn without(&self, target: WatchTarget) -> Vec<Subscription> {
    self
      .kept
      .iter()
      .filter(|subscription| subscription.target() != target)
      .cloned()
      .collect()
  }
}
