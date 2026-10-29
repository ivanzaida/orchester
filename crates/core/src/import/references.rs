//! The ids a body refers to, and the body with them replaced. Bodies refer to resources,
//! connections and secrets by id only (see [`crate::resource`]), and ids are UUIDs no two
//! entities share, so a JSON string equal to the id of an entity of the source project is a
//! reference to that entity. Working on the JSON keeps this independent of each kind's shape;
//! the rewritten body goes back through the domain constructors, which check it again.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use uuid::Uuid;

/// Every UUID that is a string value of `value`, in the order they appear, each once. Object
/// keys are not references: a map keyed by ids (a tool's allowed pipelines) holds settings per
/// entity, not something the body needs.
pub(crate) fn values(value: &Value) -> Vec<Uuid> {
  let mut found = Vec::new();
  collect(value, &mut found);
  let mut seen = HashSet::new();
  found.retain(|uuid| seen.insert(*uuid));
  found
}

fn collect(value: &Value, found: &mut Vec<Uuid>) {
  match value {
    Value::String(text) => found.extend(uuid(text)),
    Value::Array(items) => items.iter().for_each(|item| collect(item, found)),
    Value::Object(fields) => fields.values().for_each(|field| collect(field, found)),
    Value::Null | Value::Bool(_) | Value::Number(_) => {}
  }
}

/// How the ids of source entities become ids of the target.
pub(crate) struct Remap {
  /// Every resolved reference: to a copy, or to the target's own resource, connection or secret.
  pub values: HashMap<Uuid, Uuid>,
  /// Source resources copied by this import, to their copies. Settings keyed by an entity (a
  /// tool's allowed pipelines) follow only a copy: an import never grants the target's own
  /// resources anything.
  pub keys: HashMap<Uuid, Uuid>,
  /// Every entity of the source project.
  pub known: HashSet<Uuid>,
}

impl Remap {
  /// `value` with its references replaced. A key naming a source entity that was not copied is
  /// removed with its entry. A value without a replacement is kept, an unresolved reference
  /// included.
  pub(crate) fn rewrite(&self, value: Value) -> Value {
    match value {
      Value::String(text) => match uuid(&text).and_then(|id| self.values.get(&id)) {
        Some(replacement) => Value::String(replacement.to_string()),
        None => Value::String(text),
      },
      Value::Array(items) => Value::Array(items.into_iter().map(|item| self.rewrite(item)).collect()),
      Value::Object(fields) => Value::Object(
        fields
          .into_iter()
          .filter_map(|(key, field)| {
            let key = match uuid(&key) {
              Some(id) if self.keys.contains_key(&id) => self.keys[&id].to_string(),
              Some(id) if self.known.contains(&id) => return None,
              _ => key,
            };
            Some((key, self.rewrite(field)))
          })
          .collect(),
      ),
      other @ (Value::Null | Value::Bool(_) | Value::Number(_)) => other,
    }
  }
}

/// The UUID `text` is, in the hyphenated form ids serialize to.
fn uuid(text: &str) -> Option<Uuid> {
  const HYPHENATED_LEN: usize = 36;
  if text.len() == HYPHENATED_LEN {
    Uuid::try_parse(text).ok()
  } else {
    None
  }
}
