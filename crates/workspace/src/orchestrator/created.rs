//! A task the chat creates in a turn of a conversation that watches something comes back to that conversation
//! (ORC-765, `orchester_core::orchestrator::watch::created`): outside what it watches, the task becomes its task
//! subscription, with the order of the subscription whose event started the turn (the default order in a person's
//! turn), unless another conversation leads the task's module, or the tasks without a module, already. The agent is
//! told which in the tool's answer (`watch`), and the action keeps it for its row.

use orchester_core::{
  orchestrator::watch::{Created, CreatedWatch, created_task, event_subscription},
  time::Timestamp,
};
use orchester_store::{ProjectDb, WatchStart};
use serde_json::{Value, json};

use super::{guard::thinking_waits, led_start::Named, team_task, tools::ChatScope};
use crate::app_mcp::{Services, ToolError, blocking};

/// The tool whose task comes back to the conversation that created it.
pub(super) const CREATE_TASK: &str = "create_task";
/// What became of the subscription of the task `created`, the answer of `create_task` in `scope`, which then tells
/// the agent too; `None` when nothing changed. A watch that cannot be read or kept is logged: the task stays created.
pub(super) async fn watch_created(services: &Services, scope: &ChatScope, created: &mut Value) -> Option<CreatedWatch> {
  let key = created.pointer("/task/key").and_then(Value::as_str)?.to_owned();
  let (root, chat, on_its_own) = (scope.root.clone(), scope.chat, scope.on_its_own);
  // A team task the chat made a moment ago is in no record here yet: the team server is asked for it, which records
  // its identity, so a watch on it is read with the next reading.
  let watched = match team_task::found(services, scope, &key).await {
    Ok(task) => {
      blocking(move || {
        let mut db = ProjectDb::open(&root)?;
        watch(&mut db, (chat, on_its_own), &task)
      })
      .await
    }
    Err(error) => Err(error),
  };
  let watch = match watched {
    Ok(watch) => watch?,
    Err(error) => {
      tracing::error!(%chat, ?error, "cannot watch a task the Orchestrator created");
      return None;
    }
  };
  if let Value::Object(answer) = created {
    answer.insert("watch".to_owned(), told(&watch));
  }
  Some(watch)
}

/// Reads and keeps what `task`, which `chat` created, means for what it watches; `on_its_own`: an event started the
/// turn, whose subscription gives the order.
fn watch(
  db: &mut ProjectDb,
  (chat, on_its_own): (orchester_core::id::ChatId, bool),
  task: &Named,
) -> Result<Option<CreatedWatch>, ToolError> {
  let subscriptions = db.active_subscriptions()?;
  if subscriptions.of_chat(chat).next().is_none() {
    return Ok(None);
  }
  let event_task = if on_its_own {
    // Of a turn that took several events, the oldest: the one that started it. A team task's event names the task as
    // its runs recorded it.
    thinking_waits(&db.chat_entries(chat)?)
      .first()
      .and_then(|wait| Named::find(db, &wait.task).ok())
  } else {
    None
  };
  let turn_of = event_task.and_then(|event| event_subscription(&subscriptions, chat, event.id, event.module));
  let created = created_task(&subscriptions, chat, turn_of, (task.id, task.module), Timestamp::now());
  match created {
    Created::Unchanged => Ok(None),
    Created::Watch(subscription) => match db.start_subscription(&subscription)? {
      WatchStart::Started => Ok(Some(CreatedWatch::WatchedHere)),
      WatchStart::AlreadyHere => Ok(None),
      WatchStart::LedElsewhere(led) => led_elsewhere(db, led.chat()).map(Some),
    },
    Created::LedElsewhere(led) => led_elsewhere(db, led.chat()).map(Some),
  }
}

fn led_elsewhere(db: &ProjectDb, chat: orchester_core::id::ChatId) -> Result<CreatedWatch, ToolError> {
  let title = db.chat(chat)?.map(|kept| kept.title().to_owned()).unwrap_or_default();
  Ok(CreatedWatch::LedElsewhere { chat, title })
}

/// What the agent is told of `watch`, in the tool's answer.
fn told(watch: &CreatedWatch) -> Value {
  match watch {
    CreatedWatch::WatchedHere => json!({
      "kind": "watched_here",
      "note": "The task became a task subscription of this conversation: its events come here.",
    }),
    CreatedWatch::LedElsewhere { chat, title } => json!({
      "kind": "led_elsewhere",
      "chat": chat.to_string(),
      "title": title,
      "note": format!(
        "Another conversation, \"{title}\", leads this task's module (or the tasks without a module): its events go \
         there, and nothing was added here. Tell the person who leads it."
      ),
    }),
  }
}
