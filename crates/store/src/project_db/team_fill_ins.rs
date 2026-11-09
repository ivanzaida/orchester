//! What this computer fills in for the team's configuration, kept in the project database
//! (`docs/design/team-configuration.md`): the connection (and account) picked per team profile, which team MCP
//! servers are on here, and secret values by secret name. A secret's value lives in the OS keychain; its row holds
//! the name and the keychain handle only.

use orchester_core::{
  id::{AgentAccountId, ConnectionId, DecisionConnectionId, ResourceId, SecretId},
  secret::{SecretName, SecretValue, SecretVault, VaultKey},
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The connection, and the account when the connection needs one, a person picked for a team profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionPick {
  pub connection: ConnectionId,
  pub account: Option<AgentAccountId>,
}

impl ProjectDb {
  /// Keeps `pick` for the team profile `profile`, replacing an earlier one. A connection added later changes nothing.
  pub fn set_connection_pick(&mut self, profile: ResourceId, pick: ConnectionPick) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO team_connection_picks (resource_id, connection_id, account_id) VALUES (?1, ?2, ?3) \
       ON CONFLICT (resource_id) DO UPDATE SET connection_id = excluded.connection_id, account_id = excluded.account_id",
      params![
        profile.to_string(),
        pick.connection.to_string(),
        pick.account.map(|account| account.to_string())
      ],
    )?;
    Ok(())
  }

  /// The pick made for `profile` on this computer, if any.
  pub fn connection_pick(&self, profile: ResourceId) -> Result<Option<ConnectionPick>, StoreError> {
    let stored: Option<(String, Option<String>)> = self
      .connection
      .query_row(
        "SELECT connection_id, account_id FROM team_connection_picks WHERE resource_id = ?1",
        [profile.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let Some((connection, account)) = stored else {
      return Ok(None);
    };
    Ok(Some(ConnectionPick {
      connection: parse("picked connection id", &connection)?,
      account: account
        .map(|account| parse("picked account id", &account))
        .transpose()?,
    }))
  }

  /// Keeps `connection` as the decision connection the team decision reviewer `reviewer` runs on here, replacing an
  /// earlier pick. It shares the row of [`Self::set_connection_pick`]: a reviewer is of one kind at a time, and a pick
  /// of the other kind names no connection of this computer, so it reads as no pick.
  pub fn set_decision_pick(
    &mut self,
    reviewer: ResourceId,
    connection: DecisionConnectionId,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO team_connection_picks (resource_id, connection_id, account_id) VALUES (?1, ?2, NULL) \
       ON CONFLICT (resource_id) DO UPDATE SET connection_id = excluded.connection_id, account_id = NULL",
      params![reviewer.to_string(), connection.to_string()],
    )?;
    Ok(())
  }

  /// The decision connection picked for `reviewer` on this computer, if any.
  pub fn decision_pick(&self, reviewer: ResourceId) -> Result<Option<DecisionConnectionId>, StoreError> {
    let stored: Option<String> = self
      .connection
      .query_row(
        "SELECT connection_id FROM team_connection_picks WHERE resource_id = ?1",
        [reviewer.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    stored
      .map(|connection| parse("picked decision connection id", &connection))
      .transpose()
  }

  /// Forgets the pick for `profile`; a profile without one is not an error.
  pub fn clear_connection_pick(&mut self, profile: ResourceId) -> Result<(), StoreError> {
    self.connection.execute(
      "DELETE FROM team_connection_picks WHERE resource_id = ?1",
      [profile.to_string()],
    )?;
    Ok(())
  }

  /// Turns the team MCP server `server` on or off here.
  pub fn set_team_mcp_on_here(&mut self, server: ResourceId, on: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO team_mcp_switches (resource_id, enabled) VALUES (?1, ?2) \
       ON CONFLICT (resource_id) DO UPDATE SET enabled = excluded.enabled",
      params![server.to_string(), on],
    )?;
    Ok(())
  }

  /// The person's choice for the team MCP server `server` here: on, off, or `None` when nobody has decided.
  pub fn team_mcp_switch(&self, server: ResourceId) -> Result<Option<bool>, StoreError> {
    Ok(
      self
        .connection
        .query_row(
          "SELECT enabled FROM team_mcp_switches WHERE resource_id = ?1",
          [server.to_string()],
          |row| row.get(0),
        )
        .optional()?,
    )
  }

  /// Gives the team secret `name` this value on this computer, replacing an earlier one. The value goes to the
  /// keychain; the database keeps the name and the handle.
  pub fn set_team_secret_value(
    &mut self,
    vault: &dyn SecretVault,
    name: &SecretName,
    value: &SecretValue,
  ) -> Result<(), StoreError> {
    if let Some(handle) = self.team_secret_handle(name)? {
      vault.store(&self.vault_key(handle), value)?;
      return Ok(());
    }
    let handle = SecretId::generate();
    let key = self.vault_key(handle);
    vault.store(&key, value)?;
    let stored = self.connection.execute(
      "INSERT INTO team_secret_values (name, handle) VALUES (?1, ?2)",
      params![name.as_str(), handle.to_string()],
    );
    if let Err(source) = stored {
      // Without its row the new handle is unreachable, so its value must not stay; the database error is the one
      // worth reporting, and a failed cleanup adds nothing a person could act on.
      vault.delete(&key).ok();
      return Err(source.into());
    }
    Ok(())
  }

  /// Whether a value is set for the team secret `name` here. It does not read the keychain.
  pub fn has_team_secret_value(&self, name: &SecretName) -> Result<bool, StoreError> {
    Ok(self.team_secret_handle(name)?.is_some())
  }

  /// Whether the keychain has the value of the team secret `name`: a row for the name whose handle the keychain still
  /// holds. It asks [`SecretVault::exists`], so the value is not read and the person is not prompted. A name with no
  /// row has none; a keychain that cannot say is an error.
  pub fn team_secret_in_keychain(&self, vault: &dyn SecretVault, name: &SecretName) -> Result<bool, StoreError> {
    match self.team_secret_handle(name)? {
      Some(handle) => Ok(vault.exists(&self.vault_key(handle))?),
      None => Ok(false),
    }
  }

  /// The names that have a value here, in order.
  pub fn team_secret_names_set(&self) -> Result<Vec<SecretName>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT name FROM team_secret_values ORDER BY name")?;
    let names = statement
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    names
      .iter()
      .map(|name| SecretName::new(name).map_err(|error| corrupt("team secret name", error)))
      .collect()
  }

  /// The value of the team secret `name`; `None` when none is set here. A handle whose keychain value is gone is
  /// the keychain's `NotFound` error.
  pub fn team_secret_value(
    &self,
    vault: &dyn SecretVault,
    name: &SecretName,
  ) -> Result<Option<SecretValue>, StoreError> {
    match self.team_secret_handle(name)? {
      Some(handle) => Ok(Some(vault.read(&self.vault_key(handle))?)),
      None => Ok(None),
    }
  }

  /// Removes the value of the team secret `name` from the keychain and the database; a name without one is not an
  /// error. The keychain goes first, so a failure leaves a row that clearing again removes.
  pub fn clear_team_secret_value(&mut self, vault: &dyn SecretVault, name: &SecretName) -> Result<(), StoreError> {
    if let Some(handle) = self.team_secret_handle(name)? {
      vault.delete(&self.vault_key(handle))?;
      self
        .connection
        .execute("DELETE FROM team_secret_values WHERE name = ?1", [name.as_str()])?;
    }
    Ok(())
  }

  /// Removes the value of every team secret that has one from the keychain and the database, as
  /// [`Self::clear_team_secret_value`] does each; the project that leaves its team keeps none. A handle is a new id
  /// made for the team secret alone, so no local secret shares a keychain entry with one.
  pub fn clear_team_secret_values(&mut self, vault: &dyn SecretVault) -> Result<(), StoreError> {
    for name in self.team_secret_names_set()? {
      self.clear_team_secret_value(vault, &name)?;
    }
    Ok(())
  }

  fn team_secret_handle(&self, name: &SecretName) -> Result<Option<SecretId>, StoreError> {
    let handle: Option<String> = self
      .connection
      .query_row(
        "SELECT handle FROM team_secret_values WHERE name = ?1",
        [name.as_str()],
        |row| row.get(0),
      )
      .optional()?;
    handle.map(|handle| parse("team secret handle", &handle)).transpose()
  }

  fn vault_key(&self, handle: SecretId) -> VaultKey {
    VaultKey::new(self.project().id(), handle)
  }
}
