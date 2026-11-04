//! Whether a started service is ready, as it defines it, checked until its timeout: its port accepts connections, its
//! URL answers a path with a 2xx or 3xx status, or a line of its output matches a pattern. Its output is read for as
//! long as it runs, so a full pipe never holds it up; what it says is not kept yet.

use std::{
  io::{BufRead, BufReader, Read},
  net::{Ipv4Addr, SocketAddr, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use orchester_core::service::{ReadyCheck, RunValues, Service};
use orchester_exec::process_tree::ProcessTree;
use regex::Regex;
use reqwest::{blocking, redirect};

/// How often readiness is checked.
const POLL: Duration = Duration::from_millis(100);
/// How long one check may take: a connection, or an HTTP answer.
const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// One way of telling the service is ready.
enum Probe {
  Port(SocketAddr),
  Http {
    client: blocking::Client,
    url: String,
  },
  /// Set once a line of the output matched.
  Line(Arc<AtomicBool>),
}

impl Probe {
  fn ready(&self) -> bool {
    match self {
      Self::Port(address) => TcpStream::connect_timeout(address, CHECK_TIMEOUT).is_ok(),
      Self::Http { client, url } => client
        .get(url)
        .send()
        .is_ok_and(|answer| answer.status().is_success() || answer.status().is_redirection()),
      Self::Line(seen) => seen.load(Ordering::Acquire),
    }
  }
}

/// Waits until `service`, started as `tree` with `values` on `port`, is ready; why not, when it exits first, its
/// timeout passes, or `ended` says the invocation ended.
pub(super) fn wait(
  tree: &mut ProcessTree,
  (service, values): (&Service, &RunValues),
  port: u16,
  ended: &dyn Fn() -> bool,
) -> Result<(), String> {
  let check = service.readiness().check();
  let pattern = match check {
    ReadyCheck::LogLine(pattern) => {
      Some(Regex::new(pattern.as_str()).map_err(|error| format!("has a log pattern that does not read: {error}"))?)
    }
    ReadyCheck::PortOpen | ReadyCheck::Http(_) => None,
  };
  let seen = drain(tree, pattern);
  let probe = match check {
    ReadyCheck::PortOpen => Probe::Port(SocketAddr::from((Ipv4Addr::LOCALHOST, port))),
    ReadyCheck::Http(_) => {
      let path = values
        .readiness_path(service)
        .map_err(|error| format!("cannot be checked: {error}"))?
        .unwrap_or_default();
      Probe::Http {
        client: http_client()?,
        url: format!("http://127.0.0.1:{port}{path}"),
      }
    }
    ReadyCheck::LogLine(_) => Probe::Line(seen),
  };
  let timeout = service.readiness().timeout();
  let deadline = Instant::now() + timeout.duration();
  loop {
    if ended() {
      return Err("was given up: the turn ended before it was ready".to_owned());
    }
    match tree.exited() {
      Ok(Some(status)) => return Err(format!("exited before it was ready ({status})")),
      Ok(None) => {}
      Err(error) => return Err(format!("cannot be watched: {error}")),
    }
    if probe.ready() {
      return Ok(());
    }
    if Instant::now() >= deadline {
      return Err(format!(
        "was not ready within {} s: {}",
        timeout.secs(),
        unready(check, port)
      ));
    }
    thread::sleep(POLL);
  }
}

/// What did not happen in time.
fn unready(check: &ReadyCheck, port: u16) -> String {
  match check {
    ReadyCheck::PortOpen => format!("its port {port} accepted no connection"),
    ReadyCheck::Http(path) => format!("{} answered no 2xx or 3xx status", path.as_str()),
    ReadyCheck::LogLine(pattern) => format!("no line of its output matched `{}`", pattern.as_str()),
  }
}

/// A loopback client that follows no redirect and goes through no proxy: a 3xx answer counts as ready.
fn http_client() -> Result<blocking::Client, String> {
  // Reqwest's no-provider feature requires the same ring provider as the model clients.
  // Another client may already have installed one; losing that race is harmless.
  let _ = rustls::crypto::ring::default_provider().install_default();
  blocking::Client::builder()
    .no_proxy()
    .redirect(redirect::Policy::none())
    .timeout(CHECK_TIMEOUT)
    .build()
    .map_err(|error| format!("cannot be checked over HTTP: {error}"))
}

/// Reads the tree's standard output and error on threads of their own until they close; the flag is set once a line
/// matches `pattern`.
fn drain(tree: &mut ProcessTree, pattern: Option<Regex>) -> Arc<AtomicBool> {
  let seen = Arc::new(AtomicBool::new(false));
  let pattern = pattern.map(Arc::new);
  let (stdout, stderr) = tree.take_output();
  let pipes: [Option<Box<dyn Read + Send>>; 2] = [
    stdout.map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    stderr.map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
  ];
  for pipe in pipes.into_iter().flatten() {
    let (seen, pattern) = (Arc::clone(&seen), pattern.clone());
    thread::spawn(move || read_lines(pipe, pattern.as_deref(), &seen));
  }
  seen
}

fn read_lines(pipe: Box<dyn Read + Send>, pattern: Option<&Regex>, seen: &AtomicBool) {
  let mut reader = BufReader::new(pipe);
  let mut line = Vec::new();
  loop {
    line.clear();
    match reader.read_until(b'\n', &mut line) {
      Ok(0) => return,
      Ok(_) => {
        let text = String::from_utf8_lossy(&line);
        if pattern.is_some_and(|pattern| pattern.is_match(text.trim_end_matches(['\r', '\n']))) {
          seen.store(true, Ordering::Release);
        }
      }
      Err(error) => {
        tracing::debug!(%error, "a service's output can no longer be read");
        return;
      }
    }
  }
}
