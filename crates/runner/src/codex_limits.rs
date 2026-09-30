//! Reads only limit events of this invocation's Codex session. Exec JSON omits them.

use std::{
  io,
  path::{Path, PathBuf},
};

use orchester_core::{
  command::Command,
  connection::{
    AgentProvider,
    plan_limits::LimitReading,
    stream::{AgentEvent, OutputStream, parse_codex_line},
  },
  time::Timestamp,
};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};

pub struct SessionLimits {
  home: PathBuf,
  file: Option<PathBuf>,
  offset: u64,
  since: Timestamp,
}

impl SessionLimits {
  pub fn new(home: PathBuf, since: Timestamp) -> Self {
    Self {
      home,
      file: None,
      offset: 0,
      since,
    }
  }

  pub fn for_command(command: &Command, since: Timestamp) -> Option<Self> {
    let home = command
      .env()
      .iter()
      .find(|(name, _)| name.eq_ignore_ascii_case("CODEX_HOME"))
      .map(|(_, path)| PathBuf::from(path))
      .or_else(|| std::env::var_os("CODEX_HOME").map(PathBuf::from))
      .or_else(|| {
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
          .map(|home| PathBuf::from(home).join(".codex"))
      })?;
    Some(Self::new(home, since))
  }

  pub async fn read(&mut self, session: &str) -> io::Result<Vec<LimitReading>> {
    // The filename comes from the CLI, never an arbitrary path or a directory traversal.
    if session.len() != 36 || !session.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'-') {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid Codex session identifier",
      ));
    }
    if self.file.is_none() {
      self.file = find_session(&self.home.join("sessions"), session).await?;
    }
    let Some(path) = &self.file else {
      return Ok(Vec::new());
    };
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(io::SeekFrom::Start(self.offset)).await?;
    let mut reader = BufReader::new(file);
    let mut readings = Vec::new();
    loop {
      let mut line = String::new();
      let bytes = reader.read_line(&mut line).await?;
      if bytes == 0 || !line.ends_with('\n') {
        break;
      }
      self.offset += bytes as u64;
      if let Some(reading) = reading(&line, self.since) {
        readings.push(reading);
      }
    }
    Ok(readings)
  }
}

fn reading(line: &str, since: Timestamp) -> Option<LimitReading> {
  // Session logs also contain prompts and tool output. Only a complete, validated limit event
  // is returned; unrelated or unreadable lines never reach activity, IPC, or logs.
  let value: serde_json::Value = serde_json::from_str(line).ok()?;
  let at = Timestamp::parse_utc(value.get("timestamp")?.as_str()?)?;
  if at < since {
    return None;
  }
  let AgentEvent::PlanLimits { limits } = parse_codex_line(OutputStream::Stdout, line) else {
    return None;
  };
  Some(LimitReading {
    provider: AgentProvider::CodexCli,
    at,
    limits,
  })
}

async fn find_session(root: &Path, session: &str) -> io::Result<Option<PathBuf>> {
  let mut pending = vec![(root.to_path_buf(), 0)];
  let suffix = format!("-{session}.jsonl");
  while let Some((folder, depth)) = pending.pop() {
    let mut entries = match tokio::fs::read_dir(folder).await {
      Ok(entries) => entries,
      Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
      Err(error) => return Err(error),
    };
    while let Some(entry) = entries.next_entry().await? {
      let kind = entry.file_type().await?;
      if kind.is_dir() && depth < 3 {
        pending.push((entry.path(), depth + 1));
      }
      if kind.is_file() && entry.file_name().to_string_lossy().ends_with(&suffix) {
        return Ok(Some(entry.path()));
      }
    }
  }
  Ok(None)
}
