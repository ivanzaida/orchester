//! An archived local profile's panel (`Agents · Archived resource · Restore` L14 `a6ZLLg`): what it was, as values
//! read-only until it is restored, and who used it, beside the notice and *Restore*. `cargo test -p orchester-app
//! --test agents local_hub_archived`.

use orchester_app::routes::{HubFilter, HubView};
use orchester_core::time::Timestamp;
use orchester_store::ProjectDb;

use super::{actions::assigned_pipeline, harness::Agents, local_hub::open};

#[test]
fn local_hub_archived_a_profile_shows_what_it_was_and_who_used_it() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  assigned_pipeline(&agents, builder);
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(builder, Timestamp::now())
    .expect("archived");
  open(&mut agents, HubView::new(HubFilter::Archived, Some(builder)));
  agents.until("its panel", |s| s.exists("local-hub-archived-notice"));

  assert!(
    agents.exists("local-hub-archived-read-only"),
    "it says the values wait for Restore"
  );
  assert_eq!(agents.text("local-hub-archived-role-value"), "Builder");
  assert_eq!(agents.text("local-hub-archived-model-value"), "test-model");
  assert_eq!(agents.text("local-hub-archived-effort-value"), "CLI default");
  assert_eq!(agents.text("local-hub-archived-mcp-value"), "None");
  assert_eq!(agents.text("local-hub-archived-tools-value"), "None");
  assert_eq!(agents.text("local-hub-chips-end"), "was used by Delivery");
  assert!(agents.exists("archive-restore"));
}
