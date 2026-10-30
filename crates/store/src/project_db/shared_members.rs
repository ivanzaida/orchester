//! The members of a shared project as the server last listed them (`docs/architecture/server.md`, row 63): the app
//! shows them read-only, and while the server cannot be reached, as of when they were read. Make local forgets them.

use orchester_core::{id::UserId, sharing::MemberRole, time::Timestamp};
use rusqlite::{OptionalExtension, params};

use super::{
  ProjectDb,
  sharing::{parse_role, role_text},
};
use crate::{StoreError, rows::parse};

/// A member of the server project, as last read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMember {
  pub user: UserId,
  pub display_name: String,
  pub username: String,
  pub role: MemberRole,
  pub added_at: Timestamp,
}

/// One of the server's admins, as the members list last named them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerAdmin {
  pub user: UserId,
  pub display_name: String,
  pub username: String,
}

impl ProjectDb {
  /// The repository the server project was set up for as last read (`None` inside: it names none), and when; `None`
  /// until it has been read.
  pub fn remote_hint(&self) -> Result<Option<(Option<String>, Timestamp)>, StoreError> {
    let read: Option<(Option<String>, Option<i64>)> = self
      .connection
      .query_row("SELECT remote_hint, remote_hint_read_at FROM sharing", [], |row| {
        Ok((row.get(0)?, row.get(1)?))
      })
      .optional()?;
    Ok(read.and_then(|(hint, at)| at.map(|at| (hint, Timestamp::from_unix_millis(at)))))
  }

  /// Records the repository the server project was set up for, as the server answered at `read_at`.
  pub fn record_remote_hint(&mut self, hint: Option<&str>, read_at: Timestamp) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE sharing SET remote_hint = ?1, remote_hint_read_at = ?2",
      params![hint, read_at.unix_millis()],
    )?;
    if updated == 0 {
      return Err(StoreError::NotShared);
    }
    Ok(())
  }

  /// The members as last read, owners first and then by when they were added, and when that was; `None` until they have
  /// been read.
  pub fn shared_members(&self) -> Result<Option<(Timestamp, Vec<SharedMember>)>, StoreError> {
    let read_at: Option<i64> = self
      .connection
      .query_row("SELECT members_read_at FROM sharing", [], |row| row.get(0))
      .optional()?
      .flatten();
    let Some(read_at) = read_at else {
      return Ok(None);
    };
    let mut statement = self.connection.prepare(
      "SELECT user_id, display_name, username, role, added_at FROM shared_members ORDER BY role <> 'owner', added_at, display_name",
    )?;
    let rows = statement.query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, i64>(4)?,
      ))
    })?;
    let mut members = Vec::new();
    for row in rows {
      let (user, display_name, username, role, added_at) = row?;
      members.push(SharedMember {
        user: parse("member id", &user)?,
        display_name,
        username,
        role: parse_role(&role)?,
        added_at: Timestamp::from_unix_millis(added_at),
      });
    }
    Ok(Some((Timestamp::from_unix_millis(read_at), members)))
  }

  /// The person's user on the server when the members were last read; `None` until they were read by one.
  pub fn members_read_as(&self) -> Result<Option<UserId>, StoreError> {
    let read_as: Option<String> = self
      .connection
      .query_row("SELECT members_read_as FROM sharing", [], |row| row.get(0))
      .optional()?
      .flatten();
    read_as.map(|user| parse("member id", &user)).transpose()
  }

  /// Replaces the members with `members`, as the server listed them at `read_at` to `read_as`, the person's user there.
  pub fn record_shared_members(
    &mut self,
    members: &[SharedMember],
    read_at: Timestamp,
    read_as: Option<UserId>,
  ) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let updated = transaction.execute(
      "UPDATE sharing SET members_read_at = ?1, members_read_as = ?2",
      params![read_at.unix_millis(), read_as.map(|user| user.to_string())],
    )?;
    if updated == 0 {
      return Err(StoreError::NotShared);
    }
    transaction.execute("DELETE FROM shared_members", [])?;
    for member in members {
      transaction.execute(
        "INSERT INTO shared_members (user_id, display_name, username, role, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
          member.user.to_string(),
          member.display_name,
          member.username,
          role_text(member.role),
          member.added_at.unix_millis()
        ],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// The server's admins as the members list last named them, by name; none until a server named them.
  pub fn server_admins(&self) -> Result<Vec<ServerAdmin>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT user_id, display_name, username FROM shared_server_admins ORDER BY username, display_name")?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(user, display_name, username)| {
        Ok(ServerAdmin {
          user: parse("server admin id", &user)?,
          display_name,
          username,
        })
      })
      .collect()
  }

  /// Replaces the server's admins with `admins`, as the members list named them.
  pub fn record_server_admins(&mut self, admins: &[ServerAdmin]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let shared: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM sharing)", [], |row| row.get(0))?;
    if !shared {
      return Err(StoreError::NotShared);
    }
    transaction.execute("DELETE FROM shared_server_admins", [])?;
    for admin in admins {
      transaction.execute(
        "INSERT OR REPLACE INTO shared_server_admins (user_id, display_name, username) VALUES (?1, ?2, ?3)",
        params![admin.user.to_string(), admin.display_name, admin.username],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }
}
