//! Forward-only migrations of `orchester-server.db`: 1 (S1a accounts), 2 (S1b sharing), 3 (API 3: no passwords,
//! codes, invites, the cloud link, projects by slug), 4 (the server admin), 5 (two-factor sessions), 6 (email tags of
//! accounts, against repeated invites), 7 (modules), 8 (the cloud's former issuer).

use orchester_core::{name::Name, repository::RepositoryIdentity, resource::Slug};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{error::ServerError, server_keys::ServerKeys, sharing::projects::RESERVED_SLUGS};

pub(super) fn migrate(connection: &mut Connection, now: i64) -> Result<(), ServerError> {
  // Rebuilding `projects` drops the table other tables refer to, which SQLite allows only with foreign keys off;
  // the pragma is a no-op inside a transaction, so it is set around it and the keys are checked before commit.
  connection.pragma_update(None, "foreign_keys", false)?;
  let result = migrate_unchecked(connection, now);
  connection.pragma_update(None, "foreign_keys", true)?;
  result
}

fn migrate_unchecked(connection: &mut Connection, now: i64) -> Result<(), ServerError> {
  // The immediate transaction also serializes migrations with a host CLI command.
  let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
  let exists: bool = transaction.query_row(
    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='meta')",
    [],
    |row| row.get(0),
  )?;
  let version: i64 = if exists {
    transaction.query_row("SELECT schema_version FROM meta", [], |row| row.get(0))?
  } else {
    transaction.execute_batch(include_str!("schema.sql"))?;
    1
  };
  if !(1..=8).contains(&version) {
    return Err(ServerError::Configuration("unsupported server database schema"));
  }
  if version == 1 {
    transaction.execute_batch(include_str!("sharing.sql"))?;
  }
  // Review builds of schema 2 predate this retention index; opening them must also install it.
  transaction.execute_batch("CREATE INDEX IF NOT EXISTS events_project_retention ON events(project,at)")?;
  if version < 3 {
    api3(&transaction, now)?;
  }
  if version < 4 {
    transaction.execute_batch(include_str!("admin.sql"))?;
  }
  if version < 5 {
    transaction.execute_batch(include_str!("two_factor.sql"))?;
  }
  if version < 6 {
    transaction.execute_batch(include_str!("email_tags.sql"))?;
  }
  if version < 7 {
    modules(&transaction)?;
  }
  if version < 8 {
    transaction.execute_batch(include_str!("cloud_issuer.sql"))?;
  }
  ServerKeys::own_id(&transaction)?;
  free_reserved_slugs(&transaction)?;
  let broken: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)", [], |row| {
    row.get(0)
  })?;
  if broken {
    return Err(ServerError::StoredData);
  }
  transaction.commit()?;
  Ok(())
}

fn api3(transaction: &Transaction<'_>, now: i64) -> Result<(), ServerError> {
  transaction.execute(
    "UPDATE sessions SET revoked_at=?1 WHERE revoked_at IS NULL AND (initial_password=1
      OR user_id IN (SELECT id FROM users WHERE must_change_password=1))",
    [now],
  )?;
  projects_by_slug(transaction)?;
  transaction.execute_batch(include_str!("api3.sql"))?;
  ServerKeys::create(transaction)
}

/// Schema 7. Rebuilding `events` for the `modules` kind would let its sequence start again below the last pruned event,
/// behind the cursors the desktops hold; the counter is carried over so event ids keep growing.
fn modules(transaction: &Transaction<'_>) -> Result<(), ServerError> {
  let counter: Option<i64> = transaction
    .query_row("SELECT seq FROM sqlite_sequence WHERE name='events'", [], |row| {
      row.get(0)
    })
    .optional()?;
  transaction.execute_batch(include_str!("modules.sql"))?;
  if let Some(counter) = counter {
    transaction.execute("DELETE FROM sqlite_sequence WHERE name='events'", [])?;
    transaction.execute(
      "INSERT INTO sqlite_sequence(name,seq) VALUES ('events',MAX(?1,COALESCE((SELECT MAX(sequence) FROM events),0)))",
      [counter],
    )?;
  }
  Ok(())
}

