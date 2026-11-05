//! A shared project's releases on the team-authority surface (`docs/cloud-api.md`, "Team authority releases"): the
//! shapes of the definitions, the history, the changelog, the module's horizon, the reads, the filter, the scopes and
//! the refusals, each refusing unknown fields ([`shapes`], [`reads`]); and the stand-in team server accepting and
//! answering them ([`stand_in`]).

mod reads;
mod shapes;
mod stand_in;

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(value: &T, wire: &Value) {
  assert_eq!(&serde_json::to_value(value).expect("serialize"), wire);
  assert_eq!(&serde_json::from_value::<T>(wire.clone()).expect("read"), value);
}

/// `wire` with an unknown field is refused, and so is `wire` without `field`.
fn refuses_unknown_and_missing<T: for<'de> Deserialize<'de>>(wire: &Value, field: &str) {
  let mut unknown = wire.clone();
  unknown["extra"] = Value::Bool(true);
  assert!(
    serde_json::from_value::<T>(unknown).is_err(),
    "an unknown field is refused"
  );
  let mut missing = wire.clone();
  missing.as_object_mut().expect("an object").remove(field);
  assert!(serde_json::from_value::<T>(missing).is_err(), "`{field}` is required");
}
