//! This computer's bindings of a shared project's references (`docs/architecture/server.md`, "Desktop data", S5): per
//! connection, MCP server or secret a shared version names by its id on the publisher's computer, the local one it
//! means here, and how it was bound.

use orchester_core::{
  id::{ConnectionId, SecretId},
  time::Timestamp,
};
use rusqlite::{Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// How a reference was bound to a local connection, MCP server or secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundBy {
  /// A local one of the same kind has its name.
  Name,
  /// A local one has its role, the connection's or the secret's kind.
  Role,
  /// A person chose it.
  Person,
}

/// The local one a reference means here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bound<I> {
  pub local: I,
  pub by: BoundBy,
}

/// A reference by its id on the publisher's computer, with what it is bound to here; `None` while it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundReference {
  Connection {
    reference: ConnectionId,
    bound: Option<Bound<ConnectionId>>,
  },
  McpServer {
    reference: ConnectionId,
    bound: Option<Bound<ConnectionId>>,
  },
  Secret {
    reference: SecretId,
    bound: Option<Bound<SecretId>>,
  },
}

/// A reference's binding on this computer, with the name and role the reference carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceBinding {
  pub reference: BoundReference,
  pub name: String,
  pub role: Option<String>,
  pub changed_at: Timestamp,
}

impl ProjectDb {
  /// Keeps `binding` as the one of its reference, in place of any it had.
  pub fn set_reference_binding(&mut self, binding: &ReferenceBinding) -> Result<(), StoreError> {
    let (kind, reference, bound) = match binding.reference {
      BoundReference::Connection { reference, bound } => ("connection", reference.to_string(), columns(bound)),
      BoundReference::McpServer { reference, bound } => ("mcp_server", reference.to_string(), columns(bound)),
      BoundReference::Secret { reference, bound } => ("secret", reference.to_string(), columns(bound)),
    };
    let (local, by) = bound.unzip();
    self.connection.execute(
      "INSERT INTO reference_bindings (kind, reference_id, name, role, local_id, bound_by, changed_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (kind, reference_id) DO UPDATE SET name = excluded.name, \
       role = excluded.role, local_id = excluded.local_id, bound_by = excluded.bound_by, \
       changed_at = excluded.changed_at",
      params![
        kind,
        reference,
        binding.name,
        binding.role,
        local,
        by,
        binding.changed_at.unix_millis()
      ],
    )?;
    Ok(())
  }

  /// Every reference this computer has a binding row for, by kind and reference id.
  pub fn reference_bindings(&self) -> Result<Vec<ReferenceBinding>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT kind, reference_id, name, role, local_id, bound_by, changed_at FROM reference_bindings \
       ORDER BY kind, reference_id",
    )?;
    let rows = statement
      .query_map([], BindingRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(BindingRow::into_binding).collect()
  }
}

/// The `local_id` and `bound_by` columns of a binding.
fn columns<I: ToString>(bound: Option<Bound<I>>) -> Option<(String, &'static str)> {
  bound.map(|bound| (bound.local.to_string(), by_text(bound.by)))
}

const fn by_text(by: BoundBy) -> &'static str {
  match by {
    BoundBy::Name => "name",
    BoundBy::Role => "role",
    BoundBy::Person => "person",
  }
}

struct BindingRow {
  kind: String,
  reference: String,
  name: String,
  role: Option<String>,
  local: Option<String>,
  by: Option<String>,
  changed_at: i64,
}

impl BindingRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      kind: row.get(0)?,
      reference: row.get(1)?,
      name: row.get(2)?,
      role: row.get(3)?,
      local: row.get(4)?,
      by: row.get(5)?,
      changed_at: row.get(6)?,
    })
  }

  fn into_binding(self) -> Result<ReferenceBinding, StoreError> {
    let reference = match self.kind.as_str() {
      "connection" => BoundReference::Connection {
        reference: parse("reference id", &self.reference)?,
        bound: self.bound()?,
      },
      "mcp_server" => BoundReference::McpServer {
        reference: parse("reference id", &self.reference)?,
        bound: self.bound()?,
      },
      "secret" => BoundReference::Secret {
        reference: parse("reference id", &self.reference)?,
        bound: self.bound()?,
      },
      _ => return Err(corrupt("reference kind", &self.kind)),
    };
    Ok(ReferenceBinding {
      reference,
      name: self.name,
      role: self.role,
      changed_at: Timestamp::from_unix_millis(self.changed_at),
    })
  }

  fn bound<I>(&self) -> Result<Option<Bound<I>>, StoreError>
  where
    I: std::str::FromStr,
    I::Err: std::fmt::Display,
  {
    let (Some(local), Some(by)) = (&self.local, &self.by) else {
      return Ok(None);
    };
    let by = match by.as_str() {
      "name" => BoundBy::Name,
      "role" => BoundBy::Role,
      "person" => BoundBy::Person,
      other => return Err(corrupt("binding kind", other)),
    };
    Ok(Some(Bound {
      local: parse("bound id", local)?,
      by,
    }))
  }
}