/// Projects made under row 10 were identified by their repository; each gets a slug from its repository's name
/// (`github.com/acme/kontur` → `kontur`, then `kontur-2` …) and keeps the repository as its hint and its id.
fn projects_by_slug(transaction: &Transaction<'_>) -> Result<(), ServerError> {
  transaction.execute_batch(
    "CREATE TABLE projects_api3 (
      id TEXT PRIMARY KEY, key TEXT NOT NULL, name TEXT NOT NULL, slug TEXT NOT NULL UNIQUE, remote_hint TEXT,
      next_task_number INTEGER NOT NULL CHECK(next_task_number BETWEEN 1 AND 4294967296),
      created_by TEXT NOT NULL REFERENCES users(id), created_at INTEGER NOT NULL,
      event_floor INTEGER NOT NULL DEFAULT 0
    )",
  )?;
  let mut query = transaction.prepare("SELECT id,remote FROM projects ORDER BY created_at,id")?;
  let projects = query
    .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
    .collect::<Result<Vec<_>, _>>()?;
  let mut taken = reserved_slugs();
  for (id, remote) in projects {
    let slug = slug_for(&remote, &taken);
    transaction.execute(
      "INSERT INTO projects_api3(id,key,name,slug,remote_hint,next_task_number,created_by,created_at,event_floor)
        SELECT id,key,name,?2,remote,next_task_number,created_by,created_at,event_floor FROM projects WHERE id=?1",
      params![id, slug.as_str()],
    )?;
    taken.push(slug);
  }
  transaction.execute_batch("DROP TABLE projects; ALTER TABLE projects_api3 RENAME TO projects;")?;
  Ok(())
}

/// The admin answers `/admin`, the API `/api` and `/auth`: a project an older server let take one of those slugs gets
/// the next free one by the rule above (`admin` → `admin-2`), and the host's log says so (security review 1, minor 6).
/// Checked at every open, so it holds however the database came to have one.
fn free_reserved_slugs(transaction: &Transaction<'_>) -> Result<(), ServerError> {
  let mut query = transaction.prepare("SELECT id,slug FROM projects ORDER BY created_at,id")?;
  let projects = query
    .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
    .collect::<Result<Vec<_>, _>>()?;
  let mut taken = reserved_slugs();
  for (_, slug) in &projects {
    taken.push(Slug::new(slug).map_err(|_| ServerError::StoredData)?);
  }
  let fallback = Slug::new("project").expect("`project` is a slug");
  for (id, slug) in projects {
    if !RESERVED_SLUGS.contains(&slug.as_str()) {
      continue;
    }
    let name = Name::new(&slug).map_err(|_| ServerError::StoredData)?;
    let renamed = Slug::unique_for(&name, &fallback, &taken);
    transaction.execute("UPDATE projects SET slug=?2 WHERE id=?1", params![id, renamed.as_str()])?;
    tracing::warn!(project = %id, from = %slug, to = %renamed.as_str(), "a project's slug is reserved for the server's own paths; renamed");
    taken.push(renamed);
  }
  Ok(())
}

fn reserved_slugs() -> Vec<Slug> {
  RESERVED_SLUGS
    .iter()
    .map(|slug| Slug::new(slug).expect("reserved slugs are slugs"))
    .collect()
}

fn slug_for(remote: &str, taken: &[Slug]) -> Slug {
  let fallback = Slug::new("project").expect("`project` is a slug");
  let segment = RepositoryIdentity::parse(remote)
    .ok()
    .and_then(|identity| identity.as_str().rsplit('/').next().map(str::to_owned))
    .unwrap_or_default();
  let name = Name::new(&segment).unwrap_or_else(|_| Name::new("project").expect("`project` is a name"));
  Slug::unique_for(&name, &fallback, taken)
}
