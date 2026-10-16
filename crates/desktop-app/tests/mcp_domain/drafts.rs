//! Saving a tool's or a profile's draft on the revision the agent read: `get_tool` and `get_profile` name it,
//! a save that gives it as `expected_revision` is refused as `draft_changed` once someone saved the draft
//! meanwhile, and a save without it goes over that change as it always did.

use std::path::Path;

use serde_json::{Value, json};

use super::Served;

/// Kontur seeded empty, its Codex connection's program any file the seed records as checked.
fn served() -> Served {
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  Served::seeded_with("empty", Some(codex)).0
}

/// `args` with `changes` added.
fn with(mut args: Value, changes: Value) -> Value {
  if let (Value::Object(args), Value::Object(changes)) = (&mut args, changes) {
    args.extend(changes);
  }
  args
}

#[test]
fn domain_a_tool_draft_saved_on_its_revision_is_refused_once_someone_saved_it_meanwhile() {
  let mut served = served();
  served.let_automation_widen(true);
  let check = json!({ "project": "Kontur", "tool": "Check" });
  served.ok(
    "save_tool_draft",
    &json!({ "project": "Kontur", "name": "Check", "program": "git", "arguments": ["grep", "--quiet", "hello"] }),
  );
  let seen = served.ok("get_tool", &check)["tool"]["revision"].clone();
  assert!(seen.is_string(), "{seen}");
  // Someone else saves the draft meanwhile, without a revision: it goes over what is stored, as before.
  let meanwhile = served.ok(
    "save_tool_draft",
    &with(check.clone(), json!({ "description": "Theirs." })),
  );
  let theirs = meanwhile["tool"]["revision"].clone();
  assert_ne!(theirs, seen);

  let mine = json!({ "description": "Mine.", "expected_revision": seen });
  let refused = served.refused("save_tool_draft", &with(check.clone(), mine));
  assert_eq!(refused["code"], "draft_changed", "{refused}");
  assert_eq!(served.ok("get_tool", &check)["tool"]["description"], "Theirs.");
  let mine = json!({ "description": "Mine.", "expected_revision": theirs });
  let saved = served.ok("save_tool_draft", &with(check, mine));
  assert_eq!(saved["tool"]["description"], "Mine.");
}

#[test]
fn domain_a_profile_draft_saved_on_its_revision_is_refused_once_someone_saved_it_meanwhile() {
  let mut served = served();
  let builder = json!({ "project": "Kontur", "role": "Builder 2", "connection": "Codex · Kontur", "model": "gpt-5.5" });
  served.ok("save_profile_draft", &builder);
  let named = with(builder, json!({ "profile": "Builder 2" }));
  let get = json!({ "project": "Kontur", "profile": "Builder 2" });
  let seen = served.ok("get_profile", &get)["profile"]["revision"].clone();
  assert!(seen.is_string(), "{seen}");
  let meanwhile = served.ok(
    "save_profile_draft",
    &with(named.clone(), json!({ "responsibilities": "Theirs." })),
  );
  let theirs = meanwhile["profile"]["revision"].clone();
  assert_ne!(theirs, seen);

  let mine = json!({ "responsibilities": "Mine.", "expected_revision": seen });
  let refused = served.refused("save_profile_draft", &with(named.clone(), mine));
  assert_eq!(refused["code"], "draft_changed", "{refused}");
  assert_eq!(served.ok("get_profile", &get)["profile"]["responsibilities"], "Theirs.");
  let mine = json!({ "responsibilities": "Mine.", "expected_revision": theirs });
  let saved = served.ok("save_profile_draft", &with(named, mine));
  assert_eq!(saved["profile"]["responsibilities"], "Mine.");
}
