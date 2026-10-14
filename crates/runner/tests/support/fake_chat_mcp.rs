//! The fake CLIs' end of the Orchestrator chat's domain tools: a call over streamable HTTP with the bearer token, as
//! Claude Code and Codex make it.

#![allow(dead_code, reason = "each fake CLI uses a part of the shared support")]

use serde_json::{Value, json};

/// The chat's server: its address and the `Authorization` header that carries its token.
pub struct ChatServer {
  pub url: String,
  pub token: Option<String>,
}

impl ChatServer {
  /// Calls the tool `call` names (`<tool> <JSON arguments>`) and returns the text of its result.
  pub fn call(&self, call: &str) -> Result<String, String> {
    let (tool, arguments) = call.split_once(' ').unwrap_or((call, "{}"));
    let arguments: Value = serde_json::from_str(arguments).map_err(|error| error.to_string())?;
    // Reqwest without a TLS provider of its own needs one installed, as the runner's clients do; one installed
    // already is as good.
    if rustls::crypto::ring::default_provider().install_default().is_err() {
      eprintln!("a TLS provider was installed already");
    }
    let client = reqwest::blocking::Client::builder()
      .no_proxy()
      .build()
      .map_err(|error| error.to_string())?;
    let initialize = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "fake-cli", "version": "1"}}});
    let (session, _) = self.post(&client, None, &initialize)?;
    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    self.post(&client, session.as_deref(), &initialized)?;
    let called =
      json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": tool, "arguments": arguments}});
    let (_, answer) = self.post(&client, session.as_deref(), &called)?;
    let answer = answer.ok_or("no answer to the call")?;
    let text = answer
      .pointer("/result/content/0/text")
      .and_then(Value::as_str)
      .map_or_else(|| answer.to_string(), str::to_owned);
    Ok(text)
  }

  /// Posts `body`: the session the server named and the JSON-RPC message it answered with, if any.
  fn post(
    &self,
    client: &reqwest::blocking::Client,
    session: Option<&str>,
    body: &Value,
  ) -> Result<(Option<String>, Option<Value>), String> {
    let mut request = client
      .post(&self.url)
      .header("Content-Type", "application/json")
      .header("Accept", "application/json, text/event-stream")
      .body(body.to_string());
    if let Some(token) = &self.token {
      request = request.header("Authorization", token);
    }
    if let Some(session) = session {
      request = request.header("Mcp-Session-Id", session);
    }
    let response = request.send().map_err(|error| error.to_string())?;
    let status = response.status();
    let session = response
      .headers()
      .get("mcp-session-id")
      .and_then(|value| value.to_str().ok())
      .map(str::to_owned);
    let text = response.text().map_err(|error| error.to_string())?;
    if !status.is_success() {
      return Err(format!("HTTP {status}"));
    }
    let message = text
      .lines()
      .filter_map(|line| line.strip_prefix("data:"))
      .chain(std::iter::once(text.as_str()))
      .find_map(|data| {
        serde_json::from_str::<Value>(data.trim())
          .ok()
          .filter(|value| value.get("id").is_some())
      });
    Ok((session, message))
  }
}
