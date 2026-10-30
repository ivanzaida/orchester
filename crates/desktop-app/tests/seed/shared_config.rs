//! `shared-config` and its variants read back as Sharing › On this computer and a shared profile's page read them
//! (`Here::read`, `sync::waiting`): every kind of reference and standing, Reviewer's v5 waiting with what it adds, and
//! Builder's connection here.
//! `cargo test -p orchester-app --test seed seed_shared_config`.

use orchester_core::{
  id::ResourceId,
  resource::ResourceKind,
  sharing::{MemberRole, VersionWidening},
  store::ResourceStore,
};
use orchester_store::{BoundBy, ProjectDb};
use orchester_workspace::sharing::sync::{self, Here, Standing};

use super::Seed;

/// How each reference stands, by name, in the order the page lists them.
fn standings(here: &Here) -> Vec<(&str, &str)> {
  here
    .references
    .iter()
    .map(|reference| {
      let standing = match &reference.standing {
        Standing::Bound { by: BoundBy::Name, .. } => "by name",
        Standing::Bound { by: BoundBy::Role, .. } => "by role",
        Standing::Bound {
          by: BoundBy::Person, ..
        } => "chosen",
        Standing::Found { .. } => "found",
        Standing::Missing => "missing",
      };
      (reference.binding.name.as_str(), standing)
    })
    .collect()
}

fn profile(db: &ProjectDb, named: &str) -> ResourceId {
  db.resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .iter()
    .find(|resource| resource.name().as_str() == named)
    .expect("the profile")
    .id()
}

#[test]
fn seed_shared_config_names_every_kind_of_reference_and_a_version_that_widens() {
  let seed = Seed::of("shared-config");
  let here = Here::read(seed.repo()).expect("read");
  assert_eq!(
    standings(&here),
    [
      ("Work", "by name"),
      ("Default", "by role"),
      ("DeepSeek", "missing"),
      ("github", "found"),
      ("linear", "missing"),
      ("STAGING_DB_URL", "chosen"),
      ("SENTRY_TOKEN", "missing"),
    ],
    "each kind in the order the publisher made them (04b)"
  );
  assert_eq!(here.needs_you(), 3, "04b: 3 need you");
  assert_eq!(
    here.connections_of("openai_api").len(),
    2,
    "DeepSeek and OpenRouter to choose"
  );
  assert_eq!(
    here.connections_of("claude_code_cli").len(),
    2,
    "two Claude Code accounts"
  );
  assert_eq!(
    here.secrets_of(Some("access_token")).len(),
    1,
    "a secret to give SENTRY_TOKEN"
  );

  let db = seed.db();
  assert_eq!(
    db.sharing().expect("link").expect("shared").role,
    Some(MemberRole::Member)
  );
  let builder = profile(&db, "Builder");
  let work = here.connection_of(builder).expect("Builder's connection");
  assert!(matches!(&work.standing, Standing::Bound { local, .. } if local == "Claude Code · Work"));
  let reviewer = profile(&db, "Reviewer");
  let waiting = sync::waiting(seed.repo(), reviewer).expect("read").expect("v5 waits");
  assert_eq!(
    (waiting.number.get(), waiting.used.map(|used| used.get())),
    (5, Some(4))
  );
  assert!(waiting.adds.contains(&VersionWidening::McpServer), "{:?}", waiting.adds);
  assert_eq!(
    waiting.adds.len(),
    2,
    "an MCP server and network (02d): {:?}",
    waiting.adds
  );
  let added: Vec<_> = here
    .added_in(reviewer, waiting.number, waiting.used)
    .iter()
    .map(|reference| reference.binding.name.as_str())
    .collect();
  assert_eq!(added, ["github"], "v5 names github, v4 doesn't");
  assert!(
    here.missing_for(reviewer).is_empty(),
    "github waits for Confirm…, it isn't missing"
  );
  let explorer = profile(&db, "Explorer");
  assert_eq!(here.missing_for(explorer).len(), 1, "Explorer can't run here");
}

#[test]
fn seed_shared_config_variants_choose_work_here_or_have_no_claude_code() {
  let chosen = Here::read(Seed::of("shared-config-chosen").repo()).expect("read");
  assert!(standings(&chosen).contains(&("Work", "chosen")));
  let none = Here::read(Seed::of("shared-config-no-claude").repo()).expect("read");
  assert!(standings(&none).contains(&("Work", "missing")));
  assert!(none.connections_of("claude_code_cli").is_empty());
  assert_eq!(none.needs_you(), chosen.needs_you() + 1);
}
