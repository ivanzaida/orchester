//! What the project's Orchestrator conversations watch (`orchester_core::orchestrator::watch`): each subscription
//! kept with its conversation, target, order and since when it leads, and, once it ended, when and why. Only the
//! subscriptions that have not ended lead; the ended ones stay for their conversation's lines and History. Each
//! change [`orchester_core::orchestrator::Subscriptions`] decides is kept in one transaction, and a unique index on
//! the leading target keeps one leader per target even when two conversations act at once.

use orchester_core::{
  id::ChatId,
  orchestrator::{
    Chat, ChatEvent, Subscription, Subscriptions, WatchTarget,
    watch::{EndReason, WatchEnd, WatchSummary},
  },
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A subscription as kept, with when and why it ended; `None` while it leads its target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptSubscription {
  pub subscription: Subscription,
  pub ended: Option<(Timestamp, WatchEnd)>,
}

/// What became of a new subscription, as the project kept it when it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchStart {
  /// It leads its target from now on.
  Started,
  /// Its conversation leads the target already; that subscription is unchanged.
  AlreadyHere,
  /// Another conversation leads the target, with this subscription: nothing was kept.
  LedElsewhere(Subscription),
}

const SELECT: &str = "SELECT chat_id, target_kind, target_id, order_text, since, ended_at, end_reason, taken_by,
  module_done, module_canceled FROM orchestrator_subscriptions";

/// The condition on a row that it names the target `?1` (kind), `?2` (its id, empty for none) and leads it.
const LEADS: &str = "target_kind = ?1 AND IFNULL(target_id, '') = ?2 AND ended_at IS NULL";

impl ProjectDb {
  /// The project's subscriptions that lead their targets, in the order they were kept.
  pub fn active_subscriptions(&self) -> Result<Subscriptions, StoreError> {
    let kept = kept(&self.connection, None, "ended_at IS NULL")?;
    Subscriptions::new(kept.into_iter().map(|kept| kept.subscription))
      .map_err(|error| corrupt("orchestrator subscriptions", error))
  }

  /// The subscriptions of `chat` that lead their targets, in the order they were kept.
  pub fn chat_subscriptions(&self, chat: ChatId) -> Result<Vec<Subscription>, StoreError> {
    let kept = kept(&self.connection, Some(chat), "ended_at IS NULL")?;
    Ok(kept.into_iter().map(|kept| kept.subscription).collect())
  }

  /// Every subscription `chat` had, ended ones too, in the order they were kept.
  pub fn chat_subscription_history(&self, chat: ChatId) -> Result<Vec<KeptSubscription>, StoreError> {
    kept(&self.connection, Some(chat), "1")
  }

  /// Keeps `subscription` as the leader of its target ([`orchester_core::orchestrator::Claim::New`]) unless a
  /// conversation leads the target by now.
  pub fn start_subscription(&mut self, subscription: &Subscription) -> Result<WatchStart, StoreError> {
    let transaction = self.connection.transaction()?;
    if let Some(led) = leader(&transaction, subscription.target())? {
      return Ok(if led.chat() == subscription.chat() {
        WatchStart::AlreadyHere
      } else {
        WatchStart::LedElsewhere(led)
      });
    }
    insert(&transaction, subscription)?;
    transaction.commit()?;
    Ok(WatchStart::Started)
  }

  /// Makes `subscription` the leader of its target whoever led it ([`Subscriptions::take_over`]), in one transaction:
  /// the subscription that led it ends at `subscription`'s since, taken over, or renewed when it was its
  /// conversation's own. Returns the subscription it ended, if one led the target.
  pub fn take_over_subscription(&mut self, subscription: &Subscription) -> Result<Option<Subscription>, StoreError> {
    let target = subscription.target();
    let transaction = self.connection.transaction()?;
    let led = leader(&transaction, target)?;
    if let Some(led) = &led {
      let why = if led.chat() == subscription.chat() {
        WatchEnd::Renewed
      } else {
        WatchEnd::TakenOver {
          by: subscription.chat(),
        }
      };
      end_leader(&transaction, target, why, subscription.since())?;
    }
    insert(&transaction, subscription)?;
    transaction.commit()?;
    Ok(led)
  }

  /// The person stops `chat`'s subscription to `target` at `at` ([`Subscriptions::stop`]). Returns the subscription
  /// it ended; `None` when `chat` does not lead `target`.
  pub fn stop_subscription(
    &mut self,
    chat: ChatId,
    target: WatchTarget,
    at: Timestamp,
  ) -> Result<Option<Subscription>, StoreError> {
    self.end_led(chat, target, WatchEnd::Stopped, at)
  }

  /// `chat`'s subscription to `target` ends by itself at `at`, for `reason` ([`orchester_core::orchestrator::watch::
  /// ending`]). Returns the subscription it ended; `None` when `chat` does not lead `target`.
  pub fn end_subscription(
    &mut self,
    chat: ChatId,
    target: WatchTarget,
    reason: EndReason,
    at: Timestamp,
  ) -> Result<Option<Subscription>, StoreError> {
    self.end_led(chat, target, WatchEnd::ByItself(reason), at)
  }

