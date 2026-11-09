//! The import of services and stacks from a checkout (ORC-254): a fixture repository with compose files, package.json
//! workspaces and Cargo binaries gives the expected proposals with their sources; what cannot be mapped is reported,
//! not dropped; and saving chosen proposals makes drafts only, through the workspace and Orchester's MCP server.
//! Run with `cargo test -p orchester-workspace --test service_import service_import`.

mod tools;

use std::{fs, path::Path};

use orchester_core::service::{ReadyCheck, RunData, Service, Stack};
use orchester_workspace::agent_testing::import::{Draft, Proposal, Proposals, read_checkout};
use tempfile::TempDir;

/// Writes `text` to the checkout's file `path`, making its folders.
fn write(root: &Path, path: &str, text: &str) {
  let file = root.join(path);
  fs::create_dir_all(file.parent().expect("a file has a folder")).expect("create the folder");
  fs::write(file, text).expect("write the file");
}

const CLOUD_COMPOSE: &str = r#"
name: cloud
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_PASSWORD: example
      POSTGRES_USER: ${DB_USER}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
    volumes:
      - data:/var/lib/postgresql/data
  api:
    image: ghcr.io/kontur/api:latest
    build: ./api
    command: serve --port 8080
    ports:
      - "8080:8080"
      - "127.0.0.1:9090:9090/tcp"
    environment:
      - RUST_LOG=info
      - API_TOKEN
    depends_on:
      postgres:
        condition: service_healthy
      cache:
        condition: service_started
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/healthz"]
  web:
    build: ../../web
volumes:
  data: {}
"#;

const ROOT_COMPOSE: &str = "services:\n  postgres:\n    image: postgres:16\n    ports: [\"5432:5432\"]\n";

/// A checkout with two compose files, an npm workspace with a pnpm root and a yarn package, and a Cargo workspace.
fn kontur() -> TempDir {
  let repo = TempDir::new().expect("checkout");
  let root = repo.path();
  fs::create_dir(root.join(".git")).expect("create .git");
  write(root, "compose.yaml", ROOT_COMPOSE);
  write(root, "deploy/cloud/compose.yaml", CLOUD_COMPOSE);
  write(
    root,
    "package.json",
    r#"{"name": "kontur", "private": true, "workspaces": ["web/*", "docs"], "packageManager": "pnpm@9.1.0",
        "scripts": {"build": "turbo build", "dev": "turbo dev"}}"#,
  );
  write(
    root,
    "web/cloud/package.json",
    r#"{"name": "@kontur/cloud-web", "scripts": {"build": "vite build", "dev": "vite", "dev:mock": "vite --mode mock",
        "preview": "vite preview"}}"#,
  );
  write(
    root,
    "web/ui/package.json",
    r#"{"name": "@kontur/ui", "scripts": {"test": "vitest"}}"#,
  );
  write(
    root,
    "web/node_modules/vite/package.json",
    r#"{"name": "vite", "scripts": {"dev": "node dev.js"}}"#,
  );
  write(
    root,
    "docs/package.json",
    r#"{"name": "docs", "scripts": {"start": "docusaurus start"}}"#,
  );
  write(root, "docs/yarn.lock", "");
  write(root, "Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
  write(
    root,
    "crates/server/Cargo.toml",
    "[package]\nname = \"kontur-server\"\nversion = \"0.1.0\"\n\n[[bin]]\nname = \"kontur-api\"\npath = \
     \"src/api.rs\"\nrequired-features = [\"http\"]\n",
  );
  write(root, "crates/server/src/main.rs", "fn main() {}\n");
  write(root, "crates/server/src/api.rs", "fn main() {}\n");
  write(root, "crates/server/src/bin/migrate.rs", "fn main() {}\n");
  write(root, "crates/server/src/bin/seed/main.rs", "fn main() {}\n");
  write(
    root,
    "crates/cli/Cargo.toml",
    "[package]\nname = \"kontur-cli\"\nversion = \"0.1.0\"\nautobins = false\n",
  );
  write(root, "crates/cli/src/main.rs", "fn main() {}\n");
  write(
    root,
    "crates/server/tests/fixtures/package.json",
    r#"{"scripts": {"dev": "x"}}"#,
  );
  repo
}

fn read(repo: &TempDir) -> Proposals {
  read_checkout(repo.path()).expect("the checkout is read")
}

fn named<'a>(proposals: &'a Proposals, name: &str) -> &'a Proposal {
  proposals
    .proposals
    .iter()
    .find(|proposal| proposal.draft.name() == name)
    .unwrap_or_else(|| panic!("no proposal is named {name}: {:#?}", names(proposals)))
}

fn service<'a>(proposals: &'a Proposals, name: &str) -> &'a Service {
  match &named(proposals, name).draft {
    Draft::Service(service) => service,
    Draft::Stack(_) => panic!("{name} is a stack"),
  }
}

fn stack<'a>(proposals: &'a Proposals, name: &str) -> &'a Stack {
  match &named(proposals, name).draft {
    Draft::Stack(stack) => stack,
    Draft::Service(_) => panic!("{name} is a service"),
  }
}

