//! How a [`TurnFailure`] is kept: an object tagged by `kind`, an account problem under `problem`
//! (`{"kind":"account","problem":{"kind":"no_account"}}`). Before ORC-623 the problem was written into the failure's
//! own object, its `kind` a second key beside the failure's (`{"kind":"account","kind":"no_account"}`), which serde
//! cannot read back; such a row is read as the problem it names.

use std::fmt;

use serde::{
  Deserialize, Deserializer, Serialize, Serializer,
  de::{self, MapAccess, Visitor},
};
use serde_json::{Map, Value};

use super::chat::TurnFailure;
use crate::connection::AccountStartProblem;

const KIND: &str = "kind";
const ACCOUNT: &str = "account";
const PROBLEM: &str = "problem";

/// The shape a failure is kept in, checked against [`TurnFailure`] by serde's `remote`.
#[derive(Serialize, Deserialize)]
#[serde(remote = "TurnFailure", tag = "kind", rename_all = "snake_case")]
enum Kept {
  NotSignedIn,
  ConnectionGone,
  Account { problem: AccountStartProblem },
  CannotStart { reason: String },
  Failed { reason: String },
  Interrupted,
  TimeLimit { minutes: u64 },
}

impl Serialize for TurnFailure {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    Kept::serialize(self, serializer)
  }
}

impl<'de> Deserialize<'de> for TurnFailure {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let fields = deserializer.deserialize_map(Fields)?;
    Kept::deserialize(fields).map_err(de::Error::custom)
  }
}

/// A kept failure's keys and values, an account problem kept the old way moved under `problem`.
struct Fields;

impl<'de> Visitor<'de> for Fields {
  type Value = Value;

  fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("a turn failure")
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
    let mut fields = Map::new();
    let mut second_kind = None;
    while let Some(key) = map.next_key::<String>()? {
      let value: Value = map.next_value()?;
      if key == KIND && fields.contains_key(KIND) && second_kind.is_none() {
        second_kind = Some(value);
      } else if fields.contains_key(&key) {
        return Err(de::Error::custom(format_args!("duplicate field `{key}`")));
      } else {
        fields.insert(key, value);
      }
    }
    match second_kind {
      Some(kind) => old_account(fields, kind),
      None => Ok(Value::Object(fields)),
    }
  }
}

/// The failure `fields` of an account problem kept the old way, whose own kind is `kind`, as it is kept now.
fn old_account<E: de::Error>(mut fields: Map<String, Value>, kind: Value) -> Result<Value, E> {
  let outer = fields.remove(KIND);
  if outer.as_ref().and_then(Value::as_str) != Some(ACCOUNT) || fields.contains_key(PROBLEM) {
    return Err(E::duplicate_field(KIND));
  }
  fields.insert(KIND.to_owned(), kind);
  let mut failure = Map::new();
  failure.insert(KIND.to_owned(), Value::from(ACCOUNT));
  failure.insert(PROBLEM.to_owned(), Value::Object(fields));
  Ok(Value::Object(failure))
}