  /// `subscription` ends by itself at `at`, for `summary`'s reason, and its conversation keeps `summary` (its line, and
  /// the turn it starts when it is automatic), in one transaction. Whether it ended: `false` when `subscription` no
  /// longer leads its target (it was stopped, taken over or renewed meanwhile), which leaves everything as it was.
  pub fn end_subscription_by_itself(
    &mut self,
    subscription: &Subscription,
    summary: &WatchSummary,
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let target = subscription.target();
    let transaction = self.connection.transaction()?;
    if leader(&transaction, target)?.as_ref() != Some(subscription) {
      return Ok(false);
    }
    end_leader(&transaction, target, WatchEnd::ByItself(summary.reason), at)?;
    super::orchestrator_watch_summaries::keep(&transaction, subscription, summary, at)?;
    transaction.commit()?;
    Ok(true)
  }

  /// Keeps `events`, a person's watch command and the lines it writes, in `chat` (stored with it and its *Plan only*
  /// switch when it is new) at `at`, in one transaction and in their order. They start no turn: the app applies the
  /// command itself, also while a turn of the conversation is under way.
  pub fn keep_watch_entries(&mut self, chat: &Chat, events: &[ChatEvent], at: Timestamp) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT INTO orchestrator_chats (id, title, started_at, plan_only) VALUES (?1, ?2, ?3, ?4)
       ON CONFLICT (id) DO NOTHING",
      params![
        chat.id().to_string(),
        chat.title(),
        chat.started().unix_millis(),
        chat.plan_only()
      ],
    )?;
    for event in events {
      let body = serde_json::to_string(event).map_err(|error| corrupt("chat event", error))?;
      transaction.execute(
        "INSERT INTO orchestrator_entries (chat_id, kind, at, body) VALUES (?1, 'event', ?2, ?3)",
        params![chat.id().to_string(), at.unix_millis(), body],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }

  fn end_led(
    &mut self,
    chat: ChatId,
    target: WatchTarget,
    why: WatchEnd,
    at: Timestamp,
  ) -> Result<Option<Subscription>, StoreError> {
    let transaction = self.connection.transaction()?;
    let Some(led) = leader(&transaction, target)?.filter(|led| led.chat() == chat) else {
      return Ok(None);
    };
    end_leader(&transaction, target, why, at)?;
    transaction.commit()?;
    Ok(Some(led))
  }
}

/// Ends, at `at`, every subscription of `chat` that leads its target, as its conversation is deleted.
pub(super) fn end_chat_subscriptions(connection: &Connection, chat: ChatId, at: Timestamp) -> Result<(), StoreError> {
  connection.execute(
    "UPDATE orchestrator_subscriptions SET ended_at = ?2, end_reason = 'chat_deleted'
     WHERE chat_id = ?1 AND ended_at IS NULL",
    params![chat.to_string(), at.unix_millis()],
  )?;
  Ok(())
}

/// The subscriptions of `chat` (`None`: of every conversation) that hold for `condition`, a fixed SQL condition on
/// their rows, in the order they were kept.
fn kept(
  connection: &Connection,
  chat: Option<ChatId>,
  condition: &'static str,
) -> Result<Vec<KeptSubscription>, StoreError> {
  let mut statement = connection.prepare(&format!(
    "{SELECT} WHERE (?1 IS NULL OR chat_id = ?1) AND {condition} ORDER BY id"
  ))?;
  let rows = statement
    .query_map([chat.map(|chat| chat.to_string())], Columns::read)?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(Columns::into_kept).collect()
}

/// The subscription that leads `target`, if any.
fn leader(connection: &Connection, target: WatchTarget) -> Result<Option<Subscription>, StoreError> {
  let (kind, id) = target_columns(target);
  let row = connection
    .query_row(
      &format!("{SELECT} WHERE {LEADS}"),
      params![kind, id.unwrap_or_default()],
      Columns::read,
    )
    .optional()?;
  row.map(|row| row.into_kept().map(|kept| kept.subscription)).transpose()
}

fn insert(connection: &Connection, subscription: &Subscription) -> Result<(), StoreError> {
  let (kind, id) = target_columns(subscription.target());
  connection.execute(
    "INSERT INTO orchestrator_subscriptions (chat_id, target_kind, target_id, order_text, since)
     VALUES (?1, ?2, ?3, ?4, ?5)",
    params![
      subscription.chat().to_string(),
      kind,
      id,
      subscription.order().unwrap_or_default(),
      subscription.since().unix_millis()
    ],
  )?;
  Ok(())
}

