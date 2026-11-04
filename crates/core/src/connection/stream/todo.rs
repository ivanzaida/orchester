//! The todo list an agent keeps as it works, as its CLI reports it: Claude Code calls its `TodoWrite` tool with the
//! whole list each time (`input.todos[]` of `content` and `status`), Codex App Server sends the turn's plan
//! (`turn/plan/updated`, `params.plan[]` of `step` and `status`). Each update replaces the list before it
//! ([`super::AgentEvent::TodoList`]); an agent that never kept one has no list, never an empty one ([`TodoList`]).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How far the agent got with one item of its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoState {
  Pending,
  InProgress,
  Done,
}

impl TodoState {
  /// The state a CLI writes: Claude Code's `in_progress`, Codex's `inProgress`; `completed` for done.
  fn from_cli(word: &str) -> Option<Self> {
    match word {
      "pending" => Some(Self::Pending),
      "in_progress" | "inProgress" => Some(Self::InProgress),
      "completed" => Some(Self::Done),
      _ => None,
    }
  }
}

/// One item of an agent's todo list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
  pub text: String,
  pub state: TodoState,
}

/// An agent's todo list as its latest update left it: never empty, as an agent without a list has none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<TodoItem>", into = "Vec<TodoItem>")]
pub struct TodoList(Vec<TodoItem>);

impl TodoList {
  /// The list of `items`; `None` when there are none.
  pub fn new(items: Vec<TodoItem>) -> Option<Self> {
    (!items.is_empty()).then_some(Self(items))
  }

  pub fn items(&self) -> &[TodoItem] {
    &self.0
  }
}

impl TryFrom<Vec<TodoItem>> for TodoList {
  type Error = &'static str;

  fn try_from(items: Vec<TodoItem>) -> Result<Self, Self::Error> {
    Self::new(items).ok_or("a todo list holds at least one item")
  }
}

impl From<TodoList> for Vec<TodoItem> {
  fn from(list: TodoList) -> Self {
    list.0
  }
}

/// The list a `TodoWrite` call's input holds; `None` when it holds no list or an item this parser cannot read, so
/// that a list is never kept with an item left out.
pub(crate) fn claude_todos(input: &Value) -> Option<Vec<TodoItem>> {
  items(input.get("todos")?, "content")
}

/// The list a Codex `turn/plan/updated` notification's parameters hold, read as [`claude_todos`] reads its input.
pub(crate) fn codex_plan(params: &Value) -> Option<Vec<TodoItem>> {
  items(params.get("plan")?, "step")
}

fn items(list: &Value, text: &str) -> Option<Vec<TodoItem>> {
  list
    .as_array()?
    .iter()
    .map(|item| {
      Some(TodoItem {
        text: item.get(text)?.as_str()?.to_owned(),
        state: TodoState::from_cli(item.get("status")?.as_str()?)?,
      })
    })
    .collect()
}
