//! *Test connection* for a native connection (`docs/architecture/native-agent.md` §7): listing the models
//! decides whether the endpoint is reachable and takes the key, and spends no tokens.

use orchester_core::{
  connection::CheckResult,
  native::model::{ModelClient, ModelError, RemoteModel},
};

use crate::{anthropic::AnthropicClient, endpoint::Endpoint, responses::ResponsesClient};

/// What a check found: the result to record, and the models the endpoint listed when it answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiCheck {
  pub result: CheckResult,
  pub models: Vec<RemoteModel>,
  /// Why the check failed, when it did.
  pub error: Option<ModelError>,
}

/// Checks an Anthropic Messages endpoint with `GET /v1/models`.
pub async fn check_anthropic(endpoint: Endpoint) -> ApiCheck {
  let listed = match AnthropicClient::new(endpoint) {
    Ok(client) => client.models().await,
    Err(error) => Err(error),
  };
  checked(listed)
}

/// Checks an OpenAI Responses endpoint with `GET /models`.
pub async fn check_responses(endpoint: Endpoint) -> ApiCheck {
  let listed = match ResponsesClient::new(endpoint) {
    Ok(client) => client.models().await,
    Err(error) => Err(error),
  };
  checked(listed)
}

fn checked(listed: Result<Vec<RemoteModel>, ModelError>) -> ApiCheck {
  match listed {
    Ok(models) => ApiCheck {
      result: CheckResult::Api { balance: None },
      models,
      error: None,
    },
    Err(error) => ApiCheck {
      result: result_of(&error),
      models: Vec::new(),
      error: Some(error),
    },
  }
}

/// The recorded result of a failed check: a refused key, a billing problem, or an endpoint that did
/// not answer as a model API does.
const fn result_of(error: &ModelError) -> CheckResult {
  match error {
    ModelError::KeyRejected | ModelError::Forbidden => CheckResult::KeyRejected,
    ModelError::Billing => CheckResult::BillingProblem { balance: None },
    _ => CheckResult::EndpointUnreachable,
  }
}
