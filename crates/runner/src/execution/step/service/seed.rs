//! A service's seed (`docs/architecture/agent-testing.md`, "Service › seed"), run once after it is ready and before
//! its start answers: its HTTP requests in order, each filled with what the requests before it captured, or its
//! command, without a shell, in the service's folder and environment, its output kept in the service's log. A request
//! that does not answer a 2xx status (a redirect included: none is followed), a capture its answer has nothing for, or
//! a command that does not exit with 0 in time fails the start; the error names the request, its status and the start
//! of its answer.
//!
//! A header may send a project secret by its name: its value is read from the keychain before the service starts, sent
//! only in that header, marked sensitive, and replaced in the service's log, in every error and in the captured values
//! the start answers.

use std::{
  collections::BTreeMap,
  io::Read,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  command::{SecretVariable, redact_secrets},
  connection::invocation::Platform,
  resource::{FieldName, HttpMethod},
  secret::SecretName,
  service::{FilledSeedRequest, RunValues, Seed, SeedRequest, Service, ServiceValue},
};
use orchester_exec::process_tree::ProcessTree;
use reqwest::{
  Method,
  blocking::{self, Response},
  header::{CONTENT_TYPE, HeaderName, HeaderValue},
  redirect,
};

use super::{
  StepServices,
  launch::secret_value,
  readiness::{Output, SharedLog},
};

/// How long one seed request may take, its answer read too.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a seed command may run.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(300);
/// How often a seed command is checked for its end.
const POLL: Duration = Duration::from_millis(100);
/// The most bytes of a seed request's answer that are read; captures read their values within them.
const MAX_ANSWER_BYTES: u64 = 1024 * 1024;
/// The most characters of an answer a failed request's error shows.
const MAX_EXCERPT_CHARS: usize = 400;
/// How many of its last lines of output a failed seed command's error shows.
const FAILED_COMMAND_LINES: u32 = 20;

/// The secrets the headers of a seed's requests send, by their names, read from the keychain.
pub(super) type HeaderSecrets = Vec<(SecretName, SecretVariable)>;

/// What a seed's requests captured, by name, their secret values replaced: what the start answers.
pub(super) type Captured = BTreeMap<FieldName, String>;

impl StepServices {
  /// The secrets the headers of `service`'s seed requests send, read once before the service starts.
  pub(super) fn header_secrets(&self, service: &Service) -> Result<HeaderSecrets, String> {
    let Seed::Requests(requests) = service.seed() else {
      return Ok(Vec::new());
    };
    let mut secrets: HeaderSecrets = Vec::new();
    for (index, request) in requests.as_slice().iter().enumerate() {
      for header in request.headers() {
        let ServiceValue::Secret(secret) = header.value() else {
          continue;
        };
        if secrets.iter().any(|(read, _)| read == secret) {
          continue;
        }
        let user = format!("the header `{}` of seed request {}", header.name(), index + 1);
        let value = secret_value((&self.root, self.vault.as_ref()), &user, secret)?;
        secrets.push((secret.clone(), SecretVariable::new(header.name(), value)));
      }
    }
    Ok(secrets)
  }

  /// Runs `service`'s seed, filled with `values`, which take what its requests capture; why not, when a request or
  /// the command fails or `ended` says the invocation ended. `secrets` are every secret the service and its seed were
  /// given, whose values no error or captured value shows.
  pub(super) fn seed(
    &self,
    service: &Service,
    values: &mut RunValues,
    (headers, secrets): (&HeaderSecrets, &[SecretVariable]),
    (log, ended): (&SharedLog, &dyn Fn() -> bool),
  ) -> Result<Captured, String> {
    match service.seed() {
      Seed::None => Ok(Captured::new()),
      Seed::Requests(requests) => {
        let client = client()?;
        let mut captured = Captured::new();
        for (index, request) in requests.as_slice().iter().enumerate() {
          if ended() {
            return Err("was given up: the turn ended during its seed".to_owned());
          }
          let filled = values
            .seed_request(service, index)
            .map_err(|error| format!("cannot run its seed: {error}"))?
            .ok_or_else(|| format!("cannot run its seed: it has no request {}", index + 1))?;
          let sent = Sent { index, filled: &filled };
          let answer = sent.send(&client, headers, secrets)?;
          for (name, value) in sent.captures(request, &answer)? {
            captured.insert(name.clone(), redact_secrets(&value, secrets));
            values.capture(name, value);
          }
        }
        Ok(captured)
      }
      Seed::Command { .. } => self
        .seed_command(service, values, (log, ended))
        .map(|()| Captured::new()),
    }
  }

