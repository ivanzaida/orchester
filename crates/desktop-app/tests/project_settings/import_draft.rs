//! Project settings › Services › Import draft (`05 · Import draft` `DFWRJ`, ORC-264): the proposals the checkout's
//! compose file and `package.json` give, grouped by their file, each with what was found, what could not be carried
//! and Keep or Skip; what cannot be proposed shows Skip with why; Save makes drafts of the kept ones only and returns
//! to the tab. Run with `cargo test -p orchester-app --test project_settings import_draft`.

use std::{fs, path::Path};

use orchester_core::{resource::ResourceKind, store::ResourceStore};

use super::harness::Settings;

const COMPOSE: &str = "name: cloud
services:
  postgres:
    image: postgres:16
    volumes:
      - data:/var/lib/postgresql/data
  api:
    image: ghcr.io/kontur/api:latest
    command: serve --port 8080
    depends_on:
      - postgres
  web:
    build: ./web
";

const PACKAGE: &str = r#"{"name": "kontur-web", "scripts": {"dev": "vite", "build": "vite build"}}"#;

fn write(root: &Path, path: &str, text: &str) {
  let file = root.join(path);
  fs::create_dir_all(file.parent().expect("a file has a folder")).expect("create the folder");
  fs::write(file, text).expect("write the file");
}

/// Import draft on a checkout with a compose file and a `package.json`.
fn opened() -> Settings {
  let mut settings = Settings::new(false);
  write(&settings.repo, "deploy/compose.yaml", COMPOSE);
  write(&settings.repo, "web/package.json", PACKAGE);
  settings.tab("services", "settings-services-empty");
  settings.click("settings-services-import");
  settings.until("the proposals", |s| s.exists("settings-import-file-0"));
  settings
}

fn slugs(settings: &Settings, kind: ResourceKind) -> Vec<String> {
  let mut slugs: Vec<String> = settings
    .db()
    .slugs(kind)
    .expect("slugs")
    .iter()
    .map(|slug| slug.as_str().to_owned())
    .collect();
  slugs.sort();
  slugs
}

#[test]
fn import_draft_shows_the_proposals_by_their_file() {
  let mut settings = opened();
  assert_eq!(settings.text("project-settings-save-status"), "Not saved yet");
  assert_eq!(settings.text("settings-import-crumb-here"), "Import draft");
  assert_eq!(settings.text("settings-import-head-title"), "Import a first draft");
  assert_eq!(settings.text("settings-import-file-0-title"), "deploy/compose.yaml");
  assert_eq!(settings.text("settings-import-file-0-count"), "3 of 4 kept");
  assert_eq!(settings.text("settings-import-file-1-title"), "web/package.json");
  assert_eq!(settings.text("settings-import-service-postgres-kind-label"), "service");
  assert_eq!(
    settings.text("settings-import-service-postgres-hint"),
    "image postgres:16 → a fresh database container per run"
  );
  assert_eq!(
    settings.text("settings-import-service-postgres-unmapped-0-key"),
    "services.postgres.volumes"
  );
  assert_eq!(
    settings.text("settings-import-service-api-hint"),
    "image ghcr.io/kontur/api:latest → a fresh database container per run · needs postgres"
  );
  assert_eq!(settings.text("settings-import-stack-cloud-kind-label"), "stack");
  assert_eq!(
    settings.text("settings-import-stack-cloud-hint"),
    "starts postgres, api"
  );
  assert_eq!(settings.text("settings-import-file-0-skipped-0-label"), "services.web");
  assert!(settings.exists("settings-import-file-0-skipped-0-choice"));
  assert_eq!(
    settings.text("settings-import-service-kontur-web-dev-hint"),
    "scripts.dev → npm run dev"
  );
  assert_eq!(
    settings.text("settings-import-tally"),
    "4 kept · 1 skipped · each new service asks for approval on its first start"
  );
  assert_eq!(settings.text("settings-import-save"), "Save 4 drafts");
}

#[test]
fn import_draft_saves_only_the_kept_proposals_as_drafts() {
  let mut settings = opened();
  settings.click("settings-import-service-api-skip");
  settings.click("settings-import-service-kontur-web-dev-skip");
  settings.until("the tally", |s| {
    s.text("settings-import-tally").starts_with("2 kept · 3 skipped")
  });
  assert_eq!(settings.text("settings-import-file-0-count"), "2 of 4 kept");
  assert_eq!(settings.text("settings-import-save"), "Save 2 drafts");
  settings.click("settings-import-service-kontur-web-dev-keep");
  settings.until("kept again", |s| s.text("settings-import-save") == "Save 3 drafts");

  settings.click("settings-import-save");
  settings.until("back on Services", |s| s.exists("settings-service-postgres-row"));
  assert_eq!(slugs(&settings, ResourceKind::Service), ["kontur-web-dev", "postgres"]);
  assert_eq!(slugs(&settings, ResourceKind::Stack), ["cloud"]);
  assert_eq!(
    settings.text("settings-service-postgres-approval-label"),
    "Not published",
    "an imported service is a draft"
  );
  assert!(
    !settings.exists("settings-service-api-row"),
    "a skipped proposal is not saved"
  );
}

#[test]
fn import_draft_cancel_saves_nothing() {
  let mut settings = opened();
  settings.click("settings-import-cancel");
  settings.until("back on Services", |s| s.exists("settings-services-empty"));
  assert!(slugs(&settings, ResourceKind::Service).is_empty());
  assert!(slugs(&settings, ResourceKind::Stack).is_empty());
}
