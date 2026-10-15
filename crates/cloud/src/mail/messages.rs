//! The cloud's emails, with their words from `assets/locales/<locale>.json` (AGENTS.md rule 13). The cloud keeps no
//! locale of the person, so every email is in the one shipped locale, English. No email carries text a sign-up chose
//! (review 1, L1); an invite carries only what its team server signed: the inviter's display name, the server's name
//! and the project's (`docs/architecture/server.md`, "Email and personal data"). Links point to the web front's pages
//! (`/confirm/{token}`, `/reset/{token}`, `/account`, `/signup`), which call the API from their server side.

use orchester_api::{Credential, InviteKind};
use serde::Deserialize;

use super::{Email, EmailPurpose};
use crate::error::CloudError;

const ENGLISH: &str = include_str!("../../assets/locales/en.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Locale {
  email: Emails,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Emails {
  confirm: Template,
  already_registered: Template,
  reset: Template,
  invite: Template,
  invite_to_sign_up: Template,
  invite_target: Targets,
}

/// What an invite is to, in its email's sentence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Targets {
  project: String,
  server: String,
  account: String,
}

/// An invite as its email names it; every text comes from the team server's signed invite.
pub(crate) struct InviteMail<'a> {
  pub kind: InviteKind,
  pub inviter: &'a str,
  pub server: &'a str,
  pub project: Option<&'a str>,
}

/// The longest name, in characters, an email repeats from a team server.
const MAX_NAME_CHARS: usize = 100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Template {
  subject: String,
  body: String,
}

pub(crate) struct Messages {
  base_url: String,
  emails: Emails,
}

impl Messages {
  /// Fails when the locale lacks a text or a text lacks its placeholder, so a broken locale stops the start.
  pub fn new(base_url: String) -> Result<Self, CloudError> {
    let locale: Locale = serde_json::from_str(ENGLISH)
      .map_err(|_| CloudError::Configuration("the email locale does not have every email text"))?;
    let emails = locale.email;
    let placeholders = [
      (&emails.confirm.body, "{link}"),
      (&emails.already_registered.body, "{url}"),
      (&emails.reset.body, "{link}"),
      (&emails.invite.body, "{url}"),
      (&emails.invite.body, "{target}"),
      (&emails.invite.body, "{inviter}"),
      (&emails.invite_to_sign_up.body, "{url}"),
      (&emails.invite_to_sign_up.body, "{target}"),
      (&emails.invite_to_sign_up.body, "{inviter}"),
      (&emails.invite_target.project, "{project}"),
      (&emails.invite_target.project, "{server}"),
      (&emails.invite_target.server, "{server}"),
      (&emails.invite_target.account, "{server}"),
    ];
    if placeholders.iter().any(|(text, name)| !text.contains(name)) {
      return Err(CloudError::Configuration("an email text lacks one of its placeholders"));
    }
    Ok(Self { base_url, emails })
  }

  pub fn confirm(&self, to: String, token: &Credential) -> Email {
    let link = format!("{}/confirm/{}", self.base_url, token.expose());
    email(EmailPurpose::Confirm, to, &self.emails.confirm, "{link}", &link)
  }

  pub fn already_registered(&self, to: String) -> Email {
    email(
      EmailPurpose::AlreadyRegistered,
      to,
      &self.emails.already_registered,
      "{url}",
      &self.base_url,
    )
  }

  pub fn reset(&self, to: String, token: &Credential) -> Email {
    let link = format!("{}/reset/{}", self.base_url, token.expose());
    email(EmailPurpose::Reset, to, &self.emails.reset, "{link}", &link)
  }

  /// One email per invite: to the account page for a registered email, otherwise to sign-up, where the invite waits.
  pub fn invite(&self, to: String, invite: &InviteMail<'_>, registered: bool) -> Email {
    let targets = &self.emails.invite_target;
    let (server, project) = (plain(invite.server), invite.project.map(plain));
    let target = match (invite.kind, &project) {
      (InviteKind::Project, Some(project)) => fill(&targets.project, &[("{project}", project), ("{server}", &server)]),
      (InviteKind::Account, _) => fill(&targets.account, &[("{server}", &server)]),
      _ => fill(&targets.server, &[("{server}", &server)]),
    };
    let (template, url) = if registered {
      (&self.emails.invite, format!("{}/account", self.base_url))
    } else {
      (&self.emails.invite_to_sign_up, format!("{}/signup", self.base_url))
    };
    let inviter = plain(invite.inviter);
    Email {
      purpose: EmailPurpose::Invite,
      to,
      subject: template.subject.clone(),
      body: fill(
        &template.body,
        &[("{inviter}", &inviter), ("{target}", &target), ("{url}", &url)],
      ),
    }
  }
}

fn email(purpose: EmailPurpose, to: String, template: &Template, placeholder: &str, value: &str) -> Email {
  Email {
    purpose,
    to,
    subject: template.subject.clone(),
    body: fill(&template.body, &[(placeholder, value)]),
  }
}

/// `text` with each `{name}` of `values` replaced in one pass, so a value that itself holds `{…}` stays as it is.
fn fill(text: &str, values: &[(&str, &str)]) -> String {
  let mut out = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(start) = rest.find('{') {
    out.push_str(&rest[..start]);
    rest = &rest[start..];
    match values.iter().find(|(name, _)| rest.starts_with(name)) {
      Some((name, value)) => {
        out.push_str(value);
        rest = &rest[name.len()..];
      }
      None => {
        out.push('{');
        rest = &rest[1..];
      }
    }
  }
  out.push_str(rest);
  out
}

/// A name a team server chose, as an email repeats it: one line, no control characters, at most 100 characters.
fn plain(name: &str) -> String {
  let line: String = name
    .chars()
    .map(|c| if c.is_control() { ' ' } else { c })
    .take(MAX_NAME_CHARS)
    .collect();
  line.trim().to_owned()
}
