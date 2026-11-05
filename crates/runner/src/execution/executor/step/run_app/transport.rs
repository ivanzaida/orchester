//! Streamable HTTP MCP with bounded JSON/SSE responses, no redirects, proxies or credential diagnostics.

use std::{
  io::{BufRead, BufReader, Read},
  time::Duration,
};

use orchester_core::orchester_tools::{AppCall, AppDiscovery};
use orchester_exec::run_processes::RunAppProcess;
use reqwest::{
  blocking,
  header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde_json::{Value, json};

use super::AppError;

const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

pub(super) struct Client<'a> {
  http: blocking::Client,
  headers: HeaderMap,
  process: &'a RunAppProcess,
  url: String,
}

impl<'a> Client<'a> {
  pub(super) fn new(process: &'a RunAppProcess, discovery: &AppDiscovery) -> Result<Self, AppError> {
    // Reqwest's no-provider feature requires the same ring provider as the model clients.
    // Another client may already have installed one; losing that race is harmless.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let http = blocking::Client::builder()
      .no_proxy()
      .redirect(reqwest::redirect::Policy::none())
      .timeout(Duration::from_secs(15))
      .build()
      .map_err(|_| AppError::Request)?;
    let mut headers = HeaderMap::new();
    let mut authorization =
      HeaderValue::from_str(&format!("Bearer {}", discovery.token().expose())).map_err(|_| AppError::Request)?;
    authorization.set_sensitive(true);
    headers.insert(AUTHORIZATION, authorization);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
      "accept",
      HeaderValue::from_static("application/json, text/event-stream"),
    );
    Ok(Self {
      http,
      headers,
      process,
      url: discovery.url(),
    })
  }

  pub(super) fn call(mut self, call: &AppCall) -> Result<Value, AppError> {
    let initialized = self.post(
      &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{},
        "clientInfo":{"name":"orchester-run-app","version":"1"}
      }}),
      Some(1),
    )?;
    if !initialized["result"].is_object() || initialized.get("error").is_some() {
      return Err(AppError::Rejected);
    }
    self.post(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}), None)?;
    let response = self.post(
      &json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
      "params":{"name":call.tool(),"arguments":call.arguments()}}),
      Some(2),
    )?;
    if response.get("error").is_some() || !response["result"].is_object() {
      return Err(AppError::Rejected);
    }
    Ok(response["result"].clone())
  }

  fn post(&mut self, payload: &Value, id: Option<u64>) -> Result<Value, AppError> {
    self.process.check_alive()?;
    let body = serde_json::to_vec(payload).map_err(|_| AppError::Request)?;
    let response = self
      .http
      .post(&self.url)
      .headers(self.headers.clone())
      .body(body)
      .send()
      .map_err(|_| AppError::Request)?;
    if !response.status().is_success() {
      return Err(AppError::Rejected);
    }
    if let Some(session) = response.headers().get("mcp-session-id") {
      let mut session = session.clone();
      session.set_sensitive(true);
      self.headers.insert("mcp-session-id", session);
    }
    // A notification has no JSON-RPC result; 202/204 with an empty body is expected.
    if id.is_none() {
      return Ok(Value::Null);
    }
    let sse = response
      .headers()
      .get(CONTENT_TYPE)
      .is_some_and(|value| value.to_str().is_ok_and(|value| value.starts_with("text/event-stream")));
    let id = id.ok_or(AppError::Response)?;
    if sse {
      return read_sse(response, id);
    }
    let mut bytes = Vec::new();
    response
      .take(MAX_RESPONSE_BYTES + 1)
      .read_to_end(&mut bytes)
      .map_err(|_| AppError::Response)?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
      return Err(AppError::Response);
    }
    matching(&bytes, id)
  }
}

fn matching(bytes: &[u8], id: u64) -> Result<Value, AppError> {
  let value: Value = serde_json::from_slice(bytes).map_err(|_| AppError::Response)?;
  if value["jsonrpc"] != "2.0" || value["id"].as_u64() != Some(id) {
    return Err(AppError::Response);
  }
  Ok(value)
}

fn read_sse(response: impl Read, id: u64) -> Result<Value, AppError> {
  // MCP may leave its SSE stream open after the result: return on the matching event, not on EOF.
  let mut reader = BufReader::new(response.take(MAX_RESPONSE_BYTES + 1));
  let mut data = String::new();
  let mut line = String::new();
  let mut received = 0u64;
  loop {
    let bytes = reader.read_line(&mut line).map_err(|_| AppError::Response)?;
    received += bytes as u64;
    if received > MAX_RESPONSE_BYTES {
      return Err(AppError::Response);
    }
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if let Some(value) = trimmed.strip_prefix("data:") {
      if !data.is_empty() {
        data.push('\n');
      }
      data.push_str(value.strip_prefix(' ').unwrap_or(value));
    } else if trimmed.is_empty() && !data.is_empty() {
      let value: Value = serde_json::from_str(&data).map_err(|_| AppError::Response)?;
      if value["id"].as_u64() == Some(id) {
        return matching(data.as_bytes(), id);
      }
      data.clear();
    }
    if bytes == 0 {
      return Err(AppError::Response);
    }
    line.clear();
  }
}
