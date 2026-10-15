//! The words of the cloud's emails, in English: the cloud has no locale of the person yet (reported in the C1
//! notes). Links point to the web front's pages (`/confirm/{token}`, `/reset/{token}`), which call the API with the
//! token from their server side.

use orchester_api::Credential;

use super::{Email, EmailPurpose};

pub(crate) struct Messages {
  base_url: String,
}

impl Messages {
  pub fn new(base_url: String) -> Self {
    Self { base_url }
  }

  pub fn confirm(&self, to: String, display_name: &str, token: &Credential) -> Email {
    let link = format!("{}/confirm/{}", self.base_url, token.expose());
    Email {
      purpose: EmailPurpose::Confirm,
      to,
      subject: "Confirm your Orchester account".into(),
      body: format!(
        "Hello {display_name},\n\nconfirm the email of your Orchester account by opening this link within 24 \
         hours:\n\n{link}\n\nIf you did not sign up for Orchester, ignore this email; no account is created \
         without the confirmation.\n"
      ),
    }
  }

  pub fn already_registered(&self, to: String) -> Email {
    Email {
      purpose: EmailPurpose::AlreadyRegistered,
      to,
      subject: "You already have an Orchester account".into(),
      body: format!(
        "Someone tried to sign up for Orchester with this email, which already has an account.\n\nIf it was you, \
         sign in at {} instead. If you forgot your password, choose Forgot password there.\n\nIf it was not you, \
         you can ignore this email; nothing changed.\n",
        self.base_url
      ),
    }
  }

  pub fn reset(&self, to: String, token: &Credential) -> Email {
    let link = format!("{}/reset/{}", self.base_url, token.expose());
    Email {
      purpose: EmailPurpose::Reset,
      to,
      subject: "Reset your Orchester password".into(),
      body: format!(
        "Choose a new password for your Orchester account by opening this link within 1 hour:\n\n{link}\n\n\
         Setting a new password signs out every device of the account. If you did not ask for this, ignore this \
         email; your password stays as it is.\n"
      ),
    }
  }
}
