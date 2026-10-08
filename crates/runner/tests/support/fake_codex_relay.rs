//! What `orchester-fake-codex` does to the runner's end of Orchester's relay besides starting the server, chosen by
//! the thread's model, so that tests can check which connections the runner takes ([`Probe`]). A knock connects as a
//! relay would, shows a token and asks for the tools: `answered` when the runner listed them, else `closed`.

use std::{
  io::{BufRead, BufReader, Write},
  net::TcpStream,
  time::Duration,
};

use super::fake_codex_tools::setting;

/// Knocks with a wrong token: as many as the runner once gave up after.
const BAD_KNOCKS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Probe {
  None,
  /// `-knock-`: once Codex's two relays came, knocks with the token (`knock: `), and again as the turn
  /// starts (`late-knock: `).
  Knock,
  /// `-late-relay-`: the thread's relay starts only after `mcpServerStatus/list` was answered.
  LateRelay,
  /// `-no-thread-relay-`: the thread starts no relay, only `mcpServerStatus/list` does.
  NoThreadRelay,
  /// `-silent-`: before the relays, opens a connection that never shows the token and keeps it open.
  Silent,
  /// `-bad-knocks-`: before the relays, knocks [`BAD_KNOCKS`] times with a wrong token.
  BadKnocks,
}

impl Probe {
  pub fn of(model: &str) -> Self {
    [
      ("-knock-", Self::Knock),
      ("-late-relay-", Self::LateRelay),
      ("-no-thread-relay-", Self::NoThreadRelay),
      ("-silent-", Self::Silent),
      ("-bad-knocks-", Self::BadKnocks),
    ]
    .into_iter()
    .find_map(|(words, probe)| model.contains(words).then_some(probe))
    .unwrap_or(Self::None)
  }

  /// Whether the thread's relay starts while `mcpServerStatus/list` is answered, as it does in Codex.
  pub fn starts_thread_relay_with_the_list(self) -> bool {
    !matches!(self, Self::LateRelay | Self::NoThreadRelay)
  }

  /// What the probe does before the relays start: the silent connection it keeps open, if any.
  pub fn before_relays(self, args: &[String]) -> Option<TcpStream> {
    match self {
      Self::Silent => TcpStream::connect(address(args)?).ok(),
      Self::BadKnocks => {
        for _ in 0..BAD_KNOCKS {
          eprintln!("bad-knock: {}", knock(args, Some("not-the-token")));
        }
        None
      }
      Self::None | Self::Knock | Self::LateRelay | Self::NoThreadRelay => None,
    }
  }
}

fn address(args: &[String]) -> Option<String> {
  setting(args, "env.ORCHESTER_APPROVAL_ADDRESS")?
    .as_str()
    .map(str::to_owned)
}

/// Knocks with `token`, the one Codex hands Orchester's server when `None`.
pub fn knock(args: &[String], token: Option<&str>) -> &'static str {
  let handed = std::env::var("ORCHESTER_APPROVAL_TOKEN").unwrap_or_default();
  let Some(address) = address(args) else {
    return "no server";
  };
  let Ok(mut stream) = TcpStream::connect(address) else {
    return "closed";
  };
  if stream.set_read_timeout(Some(Duration::from_secs(3))).is_err()
    || writeln!(stream, "{}\n\"list\"", token.unwrap_or(&handed)).is_err()
  {
    return "closed";
  }
  let mut answer = String::new();
  match BufReader::new(stream).read_line(&mut answer) {
    Ok(length) if length > 0 => "answered",
    _ => "closed",
  }
}