fn names(proposals: &Proposals) -> Vec<&str> {
  proposals
    .proposals
    .iter()
    .map(|proposal| proposal.draft.name())
    .collect()
}

fn source(proposal: &Proposal) -> (&str, &str) {
  (&proposal.source.file, &proposal.source.key)
}

/// Each unmapped key of `proposal` with its reason's key.
fn unmapped(proposal: &Proposal) -> Vec<(&str, &str)> {
  proposal
    .unmapped
    .iter()
    .map(|unmapped| (unmapped.key.as_str(), unmapped.reason.key))
    .collect()
}

fn arguments(service: &Service) -> Vec<&str> {
  service.arguments().iter().map(|argument| argument.as_str()).collect()
}

#[test]
fn service_import_proposes_every_source_in_order() {
  let repo = kontur();
  let proposals = read(&repo);
  assert_eq!(
    names(&proposals),
    [
      "postgres",
      "compose",
      "postgres-2",
      "api",
      "cloud",
      "docs-start",
      "kontur-dev",
      "cloud-web-dev",
      "cloud-web-dev-mock",
      "cloud-web-preview",
      "kontur-api",
      "kontur-server",
      "migrate",
      "seed",
    ],
    "compose files, then package scripts, then Cargo binaries; node_modules, tests and a crate without autobins are \
     not read"
  );
  assert!(proposals.unreadable.is_empty(), "{:?}", proposals.unreadable);
  assert!(!proposals.truncated);
  assert!(proposals.proposals.iter().all(|proposal| !proposal.exists));
}

#[test]
fn service_import_maps_compose_services_and_their_stack() {
  let repo = kontur();
  let proposals = read(&repo);
  let postgres = service(&proposals, "postgres-2");
  assert_eq!(
    source(named(&proposals, "postgres-2")),
    ("deploy/cloud/compose.yaml", "services.postgres")
  );
  assert_eq!(postgres.program().as_str(), "postgres:16-alpine");
  assert_eq!(postgres.run_data(), &RunData::DatabaseContainer);
  assert_eq!(postgres.readiness().check(), &ReadyCheck::PortOpen);
  let variables: Vec<&str> = postgres.variables().iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(variables, ["POSTGRES_PASSWORD"]);

  let api = service(&proposals, "api");
  assert_eq!(arguments(api), ["serve", "--port", "8080"]);
  assert_eq!(
    api.needs().iter().map(|need| need.as_str()).collect::<Vec<_>>(),
    ["postgres-2"],
    "a need is named as its proposal is"
  );
  match api.readiness().check() {
    ReadyCheck::Http(path) => assert_eq!(path.as_str(), "/healthz"),
    other => panic!("the healthcheck's URL is the readiness check: {other:?}"),
  }

  let cloud = stack(&proposals, "cloud");
  assert_eq!(
    source(named(&proposals, "cloud")),
    ("deploy/cloud/compose.yaml", "services")
  );
  let services: Vec<&str> = cloud.services().iter().map(|name| name.as_str()).collect();
  assert_eq!(services, ["postgres-2", "api"]);
  let root = stack(&proposals, "compose");
  assert_eq!(
    root.services().iter().map(|name| name.as_str()).collect::<Vec<_>>(),
    ["postgres"]
  );

  let skipped: Vec<(&str, &str, &str)> = proposals
    .skipped
    .iter()
    .map(|skipped| {
      (
        skipped.source.file.as_str(),
        skipped.source.key.as_str(),
        skipped.reason.key,
      )
    })
    .collect();
  assert_eq!(
    skipped,
    [(
      "deploy/cloud/compose.yaml",
      "services.web",
      "services.import.skipped.builds"
    )],
    "a service Compose builds is passed over with why"
  );
}

#[test]
fn service_import_reports_what_compose_cannot_map() {
  let repo = kontur();
  let proposals = read(&repo);
  assert_eq!(
    unmapped(named(&proposals, "postgres-2")),
    [
      (
        "services.postgres.environment.POSTGRES_USER",
        "services.import.unmapped.environment"
      ),
      (
        "services.postgres.healthcheck.interval",
        "services.import.unmapped.healthTiming"
      ),
      (
        "services.postgres.healthcheck.test",
        "services.import.unmapped.healthcheck"
      ),
      ("services.postgres.volumes", "services.import.unmapped.field"),
    ]
  );
  assert_eq!(
    unmapped(named(&proposals, "api")),
    [
      ("services.api.build", "services.import.unmapped.build"),
      (
        "services.api.environment.API_TOKEN",
        "services.import.unmapped.environment"
      ),
      ("services.api.depends_on.cache", "services.import.unmapped.need"),
      ("services.api.ports", "services.import.unmapped.ports"),
    ]
  );
  let ports = &named(&proposals, "api").unmapped[3].reason;
  assert_eq!(ports.args, [("ports", "8080, 9090".to_owned())]);
  assert_eq!(
    unmapped(named(&proposals, "cloud")),
    [("volumes", "services.import.unmapped.field")]
  );
  assert!(
    named(&proposals, "postgres").unmapped.is_empty(),
    "one published port maps"
  );
}

