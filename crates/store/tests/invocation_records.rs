//! The invocation records (`InvocationStart`, `ActivityRecord`, `InvocationRecord`), the values the runner's executor
//! sends to the coordinator's sink (ORC-678): each kind survives serialization unchanged, written as it arrived it
//! stores what the direct write stores, and none carries a secret's value. `cargo test -p orchester-store --test
//! invocation_records invocation_record`.

mod support;

use std::sync::Arc;

use orchester_core::{
  code_index::CodeIndexUse,
  command::{SecretVariable, redact_secrets},
  connection::{
    AgentProvider, BaseUrl, Effort, EngineVendor,
    stream::{AgentEvent, InvocationOutcome},
  },
  id::{AgentAccountId, ConnectionId, InvocationId, MemoryId, SecretId},
  memory::{Delivery, InjectedMemory, InjectionReason, MemoryStore},
  permission::{
    AgentPermissions, CommandPermission, LaunchPermissions, NetworkAccess, OutsideCommands, RepositoryAccess,
  },
  run::{EditProgress, InvocationEngine, InvocationKind, TokenUsage},
  secret::SecretValue,
  store::ActivityStore,
  usage::UsedProvider,
};
use orchester_store::{ActivityRecord, InvocationRecord, InvocationStart, StoreError};
use serde::{Serialize, de::DeserializeOwned};

use self::support::{at, model, started_run};

/// A provider key's value; no record may carry it.
const KEY_VALUE: &str = "sk-invocation-records-7f3c9a21";

/// `record` sent as JSON and read back, as a sink in another process would receive it, with the JSON it crossed as.
fn sent<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(record: &T) -> (T, String) {
  let text = serde_json::to_string(record).expect("a record serializes");
  let received: T = serde_json::from_str(&text).expect("a record deserializes");
  assert_eq!(&received, record, "the record survives being sent");
  (received, text)
}

#[test]
fn invocation_record_start_round_trips_and_numbers_the_invocation() {
  let (_repo, mut db, run, execution) = started_run();
  let first = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("a direct start");
  let (start, _) = sent(&InvocationStart::agent(run.id(), execution, &model(), at(20)));
  let number = db.write_invocation_start(&start).expect("written");
  let second = start.invocation(number);
  assert_eq!(second.number, first.number + 1);
  let (command, _) = sent(&InvocationStart::command(run.id(), execution, at(30)));
  let third = command.invocation(db.write_invocation_start(&command).expect("written"));
  assert_eq!(third.kind, InvocationKind::Command);
  assert_eq!(db.invocations(run.id()).expect("read"), [first, second, third]);
}

#[test]
fn invocation_record_of_state_and_activity_stores_what_direct_writes_store() {
  let (_repo, mut db, run, execution) = started_run();
  let mut invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  let events = vec![
    (at(11), AgentEvent::TurnStarted),
    (
      at(12),
      AgentEvent::Message {
        text: "Done.".to_owned(),
      },
    ),
  ];
  let direct = db.append_activity(&invocation, &events[..1]).expect("direct");
  let (record, _) = sent(&ActivityRecord::new(&invocation, events[1..].to_vec()));
  assert_eq!(db.write_activity(&record).expect("written"), direct + 1);
  let stored: Vec<_> = db
    .activity(run.id(), 0)
    .expect("read")
    .into_iter()
    .map(|entry| (entry.at, entry.event))
    .collect();
  assert_eq!(stored, events);

  invocation.session = Some("thread-1".to_owned());
  invocation.usage = TokenUsage {
    input: 120,
    cached_input: 40,
    output: 9,
  };
  invocation.timing.wait_for_person(at(13));
  invocation.timing.resume(at(15));
  invocation.ended_at = Some(at(16));
  invocation.outcome = Some(InvocationOutcome::Completed);
  let (record, _) = sent(&InvocationRecord::state(&invocation));
  db.write_invocation(&record).expect("written");
  assert_eq!(db.invocations(run.id()).expect("read"), [invocation]);
}

