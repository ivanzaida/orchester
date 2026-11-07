//! What a conversation keeps of the watch commands the app applies itself (`docs/design/orchestrator-watch.md`, frames
//! 04 `OKtX2` and 05 `EIjDH`, lines `avDZO`): the person's command, as its bubble shows it with its chips and order,
//! and the lines that tell what changed for the conversation's subscriptions. Each names its targets as people read
//! them when it was written, so a line stays readable after its task or module is gone.

use serde::{Deserialize, Serialize};

use super::{OpenWork, WatchTarget};
use crate::{id::ChatId, time::Timestamp};

/// A target with the name people read it by when the entry was written: a task's key, a module's name; `None` for the
/// tasks without a module and the whole project, which the app names in its own words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedTarget {
  pub target: WatchTarget,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
}

/// The person's command, kept as their entry: it starts no turn and spends nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum WatchCommand {
  /// `/watch-task` or `/watch-module` with its chips and the order typed after them (blank: the project's default).
  Watch { targets: Vec<NamedTarget>, order: String },
  /// `/unwatch` of these targets.
  Unwatch { targets: Vec<NamedTarget> },
  /// `/unwatch` of everything the conversation watches.
  UnwatchAll,
  /// `/watching`: the app answers it with what the conversation leads and what waits there, read from the live
  /// state whenever the answer is shown (frame 06 `Ff7AD`), so the entry keeps nothing else.
  Watching,
}

/// A target another conversation leads, as the card *Open that conversation* / *Take it here* shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedElsewhere {
  pub target: NamedTarget,
  pub chat: ChatId,
  /// That conversation's title when the line was written.
  pub title: String,
  /// Since when it leads the target.
  pub since: Timestamp,
}

/// A task a stop left open, by its key, and what is open of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenTask {
  pub task: String,
  pub work: OpenWork,
}

/// A line about what the conversation watches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "line", rename_all = "snake_case")]
pub enum WatchLine {
  /// After a /watch: what the conversation leads from now on, what it led already (its order renewed when the command
  /// gave another one), what other conversations lead (kept nowhere: the card offers them), and what was closed or
  /// deleted meanwhile.
  Watching {
    started: Vec<NamedTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    already_here: Vec<NamedTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    led_elsewhere: Vec<LedElsewhere>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    gone: Vec<NamedTarget>,
  },
  /// In the conversation that lost `target`: *X is now led in «title»*, conversation `chat`. `taken_over` when the
  /// person took it with *Take it here*; otherwise a subscription inside what this conversation watches (a task of its
  /// module) took it, with no card.
  NowLedIn {
    target: NamedTarget,
    chat: ChatId,
    title: String,
    taken_over: bool,
  },
  /// After *Take it here*: the conversation leads `target`, taken from conversation `from`, titled `title` then.
  TookOver {
    target: NamedTarget,
    from: ChatId,
    title: String,
  },
  /// After *Open that conversation* on the card of `target`: the person left it where conversation `chat`, titled
  /// `title` then, leads it. Nothing changed for the subscriptions; the card settles into this line.
  LeftThere {
    target: NamedTarget,
    chat: ChatId,
    title: String,
  },
  /// A person's stop: what it ended and what it left open. Nothing was canceled or paused.
  Stopped {
    targets: Vec<NamedTarget>,
    left_open: Vec<OpenTask>,
  },
  /// In the conversation whose less specific subscription covers `target` again, once conversation `from` (titled
  /// `title` then) stopped leading it: its events come here again.
  LedHereAgain {
    target: NamedTarget,
    from: ChatId,
    title: String,
  },
}