  /// Runs `service`'s seed command to its end, its output kept in `log`.
  fn seed_command(
    &self,
    service: &Service,
    values: &RunValues,
    (log, ended): (&SharedLog, &dyn Fn() -> bool),
  ) -> Result<(), String> {
    let Seed::Command { program, .. } = service.seed() else {
      return Ok(());
    };
    let platform = Platform::current();
    let path = service.search_path(&self.project, self.inherited_path());
    let found = program
      .candidates(&self.worktree, path.as_deref(), platform)
      .into_iter()
      .find(|candidate| candidate.is_file())
      .ok_or_else(|| format!("cannot run its seed: the program `{}` was not found", program.as_str()))?;
    program.check_place(&found, &self.root).map_err(|_| {
      "cannot run its seed: its program now leads into the project's repository or off this computer".to_owned()
    })?;
    let process = service
      .seed_process(
        (&found, &self.worktree),
        (self.inherited.iter().cloned(), &self.project),
        platform,
        values,
      )
      .map_err(|error| format!("cannot run its seed: {error}"))?
      .ok_or_else(|| "cannot run its seed: it is not a command".to_owned())?;
    let mut command = process.command;
    for (variable, secret) in &process.secrets {
      command = command.with_secret_env(self.secret(variable, secret)?);
    }
    let mut tree = ProcessTree::start(&command).map_err(|error| format!("cannot run its seed command: {error}"))?;
    let output = Output::read(&mut tree, None, log);
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let failed = loop {
      match tree.exited() {
        Ok(Some(status)) => {
          output.settle();
          if status.success() {
            return Ok(());
          }
          break format!(
            "ran its seed command `{}`, which exited with {status}",
            program.as_str()
          );
        }
        Ok(None) => {}
        Err(error) => break format!("cannot watch its seed command: {error}"),
      }
      if ended() {
        break "was given up: the turn ended during its seed".to_owned();
      }
      if Instant::now() >= deadline {
        break format!(
          "ran its seed command `{}`, which did not end within {} s",
          program.as_str(),
          COMMAND_TIMEOUT.as_secs()
        );
      }
      thread::sleep(POLL);
    };
    if let Err(error) = tree.stop() {
      tracing::warn!(%error, "a seed command did not stop");
    }
    let lines = log.lock().excerpt(FAILED_COMMAND_LINES, 0);
    Err(format!("{failed}. The seed command's output is in its log: {lines}"))
  }
}

/// One seed request as sent: its place in the seed, from 0, and its filled parts.
struct Sent<'a> {
  index: usize,
  filled: &'a FilledSeedRequest,
}

