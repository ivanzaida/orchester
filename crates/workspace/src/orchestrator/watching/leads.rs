//! Who leads a module and a task (`i687ei` a and b: *Led in «Webhooks v2 and Search index» · via Webhooks v2*), and
//! what a conversation leads, for its header chips and its History row (`X93D8T`, `i687ei` c: *Leads 2 modules, 1
//! task*).

use std::sync::Arc;

use orchester_core::{
  id::{ChatId, ModuleId, TaskId},
  orchestrator::{Subscription, WatchTarget},
  time::Timestamp,
};

use super::{TargetView, WatchOrder, WatchReads, counted};
use crate::{links::Link, messages::Message};

/// How many of each kind of target a conversation leads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LeadCounts {
  pub modules: usize,
  pub tasks: usize,
  pub without_module: bool,
  pub project: bool,
}

impl LeadCounts {
  pub(super) fn of<'a>(subscriptions: impl IntoIterator<Item = &'a Subscription>) -> Self {
    subscriptions
      .into_iter()
      .fold(Self::default(), |counts, subscription| match subscription.target() {
        WatchTarget::Module(_) => Self {
          modules: counts.modules + 1,
          ..counts
        },
        WatchTarget::Task(_) => Self {
          tasks: counts.tasks + 1,
          ..counts
        },
        WatchTarget::WithoutModule => Self {
          without_module: true,
          ..counts
        },
        WatchTarget::Project => Self {
          project: true,
          ..counts
        },
      })
  }

  pub const fn is_empty(&self) -> bool {
    self.modules == 0 && self.tasks == 0 && !self.without_module && !self.project
  }

  /// The counts in words, one part per kind it leads, modules first (*2 modules*, *1 task*): what the app joins
  /// after *Leads*.
  pub fn parts(&self) -> Vec<Message> {
    let mut parts = Vec::new();
    if self.modules > 0 {
      parts.push(counted(
        self.modules,
        (
          "orchestrator.watch.leads.modules.one",
          "orchestrator.watch.leads.modules.other",
        ),
      ));
    }
    if self.tasks > 0 {
      parts.push(counted(
        self.tasks,
        (
          "orchestrator.watch.leads.tasks.one",
          "orchestrator.watch.leads.tasks.other",
        ),
      ));
    }
    if self.without_module {
      parts.push(Message::new("orchestrator.watch.leads.withoutModule"));
    }
    if self.project {
      parts.push(Message::new("orchestrator.watch.leads.project"));
    }
    parts
  }
}

/// One target a conversation leads, as its header chip and the chip's tooltip show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeadChip {
  pub target: TargetView,
  pub since: Timestamp,
  pub order: WatchOrder,
}

/// What a conversation leads: its targets in the order they were kept, with their counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatLeads {
  pub targets: Vec<LeadChip>,
  pub counts: LeadCounts,
}

/// The conversation that leads a module or a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leader {
  pub chat: ChatId,
  pub title: Arc<str>,
  /// The conversation's link.
  pub link: String,
  /// The less specific subscription it leads through: a task's module, the tasks without a module or the whole
  /// project; `None` when it leads the target itself.
  pub via: Option<TargetView>,
  /// *«Webhooks v2 and Search index»*, *«…» · via Webhooks v2*.
  pub words: Message,
}

/// Who leads a module or a task: *Led in*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedIn {
  /// No subscription covers it: its events go to the usual conversation.
  Nobody,
  Chat(Box<Leader>),
}

impl LedIn {
  /// *Nobody*, or the leader's words.
  pub fn message(&self) -> Message {
    match self {
      Self::Nobody => Message::new("orchestrator.watch.ledIn.nobody"),
      Self::Chat(leader) => leader.words.clone(),
    }
  }
}

impl WatchReads {
  /// What `chat` leads.
  pub fn chat_leads(&self, chat: ChatId) -> ChatLeads {
    ChatLeads {
      targets: self
        .subscriptions
        .of_chat(chat)
        .map(|subscription| LeadChip {
          target: self.target_view(subscription.target()),
          since: subscription.since(),
          order: self.order(subscription),
        })
        .collect(),
      counts: LeadCounts::of(self.subscriptions.of_chat(chat)),
    }
  }

  /// Who leads module `module`: its own subscription, else the whole project's.
  pub fn module_led_in(&self, module: ModuleId) -> LedIn {
    let leading = [WatchTarget::Module(module), WatchTarget::Project]
      .into_iter()
      .find_map(|target| self.subscriptions.of(target));
    self.led_in(leading, WatchTarget::Module(module))
  }

  /// Who leads task `task`: the conversation its events go to, through the most specific subscription that covers
  /// it. [`LedIn::Nobody`] for a task these reads do not hold.
  pub fn task_led_in(&self, task: TaskId) -> LedIn {
    let leading = self.task(task).and_then(|task| self.leading(task));
    self.led_in(leading, WatchTarget::Task(task))
  }

  fn led_in(&self, leading: Option<&Subscription>, own: WatchTarget) -> LedIn {
    let Some(subscription) = leading else {
      return LedIn::Nobody;
    };
    let chat = subscription.chat();
    let title = self.title(chat);
    let via = subscription.target();
    let titled = |key: &'static str| Message::new(key).with("title", title.to_string());
    let words = match via {
      _ if via == own => titled("orchestrator.watch.ledIn.chat"),
      WatchTarget::Module(module) => match self.module_name(module) {
        Some(name) => titled("orchestrator.watch.ledIn.viaModule").with("module", name),
        None => titled("orchestrator.watch.ledIn.chat"),
      },
      WatchTarget::WithoutModule => titled("orchestrator.watch.ledIn.viaWithoutModule"),
      WatchTarget::Project => titled("orchestrator.watch.ledIn.viaProject"),
      // A task is led through its own subscription only, which is `own`.
      WatchTarget::Task(_) => titled("orchestrator.watch.ledIn.chat"),
    };
    LedIn::Chat(Box::new(Leader {
      chat,
      link: Link::Orchestrator(self.project, chat).path(),
      via: (via != own).then(|| self.target_view(via)),
      title,
      words,
    }))
  }
}