#[test]
fn invocation_record_of_each_launch_row_is_written_as_directly() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start")
    .id;
  let permissions = LaunchPermissions::new(
    AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AskBefore,
      NetworkAccess::Off,
    ),
    OutsideCommands::SandboxApproval,
  );
  let engine = InvocationEngine {
    endpoint: BaseUrl::new("https://api.anthropic.com").expect("base URL"),
    vendor: EngineVendor::Provider(AgentProvider::AnthropicApi),
    effort: Some(Effort::High),
  };
  let injected = [InjectedMemory {
    memory: MemoryId::generate(),
    reason: InjectionReason::Handoff,
    tokens: 120,
    delivery: Delivery::Truncated,
  }];
  let account = AgentAccountId::generate();
  let (base_url, key) = (
    BaseUrl::new("https://api.deepseek.com").expect("base URL"),
    SecretId::generate(),
  );
  let connection = UsedProvider {
    id: ConnectionId::generate(),
    name: Arc::from("Codex"),
  };
  let records = [
    InvocationRecord::permissions(invocation, &permissions),
    InvocationRecord::connection(invocation, &connection),
    InvocationRecord::engine(invocation, &engine),
    InvocationRecord::account(invocation, account),
    InvocationRecord::provider(invocation, &base_url, key),
    InvocationRecord::code_index(invocation, &CodeIndexUse::Off),
    InvocationRecord::edit_progress(invocation, &EditProgress::started()),
    InvocationRecord::memories(invocation, &injected),
  ];
  for record in &records {
    db.write_invocation(&sent(record).0).expect("written");
  }
  let stored = db.invocations(run.id()).expect("read").remove(0);
  assert_eq!(stored.permissions, Some(permissions));
  assert_eq!(db.invocation_engine(invocation).expect("read"), Some(engine));
  assert_eq!(db.invocation_accounts(run.id()).expect("read"), [(invocation, account)]);
  assert_eq!(db.invocation_provider(invocation).expect("read"), Some((base_url, key)));
  assert_eq!(
    db.invocation_code_index(invocation).expect("read"),
    Some(CodeIndexUse::Off)
  );
  assert_eq!(
    db.edit_progress(invocation).expect("read"),
    Some(EditProgress::started())
  );
  assert_eq!(db.injected_memory(invocation).expect("read"), injected);
  assert!(
    db.save_invocation_connection(invocation, &connection).is_err(),
    "the connection was recorded once already"
  );
}

#[test]
fn invocation_record_keeps_the_stores_checks_after_it_was_sent() {
  let (_repo, mut db, _run, _execution) = started_run();
  let unknown = InvocationId::generate();
  let (record, _) = sent(&InvocationRecord::code_index(unknown, &CodeIndexUse::Off));
  assert!(matches!(
    db.write_invocation(&record),
    Err(StoreError::InvocationNotFound(id)) if id == unknown
  ));
}

#[test]
fn invocation_record_carries_no_secret_value() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  let value = Arc::new(SecretValue::new(KEY_VALUE.to_owned()).expect("not blank"));
  let secrets = [SecretVariable::new("PROVIDER_KEY", value)];
  // The runner redacts what the agent printed before it becomes a record.
  let printed = redact_secrets(&format!("provider key: {KEY_VALUE}"), &secrets);
  let activity = ActivityRecord::new(&invocation, vec![(at(11), AgentEvent::Message { text: printed })]);
  let key = SecretId::generate();
  let provider = InvocationRecord::provider(
    invocation.id,
    &BaseUrl::new("https://api.deepseek.com").expect("base URL"),
    key,
  );
  let (activity, activity_text) = sent(&activity);
  let (provider, provider_text) = sent(&provider);
  for text in [&activity_text, &provider_text] {
    assert!(!text.contains(KEY_VALUE), "a record carries the key's value: {text}");
  }
  assert!(
    provider_text.contains(&key.to_string()),
    "the key is named by its secret"
  );
  db.write_activity(&activity).expect("written");
  db.write_invocation(&provider).expect("written");
}
