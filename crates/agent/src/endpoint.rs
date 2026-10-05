//! Where a model client sends its requests and how (`docs/architecture/native-agent.md` §3, §3.3): the
//! base URL, the API key, what the server is known to accept, and the client's timing.

use std::{fmt, sync::Arc, time::Duration};

use orchester_core::{
  connection::{AgentProvider, ApiEndpoint, BaseUrl, EndpointPreset},
  secret::SecretValue,
};

/// What a server accepts beyond the plain wire format. A preset decides it; the client never looks at
/// host names (§3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireQuirks {
  /// `anthropic-beta` headers and the fields they allow: thinking block binding, provider fallback.
  pub betas: bool,
  /// `eager_input_streaming` on tools; proxies and compatible servers may reject the field.
  pub eager_input_streaming: bool,
  /// `cache_control` breakpoints.
  pub cache_control: bool,
}

impl WireQuirks {
  /// The provider's own API.
  pub const OFFICIAL: Self = Self {
    betas: true,
    eager_input_streaming: true,
    cache_control: true,
  };
  /// Any other server: the plain wire format only.
  pub const CONSERVATIVE: Self = Self {
    betas: false,
    eager_input_streaming: false,
    cache_control: false,
  };

  /// The quirks of an Anthropic API endpoint: the official ones only for the Official preset at the
  /// preset's own base URL; a base URL changed by hand may be a proxy (§3.1, "Custom base URLs leave
  /// `eager_input_streaming` off").
  pub fn anthropic(api: &ApiEndpoint) -> Self {
    let official_url = EndpointPreset::Official.base_url(AgentProvider::AnthropicApi);
    if api.preset() == EndpointPreset::Official && official_url.as_ref() == Some(api.base_url()) {
      Self::OFFICIAL
    } else {
      Self::CONSERVATIVE
    }
  }
}

/// How long the client waits (§3.0, §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
  /// Making the connection.
  pub connect: Duration,
  /// A response that sends no byte for this long has failed; keep-alive comments count as bytes.
  pub idle: Duration,
  /// The first wait before a retry; each further one doubles.
  pub first_backoff: Duration,
}

impl Timing {
  pub const DEFAULT: Self = Self {
    connect: Duration::from_secs(30),
    idle: Duration::from_secs(120),
    first_backoff: Duration::from_secs(2),
  };
}

/// A model API as one client reaches it. `Debug` shows no key, and there is no `Display`.
#[derive(Clone)]
pub struct Endpoint {
  base_url: BaseUrl,
  key: Arc<SecretValue>,
  quirks: WireQuirks,
  provider_fallback: bool,
  timing: Timing,
}

impl Endpoint {
  /// An endpoint with provider fallback off (decision 4) and the default timing.
  pub fn new(base_url: BaseUrl, key: Arc<SecretValue>, quirks: WireQuirks) -> Self {
    Self {
      base_url,
      key,
      quirks,
      provider_fallback: false,
      timing: Timing::DEFAULT,
    }
  }

  /// Retries a declined request on another model where the server offers it (§3.1); it needs
  /// [`WireQuirks::betas`].
  #[must_use]
  pub const fn with_provider_fallback(mut self, on: bool) -> Self {
    self.provider_fallback = on;
    self
  }

  #[must_use]
  pub const fn with_timing(mut self, timing: Timing) -> Self {
    self.timing = timing;
    self
  }

  pub const fn base_url(&self) -> &BaseUrl {
    &self.base_url
  }

  pub(crate) fn key(&self) -> &SecretValue {
    &self.key
  }

  pub(crate) fn shared_key(&self) -> Arc<SecretValue> {
    Arc::clone(&self.key)
  }

  pub const fn quirks(&self) -> WireQuirks {
    self.quirks
  }

  pub const fn provider_fallback(&self) -> bool {
    self.provider_fallback && self.quirks.betas
  }

  pub const fn timing(&self) -> Timing {
    self.timing
  }
}

impl fmt::Debug for Endpoint {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Endpoint")
      .field("base_url", &self.base_url)
      .field("key", &"<redacted>")
      .field("quirks", &self.quirks)
      .field("provider_fallback", &self.provider_fallback)
      .field("timing", &self.timing)
      .finish()
  }
}
