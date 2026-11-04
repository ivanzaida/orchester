//! Whether a started service is ready, as it defines it, checked until its timeout: its port accepts connections, its
//! URL answers a path with a 2xx or 3xx status, or a line of its output matches a pattern. Its output is read for as
//! long as it runs, so a full pipe never holds it up, and kept in its log ([`ServiceLog`]), at most
//! [`MAX_LINE_BYTES`] of a line: the rest of a longer one is read and dropped.

use std::{
  io::{self, BufRead, BufReader, Read},
  net::{Ipv4Addr, SocketAddr, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use orchester_core::service::{LogStream, MAX_LINE_BYTES, ReadyCheck, RunValues, Service, ServiceLog};
use orchester_exec::process_tree::ProcessTree;
use parking_lot::Mutex;
use regex::Regex;
use reqwest::{blocking, redirect};

/// How often readiness is checked.
const POLL: Duration = Duration::from_millis(100);
/// How long one check may take: a connection, or an HTTP answer.
const CHECK_TIMEOUT: Duration = Duration::from_secs(2);
/// How long the output of a service that exited before it was ready may take to be read to its end.
const SETTLE: Duration = Duration::from_secs(1);

/// A service's log, which the readers of its output write and its tools read.
pub(super) type SharedLog = Arc<Mutex<ServiceLog>>;

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

/// Waits until `service`, started as `tree` with `values` on `port`, is ready, its output kept in `log`; why not,
/// when it exits first, its timeout passes, or `ended` says the invocation ended.
pub(super) fn wait(
  tree: &mut ProcessTree,
  (service, values): (&Service, &RunValues),
  port: u16,
  (log, ended): (&SharedLog, &dyn Fn() -> bool),
) -> Result<(), String> {
  let check = service.readiness().check();
  let pattern = match check {
    ReadyCheck::LogLine(pattern) => {
      Some(Regex::new(pattern.as_str()).map_err(|error| format!("has a log pattern that does not read: {error}"))?)
    }
    ReadyCheck::PortOpen | ReadyCheck::Http(_) => None,
  };
  let output = Output::read(tree, pattern, log);
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
    ReadyCheck::LogLine(_) => Probe::Line(Arc::clone(&output.seen)),
  };
  let timeout = service.readiness().timeout();
  let deadline = Instant::now() + timeout.duration();
  loop {
    if ended() {
      return Err("was given up: the turn ended before it was ready".to_owned());
    }
    match tree.exited() {
      Ok(Some(status)) => {
        output.settle();
        return Err(format!("exited before it was ready ({status})"));
      }
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

/// The readers of the tree's standard output and error, each on a thread of its own until its stream closes.
struct Output {
  /// Set once a line matches the readiness pattern.
  seen: Arc<AtomicBool>,
  /// How many of the streams are still open.
  open: Arc<AtomicUsize>,
}

impl Output {
  /// Starts reading the tree's output into `log`, watching for `pattern`.
  fn read(tree: &mut ProcessTree, pattern: Option<Regex>, log: &SharedLog) -> Self {
    let output = Self {
      seen: Arc::new(AtomicBool::new(false)),
      open: Arc::new(AtomicUsize::new(0)),
    };
    let pattern = pattern.map(Arc::new);
    let (stdout, stderr) = tree.take_output();
    let pipes: [Option<(Box<dyn Read + Send>, LogStream)>; 2] = [
      stdout.map(|pipe| (Box::new(pipe) as Box<dyn Read + Send>, LogStream::Stdout)),
      stderr.map(|pipe| (Box::new(pipe) as Box<dyn Read + Send>, LogStream::Stderr)),
    ];
    for (pipe, stream) in pipes.into_iter().flatten() {
      let reader = Reader {
        stream,
        pattern: pattern.clone(),
        seen: Arc::clone(&output.seen),
        log: Arc::clone(log),
      };
      let open = Arc::clone(&output.open);
      open.fetch_add(1, Ordering::AcqRel);
      thread::spawn(move || {
        reader.run(pipe);
        open.fetch_sub(1, Ordering::AcqRel);
      });
    }
    output
  }

  /// Waits up to [`SETTLE`] until both streams closed, so the log holds what an exited program wrote last; a process
  /// it started may hold them open longer.
  fn settle(&self) {
    let deadline = Instant::now() + SETTLE;
    while self.open.load(Ordering::Acquire) > 0 && Instant::now() < deadline {
      thread::sleep(Duration::from_millis(20));
    }
  }
}

/// What one reader keeps its stream's lines in.
struct Reader {
  stream: LogStream,
  pattern: Option<Arc<Regex>>,
  seen: Arc<AtomicBool>,
  log: SharedLog,
}

impl Reader {
  fn run(&self, pipe: Box<dyn Read + Send>) {
    let mut reader = BufReader::new(pipe);
    let mut line = Vec::new();
    loop {
      line.clear();
      let limit = u64::try_from(MAX_LINE_BYTES + 1).expect("a line's limit fits in 64 bits");
      let read = (&mut reader).take(limit).read_until(b'\n', &mut line);
      match read {
        Ok(0) => return,
        Ok(_) => {
          let whole = line.ends_with(b"\n") || line.len() <= MAX_LINE_BYTES;
          if !whole && let Err(error) = skip_line(&mut reader) {
            tracing::debug!(%error, "a service's output can no longer be read");
            return;
          }
          self.keep(&String::from_utf8_lossy(&line), whole);
        }
        Err(error) => {
          tracing::debug!(%error, "a service's output can no longer be read");
          return;
        }
      }
    }
  }

  fn keep(&self, line: &str, whole: bool) {
    let text = line.trim_end_matches(['\r', '\n']);
    if self.pattern.as_ref().is_some_and(|pattern| pattern.is_match(text)) {
      self.seen.store(true, Ordering::Release);
    }
    let mut log = self.log.lock();
    if whole {
      log.push(self.stream, text);
    } else {
      log.push_cut(self.stream, text);
    }
  }
}

/// Reads and drops the rest of a line, up to and with its line ending.
fn skip_line(reader: &mut impl BufRead) -> io::Result<()> {
  loop {
    let buffer = reader.fill_buf()?;
    if buffer.is_empty() {
      return Ok(());
    }
    if let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
      reader.consume(end + 1);
      return Ok(());
    }
    let length = buffer.len();
    reader.consume(length);
  }
}