impl Sent<'_> {
  /// How errors name it, e.g. `seed request 2 (POST http://127.0.0.1:4100/api/teams)`.
  fn named(&self, secrets: &[SecretVariable]) -> String {
    let shown = format!(
      "seed request {} ({} {})",
      self.index + 1,
      method(self.filled.method()),
      self.filled.url()
    );
    redact_secrets(&shown, secrets)
  }

  /// Sends it with `headers`' secrets and reads its answer, which has a 2xx status.
  fn send(
    &self,
    client: &blocking::Client,
    headers: &HeaderSecrets,
    secrets: &[SecretVariable],
  ) -> Result<String, String> {
    let named = || self.named(secrets);
    let mut request = client.request(method(self.filled.method()), self.filled.url());
    let mut typed = false;
    for (name, value) in self.filled.headers() {
      let header = HeaderName::from_bytes(name.as_bytes())
        .map_err(|_| format!("cannot send {}: its header `{name}` is not a header name", named()))?;
      typed |= header == CONTENT_TYPE;
      let value = match value {
        ServiceValue::Plain(text) => HeaderValue::from_str(text).map_err(|_| {
          format!(
            "cannot send {}: the value of its header `{name}` cannot be sent",
            named()
          )
        })?,
        ServiceValue::Secret(secret) => {
          let read = headers
            .iter()
            .find(|(read, _)| read == secret)
            .map(|(_, value)| value)
            .ok_or_else(|| format!("cannot send {}: the secret `{secret}` was not read", named()))?;
          let mut value = HeaderValue::from_str(read.expose()).map_err(|_| {
            format!(
              "cannot send {}: the secret `{secret}` of its header `{name}` cannot be sent in a header",
              named()
            )
          })?;
          value.set_sensitive(true);
          value
        }
      };
      request = request.header(header, value);
    }
    if let Some(body) = self.filled.body() {
      if !typed {
        request = request.header(CONTENT_TYPE, HeaderValue::from_static("application/json"));
      }
      request = request.body(body.to_owned());
    }
    // An error of reqwest may name the URL, which holds no secret; it is redacted all the same.
    let answer = request
      .send()
      .map_err(|error| redact_secrets(&format!("{} got no answer: {error}", named()), secrets))?;
    let status = answer.status();
    let text =
      read(answer).map_err(|error| redact_secrets(&format!("{} answered {status}, but {error}", named()), secrets))?;
    if !status.is_success() {
      return Err(format!(
        "ran its seed, but {} answered {status}: {}",
        named(),
        excerpt(&text, secrets)
      ));
    }
    Ok(text)
  }

  /// The values `request`'s captures read in `answer`, as text: a string as it is, any other value as JSON.
  fn captures(&self, request: &SeedRequest, answer: &str) -> Result<Vec<(FieldName, String)>, String> {
    if request.captures().is_empty() {
      return Ok(Vec::new());
    }
    let named = || format!("seed request {}", self.index + 1);
    let json: serde_json::Value = serde_json::from_str(answer)
      .map_err(|_| format!("ran its seed, but {} answered no JSON to capture values from", named()))?;
    request
      .captures()
      .iter()
      .map(|capture| {
        let value = capture
          .path()
          .find(&json)
          .filter(|value| !value.is_null())
          .ok_or_else(|| {
            format!(
              "ran its seed, but the answer of {} has nothing at `{}` for `{}`",
              named(),
              capture.path().as_str(),
              capture.name()
            )
          })?;
        let text = match value {
          serde_json::Value::String(text) => text.clone(),
          other => other.to_string(),
        };
        Ok((capture.name().clone(), text))
      })
      .collect()
  }
}

/// The start of a failed answer, on one line, its secret values replaced first: at most [`MAX_EXCERPT_CHARS`].
fn excerpt(answer: &str, secrets: &[SecretVariable]) -> String {
  let redacted = redact_secrets(answer.trim(), secrets);
  if redacted.is_empty() {
    return "an empty body".to_owned();
  }
  let one_line: String = redacted.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
  let mut shown: String = one_line.chars().take(MAX_EXCERPT_CHARS).collect();
  if one_line.chars().count() > MAX_EXCERPT_CHARS {
    shown.push('…');
  }
  format!("`{shown}`")
}

/// At most [`MAX_ANSWER_BYTES`] of `answer`'s body, as text.
fn read(answer: Response) -> Result<String, String> {
  let mut body = Vec::new();
  answer
    .take(MAX_ANSWER_BYTES)
    .read_to_end(&mut body)
    .map_err(|error| format!("its answer cannot be read: {error}"))?;
  Ok(String::from_utf8_lossy(&body).into_owned())
}

fn method(method: HttpMethod) -> Method {
  match method {
    HttpMethod::Get => Method::GET,
    HttpMethod::Post => Method::POST,
    HttpMethod::Put => Method::PUT,
    HttpMethod::Patch => Method::PATCH,
    HttpMethod::Delete => Method::DELETE,
  }
}

/// A client that follows no redirect and goes through no proxy, so a secret header goes only where its URL says.
fn client() -> Result<blocking::Client, String> {
  // Reqwest's no-provider feature requires the same ring provider as the model clients.
  // Another client may already have installed one; losing that race is harmless.
  let _ = rustls::crypto::ring::default_provider().install_default();
  blocking::Client::builder()
    .no_proxy()
    .redirect(redirect::Policy::none())
    .timeout(REQUEST_TIMEOUT)
    .build()
    .map_err(|error| format!("cannot run its seed over HTTP: {error}"))
}
