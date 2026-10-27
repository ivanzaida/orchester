//! Orchester Cloud's two addresses (`docs/cloud-api.md`, "Base"): its web address, of the pages the app opens in the
//! browser (sign-up, forgot password, the account, invites), and its API address, which every API call goes to. The
//! public cloud's API has a host of its own and answers at `/v1` there; a cloud without one, such as an older self-run
//! cloud, answers at `/api/v1` on its web address, as a team server does.

use reqwest::Url;

use super::{AddressError, ServerAddress, address::API_BASE};

/// Where a cloud's API answers on an address of its own.
const OWN_API_BASE: &str = "/v1";

/// A cloud's web address and, when it has one, its API address.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CloudAddress {
  web: ServerAddress,
  api: Option<ServerAddress>,
}

impl CloudAddress {
  /// The cloud at `web` whose API is at `api`, or on `web` with the `/api/v1` paths without one.
  pub fn new(web: ServerAddress, api: Option<ServerAddress>) -> Self {
    Self { web, api }
  }

  /// The address of the cloud's web pages, e.g. `https://orchester.cloud`.
  pub fn web(&self) -> &ServerAddress {
    &self.web
  }

  /// The API's own address, e.g. `https://api.orchester.cloud`; `None` while the API is on the web address.
  pub fn api(&self) -> Option<&ServerAddress> {
    self.api.as_ref()
  }

  /// The address the API calls go to: the API's own, or the web address.
  pub fn api_host(&self) -> &ServerAddress {
    self.api.as_ref().unwrap_or(&self.web)
  }

  /// The API's base URL, e.g. `https://api.orchester.cloud/v1`, or `https://cloud.kontur.lan/api/v1` for a cloud
  /// without an API address.
  pub fn api_base(&self) -> String {
    format!("{}{}", self.api_host(), self.base_path())
  }

  /// The URL of `path` in the cloud's API, e.g. `/keys`.
  pub(crate) fn endpoint(&self, path: &str) -> Result<Url, AddressError> {
    self.api_host().endpoint_under(self.base_path(), path)
  }

  const fn base_path(&self) -> &'static str {
    match self.api {
      Some(_) => OWN_API_BASE,
      None => API_BASE,
    }
  }
}
