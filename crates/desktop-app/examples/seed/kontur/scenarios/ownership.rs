//! Whose each project is (board *Signed out · redesign* `uBND2`; the person's decision, 2026-10-30), with the frames'
//! four projects, each with no task. Kontur (opened now) is the stand-in account's and shared through its team server
//! Acme, Dotfiles (three days ago) is the account's, and Ledger (yesterday, `origin` `github.com/acme/ledger`) and Site
//! (last week) are this computer's. Acme is only a row of the account's list: no account is signed in there, so nothing
//! asks it, and nothing listens at its address.
//!
//! - `ownership`: nobody signed in, so Ledger and Site are listed and two projects wait (`NpTDN`).
//! - `ownership-signed-in`: the QA app signs in and joins a stand-in team server that lists Ledger (`LinZt`, `ggjyN`,
//!   `xx4ST`; `Ib9ca` on Ledger's Sharing; `xQQbr`).
//! - `ownership-ended`: `ownership` after the cloud ended the sign-in from another device (`qxq4t`, `CBmxe`).
//! - `ownership-nothing-here`: only the account's two projects (`MvIpm`).
//! - `invite-signed-out`: the catalogue, nobody signed in, and the server [`INVITE_SERVER`] in the account's list,
//!   which the invite landing names while it asks for the sign-in (`zoihT`).

use std::{collections::HashMap, fs, path::PathBuf};

use orchester_api::test_support::account_cloud::{ACCOUNT, EMAIL};
use orchester_core::{
  id::{CloudAccountId, ProjectId, ServerId},
  project::{Project, ProjectKey},
  sharing::MemberRole,
};
use orchester_store::{AppDb, CloudSignInEnd, EndedCloudSignIn, ProjectDb, SharingLink, StoredServer};

use super::qa_app_asks;
use crate::kontur::{Kontur, SeedError, files, git, io_error, name};

const DAY: i64 = 24 * 60;
/// Acme's id.
const ACME: &str = "0191c7c0-0000-7000-8000-0000000005e3";
/// Nothing listens here: the port of the discard service, on this computer.
const ACME_ADDRESS: &str = "http://127.0.0.1:9";
/// The server `navigate` names for the invite landing, with `"tab": "invite"` and `"project": "kontur"`.
const INVITE_SERVER: &str = "0191c7c0-0000-7000-8000-0000000005e4";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  nothing_here(kontur)?;
  let ledger = project(kontur, ("Ledger", "LED", "repo-ledger"), DAY, None)?;
  git::set_origin(&ledger, "https://github.com/acme/ledger.git")?;
  project(kontur, ("Site", "SIT", "repo-site"), 7 * DAY, None).map(drop)
}

pub fn signed_in(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  kontur.ask_qa_app(qa_app_asks::SIGN_IN)?;
  kontur.ask_qa_app(qa_app_asks::JOIN_TEAM)
}

pub fn ended(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  AppDb::open(&kontur.app_database)?.set_cloud_sign_in_ended(Some(&EndedCloudSignIn {
    email: EMAIL.to_owned(),
    reason: CloudSignInEnd::Revoked,
    ended_at: kontur.ago(90),
    invite: None,
  }))?;
  Ok(())
}

pub fn nothing_here(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.catalogue()?;
  let account = account()?;
  share_kontur(kontur, &account)?;
  project(kontur, ("Dotfiles", "DOT", "repo-dotfiles"), 3 * DAY, Some(&account))?;
  kontur.owner = Some(account);
  Ok(())
}

pub fn invite(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.catalogue()?;
  add_server(
    kontur,
    (INVITE_SERVER, "orchester.example.com", "https://orchester.example.com"),
    &account()?,
  )
}

/// The stand-in cloud's account, as a sign-in to it stores it.
fn account() -> Result<CloudAccountId, SeedError> {
  ACCOUNT.parse().map_err(invalid)
}

/// Kontur linked to Acme as a member, with Acme in `account`'s list.
fn share_kontur(kontur: &mut Kontur, account: &CloudAccountId) -> Result<(), SeedError> {
  add_server(kontur, (ACME, "Acme", ACME_ADDRESS), account)?;
  let joined = kontur.ago(30 * DAY);
  let link = SharingLink {
    server: ACME.parse().map_err(invalid)?,
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").map_err(invalid)?,
    role: Some(MemberRole::Member),
    joined_at: joined,
    cursor: 0,
    last_synced_at: joined,
    removed_at: None,
    address: Some(format!("{ACME_ADDRESS}/kontur")),
  };
  kontur.db.start_sharing(&link, &[], (&[], &HashMap::new()))?;
  Ok(())
}

/// The server `(id, name, address)` in `account`'s list, with nobody signed in there from this computer.
fn add_server(
  kontur: &Kontur,
  (id, named, url): (&str, &str, &str),
  account: &CloudAccountId,
) -> Result<(), SeedError> {
  let server = StoredServer {
    id: id.parse::<ServerId>().map_err(invalid)?,
    url: url.to_owned(),
    name: named.to_owned(),
    plain_http_allowed: false,
    certificate_pin: None,
    added_at: kontur.ago(30 * DAY),
    joined_with: None,
    account: None,
  };
  AppDb::open(&kontur.app_database)?.add_server(&server, Some(account))?;
  Ok(())
}

/// A project of its own repository beside the application database, opened `opened` minutes ago and `owner`'s; its
/// folder.
fn project(
  kontur: &Kontur,
  (title, key, folder): (&str, &str, &str),
  opened: i64,
  owner: Option<&CloudAccountId>,
) -> Result<PathBuf, SeedError> {
  let root = kontur.app_database.with_file_name(folder);
  fs::create_dir_all(&root).map_err(io_error(&root))?;
  git::init(&root, &files::initial())?;
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new(key).map_err(invalid)?,
    name(title),
  );
  ProjectDb::create(&root, project.clone())?;
  AppDb::open(&kontur.app_database)?.register_project(&project, &root, kontur.ago(opened), owner)?;
  Ok(root)
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
