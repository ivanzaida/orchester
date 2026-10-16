//! The cloud's emails, with their words from `assets/locales/<locale>.json` (AGENTS.md rule 13). The cloud keeps no
//! locale of the person, so every email is in the one shipped locale, English. No email carries text a sign-up chose
//! (review 1, L1). Links point to the web front's pages (`/confirm/{token}`, `/reset/{token}`), which call the API
//! with the token from their server side.

use orchester_api::Credential;
use orchester_core::id::InviteId;
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
  invite_sign_up: Template,
  two_factor_on: Template,
  two_factor_off: Template,
  recovery_code_used: Template,
}

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
      (&emails.confirm, "{link}"),
      (&emails.already_registered, "{url}"),
      (&emails.reset, "{link}"),
      (&emails.invite, "{url}"),
      (&emails.invite_sign_up, "{url}"),
      (&emails.two_factor_on, "{url}"),
      (&emails.two_factor_off, "{url}"),
      (&emails.recovery_code_used, "{url}"),
    ];
    if placeholders
      .iter()
      .any(|(template, name)| !template.body.contains(name))
    {
      return Err(CloudError::Configuration("an email text lacks its link placeholder"));
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

  /// An invite to an email with an account: open Orchester. The words are the cloud's own; nothing the team server
  /// chose (its name, the inviter's, the project's) goes into an email, so a server cannot write to anyone through it.
  pub fn invite(&self, to: String) -> Email {
    email(EmailPurpose::Invite, to, &self.emails.invite, "{url}", &self.base_url)
  }

  /// An invite to an email with no account: create one first, on the sign-up page that names the invite.
  pub fn invite_sign_up(&self, to: String, invite: InviteId) -> Email {
    let url = format!("{}/signup?invite={invite}", self.base_url);
    email(EmailPurpose::Invite, to, &self.emails.invite_sign_up, "{url}", &url)
  }

  /// A notice about the account's two-factor authentication, with a link to the account's security section. It names
  /// nothing but what happened, so it tells a reader of the mailbox nothing they could use.
  pub fn two_factor(&self, to: String, purpose: EmailPurpose) -> Email {
    let template = match purpose {
      EmailPurpose::TwoFactorOff => &self.emails.two_factor_off,
      EmailPurpose::RecoveryCodeUsed => &self.emails.recovery_code_used,
      _ => &self.emails.two_factor_on,
    };
    let url = format!("{}/account#security", self.base_url);
    email(purpose, to, template, "{url}", &url)
  }

  pub fn reset(&self, to: String, token: &Credential) -> Email {
    let link = format!("{}/reset/{}", self.base_url, token.expose());
    email(EmailPurpose::Reset, to, &self.emails.reset, "{link}", &link)
  }
}

fn email(purpose: EmailPurpose, to: String, template: &Template, placeholder: &str, value: &str) -> Email {
  Email {
    purpose,
    to,
    subject: template.subject.clone(),
    body: template.body.replace(placeholder, value),
  }
}