/// Ends the subscription that leads `target` at `at`, for `why`.
fn end_leader(connection: &Connection, target: WatchTarget, why: WatchEnd, at: Timestamp) -> Result<(), StoreError> {
  let (kind, id) = target_columns(target);
  let (reason, taken_by, counts) = end_columns(why)?;
  connection.execute(
    &format!(
      "UPDATE orchestrator_subscriptions SET ended_at = ?3, end_reason = ?4, taken_by = ?5, module_done = ?6,
         module_canceled = ?7
       WHERE {LEADS}"
    ),
    params![
      kind,
      id.unwrap_or_default(),
      at.unix_millis(),
      reason,
      taken_by,
      counts.map(|(done, _)| done),
      counts.map(|(_, canceled)| canceled)
    ],
  )?;
  Ok(())
}

fn target_columns(target: WatchTarget) -> (&'static str, Option<String>) {
  match target {
    WatchTarget::Task(task) => ("task", Some(task.to_string())),
    WatchTarget::Module(module) => ("module", Some(module.to_string())),
    WatchTarget::WithoutModule => ("without_module", None),
    WatchTarget::Project => ("project", None),
  }
}

fn target_of(kind: &str, id: Option<&str>) -> Result<WatchTarget, StoreError> {
  match (kind, id) {
    ("task", Some(task)) => Ok(WatchTarget::Task(parse("task id", task)?)),
    ("module", Some(module)) => Ok(WatchTarget::Module(parse("module id", module)?)),
    ("without_module", None) => Ok(WatchTarget::WithoutModule),
    ("project", None) => Ok(WatchTarget::Project),
    _ => Err(corrupt("watch target", kind)),
  }
}

/// The reason, the conversation that took the target over and the module's done and canceled tasks of `why`.
type EndColumns = (&'static str, Option<String>, Option<(i64, i64)>);

fn end_columns(why: WatchEnd) -> Result<EndColumns, StoreError> {
  Ok(match why {
    WatchEnd::ByItself(EndReason::TaskDone) => ("task_done", None, None),
    WatchEnd::ByItself(EndReason::TaskCanceled) => ("task_canceled", None, None),
    WatchEnd::ByItself(EndReason::ModuleClosed { done, canceled }) => {
      let count = |tasks: usize| i64::try_from(tasks).map_err(|_| StoreError::Unrepresentable("module task count"));
      ("module_closed", None, Some((count(done)?, count(canceled)?)))
    }
    WatchEnd::Stopped => ("stopped", None, None),
    WatchEnd::TakenOver { by } => ("taken_over", Some(by.to_string()), None),
    WatchEnd::Renewed => ("renewed", None, None),
    WatchEnd::ChatDeleted => ("chat_deleted", None, None),
  })
}

fn end_of(reason: &str, taken_by: Option<&str>, counts: Option<(i64, i64)>) -> Result<WatchEnd, StoreError> {
  let count = |tasks: i64| usize::try_from(tasks).map_err(|_| corrupt("module task count", tasks));
  Ok(match (reason, taken_by, counts) {
    ("task_done", None, None) => WatchEnd::ByItself(EndReason::TaskDone),
    ("task_canceled", None, None) => WatchEnd::ByItself(EndReason::TaskCanceled),
    ("module_closed", None, Some((done, canceled))) => WatchEnd::ByItself(EndReason::ModuleClosed {
      done: count(done)?,
      canceled: count(canceled)?,
    }),
    ("stopped", None, None) => WatchEnd::Stopped,
    ("taken_over", Some(by), None) => WatchEnd::TakenOver {
      by: parse("chat id", by)?,
    },
    ("renewed", None, None) => WatchEnd::Renewed,
    ("chat_deleted", None, None) => WatchEnd::ChatDeleted,
    _ => return Err(corrupt("watch end", reason)),
  })
}

/// The columns of [`SELECT`], read as stored.
struct Columns {
  chat: String,
  target: (String, Option<String>),
  order: String,
  since: i64,
  ended_at: Option<i64>,
  reason: Option<String>,
  taken_by: Option<String>,
  counts: (Option<i64>, Option<i64>),
}

impl Columns {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      chat: row.get(0)?,
      target: (row.get(1)?, row.get(2)?),
      order: row.get(3)?,
      since: row.get(4)?,
      ended_at: row.get(5)?,
      reason: row.get(6)?,
      taken_by: row.get(7)?,
      counts: (row.get(8)?, row.get(9)?),
    })
  }

  fn into_kept(self) -> Result<KeptSubscription, StoreError> {
    let target = target_of(&self.target.0, self.target.1.as_deref())?;
    let subscription = Subscription::new(
      parse("chat id", &self.chat)?,
      target,
      self.order,
      Timestamp::from_unix_millis(self.since),
    );
    let ended = match (self.ended_at, self.reason.as_deref()) {
      (None, None) => None,
      (Some(at), Some(reason)) => {
        let counts = self.counts.0.zip(self.counts.1);
        Some((
          Timestamp::from_unix_millis(at),
          end_of(reason, self.taken_by.as_deref(), counts)?,
        ))
      }
      _ => return Err(corrupt("watch end", "an end without its time or its reason")),
    };
    Ok(KeptSubscription { subscription, ended })
  }
}
