//! Native agents' conversations (`docs/architecture/native-agent.md` §6.2): `native_conversations` holds
//! what every request of a conversation sends (its model, system prompt and tools), `native_items` its
//! items in order, each with the invocation that added it and its segment, which a compaction starts.
//! Both are append-only. `invocation_engine` keeps what a native invocation reached its model through.

use orchester_core::{
  connection::WireApi,
  id::{ConversationId, InvocationId},
  native::{
    item::Item,
    store::{NativeConversation, NativeConversationStore},
  },
  permission::RepositoryAccess,
  resource::ModelId,
  run::InvocationEngine,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const ITEM_SCHEMA: u32 = 1;
const ENGINE_SCHEMA: u32 = 1;
const ITEM: &str = "native conversation item";
const ENGINE: &str = "invocation engine";

impl NativeConversationStore for ProjectDb {
  type Error = StoreError;

  fn create_native_conversation(&mut self, conversation: &NativeConversation) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO native_conversations \
       (id, run_id, step_execution_id, connection_id, wire, model, system, access, created_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
      params![
        conversation.id.to_string(),
        conversation.run.to_string(),
        conversation.execution.to_string(),
        conversation.connection.to_string(),
        wire_text(conversation.wire),
        conversation.model.as_str(),
        conversation.system,
        access_text(conversation.access),
        conversation.created_at.unix_millis(),
      ],
    )?;
    Ok(())
  }

  fn native_conversation(&self, id: ConversationId) -> Result<Option<NativeConversation>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT id, run_id, step_execution_id, connection_id, wire, model, system, access, created_at \
         FROM native_conversations WHERE id = ?1",
        [id.to_string()],
        |row| Ok(conversation_row(row)),
      )
      .optional()?
      .transpose()
  }

  fn append_native_items(
    &mut self,
    conversation: ConversationId,
    invocation: InvocationId,
    items: &[Item],
  ) -> Result<(), StoreError> {
    if items.is_empty() {
      return Ok(());
    }
    let transaction = self.connection.transaction()?;
    let (mut seq, mut segment) = last_item(&transaction, conversation)?;
    {
      let mut insert = transaction.prepare_cached(
        "INSERT INTO native_items (conversation_id, seq, invocation_id, segment, kind, body, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      )?;
      let now = Timestamp::now().unix_millis();
      for item in items {
        seq += 1;
        if matches!(item, Item::Compaction { .. }) {
          segment += 1;
        }
        insert.execute(params![
          conversation.to_string(),
          seq,
          invocation.to_string(),
          segment,
          kind(item),
          encode_json(ITEM, ITEM_SCHEMA, item)?,
          now,
        ])?;
      }
    }
    transaction.commit()?;
    Ok(())
  }

  fn native_items(&self, conversation: ConversationId) -> Result<Vec<Item>, StoreError> {
    let mut statement = self
      .connection
      .prepare_cached("SELECT body FROM native_items WHERE conversation_id = ?1 ORDER BY seq")?;
    let bodies = statement.query_map([conversation.to_string()], |row| row.get::<_, String>(0))?;
    bodies.map(|body| decode_json(ITEM, ITEM_SCHEMA, &body?)).collect()
  }
}

pub(super) fn save_engine(
  connection: &Connection,
  invocation: InvocationId,
  engine: &InvocationEngine,
) -> Result<(), StoreError> {
  let changed = connection.execute(
    "INSERT INTO invocation_engine (invocation_id, body) SELECT ?1, ?2 WHERE EXISTS \
     (SELECT 1 FROM invocations WHERE id = ?1)",
    params![invocation.to_string(), encode_json(ENGINE, ENGINE_SCHEMA, engine)?],
  )?;
  if changed == 0 {
    return Err(StoreError::InvocationNotFound(invocation));
  }
  Ok(())
}

pub(super) fn load_engine(
  connection: &Connection,
  invocation: InvocationId,
) -> Result<Option<InvocationEngine>, StoreError> {
  let body: Option<String> = connection
    .query_row(
      "SELECT body FROM invocation_engine WHERE invocation_id = ?1",
      [invocation.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  body.map(|body| decode_json(ENGINE, ENGINE_SCHEMA, &body)).transpose()
}

/// The sequence and segment of the conversation's last item; `(0, 0)` before its first.
fn last_item(connection: &Connection, conversation: ConversationId) -> Result<(i64, i64), StoreError> {
  let last: Option<(i64, i64)> = connection
    .query_row(
      "SELECT seq, segment FROM native_items WHERE conversation_id = ?1 ORDER BY seq DESC LIMIT 1",
      [conversation.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?;
  Ok(last.unwrap_or((0, 0)))
}

fn conversation_row(row: &Row<'_>) -> Result<NativeConversation, StoreError> {
  let text = |index: usize| row.get::<_, String>(index);
  Ok(NativeConversation {
    id: parse("native conversation id", &text(0)?)?,
    run: parse("native conversation run", &text(1)?)?,
    execution: parse("native conversation execution", &text(2)?)?,
    connection: parse("native conversation connection", &text(3)?)?,
    wire: wire_value(&text(4)?)?,
    model: ModelId::new(&text(5)?).map_err(|error| corrupt("native conversation model", error))?,
    system: text(6)?,
    access: access_value(&text(7)?)?,
    created_at: Timestamp::from_unix_millis(row.get(8)?),
  })
}

const fn kind(item: &Item) -> &'static str {
  match item {
    Item::User { .. } => "user",
    Item::Assistant { .. } => "assistant",
    Item::Reasoning { .. } => "reasoning",
    Item::ToolCall { .. } => "tool_call",
    Item::ToolResult { .. } => "tool_result",
    Item::Compaction { .. } => "compaction",
  }
}

const fn wire_text(wire: WireApi) -> &'static str {
  match wire {
    WireApi::AnthropicMessages => "anthropic_messages",
    WireApi::OpenAiResponses => "openai_responses",
  }
}

fn wire_value(text: &str) -> Result<WireApi, StoreError> {
  match text {
    "anthropic_messages" => Ok(WireApi::AnthropicMessages),
    "openai_responses" => Ok(WireApi::OpenAiResponses),
    other => Err(corrupt("native conversation wire", other)),
  }
}

const fn access_text(access: RepositoryAccess) -> &'static str {
  match access {
    RepositoryAccess::ReadOnly => "read_only",
    RepositoryAccess::Write => "write",
  }
}

fn access_value(text: &str) -> Result<RepositoryAccess, StoreError> {
  match text {
    "read_only" => Ok(RepositoryAccess::ReadOnly),
    "write" => Ok(RepositoryAccess::Write),
    other => Err(corrupt("native conversation access", other)),
  }
}
