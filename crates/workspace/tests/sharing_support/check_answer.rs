//! How the stand-in cloud answers the check of a sign-in (`GET /me`) when a test asks for something else than the
//! account: busy, limited, or what a network that needs a sign-in page first answers.

use super::error;

/// How the check of a sign-in (`GET /me`) is answered instead of the account.
#[derive(Clone, Copy, Debug)]
pub enum CheckAnswer {
  /// `503 busy` with `Retry-After` in seconds.
  Busy(u64),
  /// `429 rate_limited` with `Retry-After` in seconds.
  Limited(u64),
  /// A redirect elsewhere, as a network that needs a sign-in page first answers.
  Redirect,
  /// That sign-in page itself: a `401` that is not Orchester's.
  SignInPage,
}

impl CheckAnswer {
  pub(super) fn reply(self) -> String {
    let (status, headers, body) = match self {
      Self::Busy(seconds) => (
        "503 Service Unavailable",
        format!("Content-Type: application/json\r\nRetry-After: {seconds}\r\n"),
        error("busy").to_string(),
      ),
      Self::Limited(seconds) => (
        "429 Too Many Requests",
        format!("Content-Type: application/json\r\nRetry-After: {seconds}\r\n"),
        error("rate_limited").to_string(),
      ),
      Self::Redirect => (
        "302 Found",
        "Location: http://portal.test/login\r\n".to_owned(),
        String::new(),
      ),
      Self::SignInPage => (
        "401 Unauthorized",
        "Content-Type: text/html\r\n".to_owned(),
        "<html><body>Sign in to use this network</body></html>".to_owned(),
      ),
    };
    format!(
      "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
      body.len()
    )
  }
}
