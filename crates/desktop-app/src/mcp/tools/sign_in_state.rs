use lurq::mcp::{McpTool, Scope};
use orchester_store::CloudSignInEnd;
use orchester_workspace::sharing::cloud::{Cloud, SignIn};
use serde_json::{Value, json};

/// The one tool that answers while this computer is signed out of Orchester Cloud.
pub const SIGN_IN_STATE: &str = "sign_in_state";

const DESCRIPTION: &str = "Whether Orchester is signed in to Orchester Cloud on this computer. While it is not, the \
  window shows only the sign-in and every other tool answers sign_in_required; only a person signs in, in the window, \
  never over MCP. Answers `signed_in`, and while signed out `reason`: why the last sign-in here ended (revoked, \
  password_reset, account_deleted, signed_out, two_factor to sign in again with a code, other), or null on a computer never signed in. No email, no token.";

pub fn tool(cloud: Cloud) -> McpTool {
  McpTool::new(SIGN_IN_STATE)
    .description(DESCRIPTION)
    .input_schema(json!({ "type": "object", "properties": {}, "additionalProperties": false }))
    .scope(Scope::Observe)
    .read_only()
    .async_handler(move |_| {
      let cloud = cloud.clone();
      async move { Ok(sign_in_state(&cloud.sign_in_state())) }
    })
}

/// What the tool answers for `sign_in`.
pub fn sign_in_state(sign_in: &SignIn) -> Value {
  match sign_in {
    SignIn::SignedIn { .. } => json!({ "signed_in": true }),
    SignIn::SignedOut { ended, .. } => json!({
      "signed_in": false,
      "reason": ended.as_ref().map(|ended| reason(ended.reason)),
    }),
  }
}

const fn reason(end: CloudSignInEnd) -> &'static str {
  match end {
    CloudSignInEnd::Revoked => "revoked",
    CloudSignInEnd::PasswordReset => "password_reset",
    CloudSignInEnd::AccountDeleted => "account_deleted",
    CloudSignInEnd::SignedOut => "signed_out",
    CloudSignInEnd::TwoFactor => "two_factor",
    CloudSignInEnd::Other => "other",
  }
}