#[test]
fn service_import_runs_package_scripts_through_their_workspaces_manager() {
  let repo = kontur();
  let proposals = read(&repo);
  let cases = [
    ("kontur-dev", "package.json", "scripts.dev", "pnpm", "dev", ""),
    (
      "cloud-web-dev",
      "web/cloud/package.json",
      "scripts.dev",
      "pnpm",
      "dev",
      "web/cloud",
    ),
    (
      "cloud-web-dev-mock",
      "web/cloud/package.json",
      "scripts.dev:mock",
      "pnpm",
      "dev:mock",
      "web/cloud",
    ),
    (
      "cloud-web-preview",
      "web/cloud/package.json",
      "scripts.preview",
      "pnpm",
      "preview",
      "web/cloud",
    ),
    (
      "docs-start",
      "docs/package.json",
      "scripts.start",
      "yarn",
      "start",
      "docs",
    ),
  ];
  for (name, file, key, manager, script, folder) in cases {
    let proposal = named(&proposals, name);
    assert_eq!(source(proposal), (file, key), "{name}");
    let service = service(&proposals, name);
    assert_eq!(service.program().as_str(), manager, "{name}");
    assert_eq!(arguments(service), ["run", script], "{name}");
    assert_eq!(service.working_folder().as_str(), folder, "{name}");
    let owned: Vec<&str> = service.owned_paths().iter().map(|path| path.as_str()).collect();
    assert_eq!(owned, if folder.is_empty() { vec![] } else { vec![folder] }, "{name}");
    assert!(matches!(service.readiness().check(), ReadyCheck::LogLine(_)), "{name}");
    assert_eq!(unmapped(proposal), [(key, "services.import.unmapped.port")], "{name}");
  }
}

#[test]
fn service_import_runs_cargo_binaries_with_the_features_they_require() {
  let repo = kontur();
  let proposals = read(&repo);
  let cases: [(&str, &str, &[&str]); 4] = [
    (
      "kontur-api",
      "bin.kontur-api",
      &["run", "--bin", "kontur-api", "--features", "http"],
    ),
    ("kontur-server", "src/main.rs", &["run", "--bin", "kontur-server"]),
    ("migrate", "src/bin/migrate.rs", &["run", "--bin", "migrate"]),
    ("seed", "src/bin/seed/main.rs", &["run", "--bin", "seed"]),
  ];
  for (name, key, expected) in cases {
    let proposal = named(&proposals, name);
    assert_eq!(source(proposal), ("crates/server/Cargo.toml", key), "{name}");
    let service = service(&proposals, name);
    assert_eq!(service.program().as_str(), "cargo", "{name}");
    assert_eq!(arguments(service), expected, "{name}");
    assert_eq!(service.working_folder().as_str(), "crates/server", "{name}");
    assert_eq!(service.readiness().check(), &ReadyCheck::PortOpen, "{name}");
    assert_eq!(unmapped(proposal), [(key, "services.import.unmapped.port")], "{name}");
  }
}

#[test]
fn service_import_says_which_files_it_could_not_read() {
  let repo = TempDir::new().expect("checkout");
  let root = repo.path();
  write(root, "package.json", "{ not json");
  write(root, "crates/broken/Cargo.toml", "[package\nname = 1");
  write(root, "crates/nameless/Cargo.toml", "[package]\nversion = \"0.1.0\"\n");
  write(root, "compose.yml", "services: [unclosed");
  let bomb = "a: &a [x, x, x, x, x, x, x, x, x, x]\nb: &b [*a, *a, *a, *a, *a, *a, *a, *a, *a, *a]\nc: &c [*b, *b, *b, \
              *b, *b, *b, *b, *b, *b, *b]\nd: &d [*c, *c, *c, *c, *c, *c, *c, *c, *c, *c]\ne: &e [*d, *d, *d, *d, *d, \
              *d, *d, *d, *d, *d]\nservices: {}\n";
  write(root, "deploy/compose.yaml", bomb);
  write(root, "deploy/empty/compose.yaml", "name: empty\n");
  let proposals = read(&repo);
  assert!(proposals.proposals.is_empty(), "{:?}", names(&proposals));
  let unreadable: Vec<(&str, &str)> = proposals
    .unreadable
    .iter()
    .map(|unreadable| (unreadable.file.as_str(), unreadable.reason.key))
    .collect();
  assert_eq!(
    unreadable,
    [
      ("compose.yml", "services.import.unreadable.yaml"),
      ("deploy/compose.yaml", "services.import.unreadable.yamlTooLarge"),
      ("deploy/empty/compose.yaml", "services.import.unreadable.noServices"),
      ("package.json", "services.import.unreadable.json"),
      ("crates/broken/Cargo.toml", "services.import.unreadable.toml"),
      ("crates/nameless/Cargo.toml", "services.import.unreadable.cargoName"),
    ]
  );
}
