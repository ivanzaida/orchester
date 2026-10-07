//! Auto mode's reviewer on a native connection: one request of `orchester-agent`'s Anthropic Messages
//! client, built by core ([`native_request`]), with the connection's API key read from the keychain here
//! (it never leaves the client: not in the review, its errors or the activity). It is bounded by the
//! review's time and dropped at once when the review is no longer wanted. A response reports what it
//! spent; a request that failed, timed out or was dropped spent an unknown amount.

use std::{sync::Arc, time::Duration};

use orchester_agent::{AnthropicClient, Endpoint, WireQuirks};
use orchester_core::{
  connection::{
    ApiEndpoint,
    reviewer::{native_request, native_review},
  },
  native::model::ModelClient,
  resource::ModelId,
  run::review::{RequestReviewer, ReviewError, ReviewInput, ReviewSpend, Reviewed},
  secret::SecretValue,
};
use tokio::{runtime::Handle, sync::watch};

/// The reviewer on an Anthropic API connection, run from the blocking pool on the runner's runtime.
pub(super) struct NativeReviewer {
  pub(super) runtime: Handle,
  pub(super) client: AnthropicClient,
  pub(super) model: ModelId,
  pub(super) effort: Option<String>,
  pub(super) timeout: Duration,
  pub(super) stop: watch::Receiver<()>,
}

/// The client of `api` with `key`.
pub(super) fn client(api: &ApiEndpoint, key: SecretValue) -> Result<AnthropicClient, ReviewError> {
  let endpoint = Endpoint::new(api.base_url().clone(), Arc::new(key), WireQuirks::anthropic(api));
  AnthropicClient::new(endpoint).map_err(|error| ReviewError::CouldNotRun(error.to_string()))
}

impl RequestReviewer for NativeReviewer {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, input: &ReviewInput) -> Reviewed {
    let request = native_request(input, &self.model, self.effort.clone());
    self.runtime.block_on(async {
      let mut stop = self.stop.clone();
      // Nothing of the answer is shown while it streams.
      let mut streamed = |_| {};
      let unknown = |error| Reviewed::new(Err(error), ReviewSpend::Unknown);
      let answer = tokio::select! {
        answer = tokio::time::timeout(self.timeout, self.client.respond(&request, &mut streamed)) => answer,
        _ = stop.changed() => {
          return unknown(ReviewError::CouldNotRun("the review is no longer wanted".to_owned()));
        }
      };
      match answer {
        Ok(Ok(response)) => native_review(&response),
        Ok(Err(error)) => unknown(ReviewError::CouldNotRun(error.to_string())),
        Err(_) => unknown(ReviewError::TimedOut),
      }
    })
  }
}
