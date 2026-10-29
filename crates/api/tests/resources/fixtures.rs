//! The JSON a team server answers with for resources and versions.

use orchester_core::id::{ConnectionId, IdempotencyKey, ResourceId, SecretId, UserId};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

pub fn user() -> Value {
  json!({"id":UserId::generate(),"display_name":"Owner"})
}

pub fn hash(digit: char) -> String {
  digit.to_string().repeat(64)
}

pub fn key() -> IdempotencyKey {
  IdempotencyKey::generate()
}

pub fn resource(id: ResourceId, kind: &str) -> Value {
  json!({"id":id,"kind":kind,"slug":"feature-delivery","name":"Feature delivery","archived":false,
    "revision":2,"latest":3,"created":{"by":user(),"at":1},"updated":{"by":user(),"at":2}})
}

/// One reference of each kind a body names.
pub fn references() -> Value {
  json!([
    {"kind":"connection","id":ConnectionId::generate(),"name":"Claude","role":"claude_cli"},
    {"kind":"mcp_server","id":ConnectionId::generate(),"name":"GitHub","role":null},
    {"kind":"secret","id":SecretId::generate(),"name":"API token","role":"token"},
    {"kind":"tool","id":ResourceId::generate(),"name":"Deploy","role":null},
  ])
}

pub fn version(resource: ResourceId, number: u32) -> Value {
  json!({"resource":resource,"number":number,"body":{"name":"Feature delivery","steps":[]},
    "hash":hash('a'),"references":references(),"based_on":number.checked_sub(1).filter(|n| *n > 0),
    "published":{"by":user(),"at":3}})
}

/// `value` read as `T` and written back is `value` again.
pub fn round_trip<T: Serialize + DeserializeOwned>(value: &Value) -> T {
  let decoded: T = serde_json::from_value(value.clone()).expect("the shape reads");
  assert_eq!(&serde_json::to_value(&decoded).expect("the shape writes"), value);
  decoded
}
