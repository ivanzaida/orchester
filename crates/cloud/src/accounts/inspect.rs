use std::net::IpAddr;

use orchester_api::cloud::{CloudErrorCode, LinkInspect, LinkInspected};

use super::{Cloud, links::link_email};
use crate::error::CloudError;

impl Cloud {
  /// The email a live link was sent to, so its page can show the address before anything is pressed; the link stays
  /// unused. An unknown, used or expired link, or one for the other purpose, answers `invalid_link` after the same one
  /// query, and counts against the client's address as a failed finish does.
  pub(crate) async fn inspect_link(&self, input: LinkInspect, address: IpAddr) -> Result<LinkInspected, CloudError> {
    self.inner.limits.link_inspect(address, self.now())?;
    let client = self.inner.db.client().await?;
    match link_email(&client, &input.link_token, input.purpose, self.now()).await? {
      Some(email) => Ok(LinkInspected { email }),
      None => {
        self.inner.limits.link_failed(address, self.now());
        Err(CloudErrorCode::InvalidLink.into())
      }
    }
  }
}
