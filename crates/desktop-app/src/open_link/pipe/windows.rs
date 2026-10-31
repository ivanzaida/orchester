//! The link pipe on Windows: `\\.\pipe\orchester-link-<hash of the data folder>`. The holder makes the pipe's first
//! instance, so a name another process already serves is refused, and takes no remote clients; Windows' default
//! security of a pipe lets only its maker's account (and the system and administrators) write to it, so no other
//! person on the computer can hand a link over. The program that hands a link over checks that the process serving
//! the pipe is the app the lock's record names, so another process that made the name first never gets a link.

use std::{io, path::Path, time::Duration};

use orchester_ipc::{fnv1a, server_process_id};
use tokio::{
  io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
  net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
  runtime::Handle,
  task::JoinHandle,
  time::timeout,
};

use super::{ANSWER_WAIT, HandOverError};
use crate::open_link::{LinkInbox, MAX_LINE, OpenLink};

/// What the holder answers once it took a link.
const ANSWER: &str = "ok";
/// Every instance of the pipe is busy: the holder is taking another link.
const ERROR_PIPE_BUSY: i32 = 231;
/// How long the program that hands a link over waits before it tries a busy pipe again.
const BUSY_RETRY: Duration = Duration::from_millis(50);

/// The pipe of the data folder `folder`: its canonical path without regard to ASCII case, as Windows reads paths, so
/// two spellings of one folder agree. A folder that cannot be resolved is named as it is, the same way in the program
/// that hands a link over and in the holder.
pub fn name(folder: &Path) -> String {
  let folder = folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
  let key = folder.as_os_str().as_encoded_bytes().to_ascii_lowercase();
  format!(r"\\.\pipe\orchester-link-{:016x}", fnv1a(&key))
}

/// Sends `link` over the pipe `name` to the holder, the process `holder`, and reads its answer.
pub(super) async fn send(name: &str, holder: u32, link: &str) -> Result<(), HandOverError> {
  let mut client = loop {
    match ClientOptions::new().open(name) {
      Ok(client) => break client,
      Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => tokio::time::sleep(BUSY_RETRY).await,
      Err(error) => return Err(error.into()),
    }
  };
  let served = server_process_id(&client)?;
  if served != holder {
    return Err(HandOverError::NotTheHolder { served, holder });
  }
  client.write_all(framed(link).as_bytes()).await?;
  if read_line(&mut client).await? != ANSWER {
    return Err(HandOverError::NotTaken);
  }
  Ok(())
}

/// Makes the pipe `name` and takes the links handed over on it into `inbox`, one connection at a time, until the task
/// is ended.
pub(super) fn listen(runtime: &Handle, name: &str, inbox: LinkInbox) -> io::Result<JoinHandle<()>> {
  let first = {
    let _entered = runtime.enter();
    instance(name, true)?
  };
  let name = name.to_owned();
  Ok(runtime.spawn(async move {
    let mut server = first;
    loop {
      if let Err(error) = server.connect().await {
        tracing::warn!(%error, "the link pipe stopped taking links");
        return;
      }
      // The next instance is made before this one is read, so a program arriving meanwhile waits for it.
      let next = match instance(&name, false) {
        Ok(next) => next,
        Err(error) => {
          tracing::warn!(%error, "the link pipe cannot take another link");
          take(server, &inbox).await;
          return;
        }
      };
      take(server, &inbox).await;
      server = next;
    }
  }))
}

fn instance(name: &str, first: bool) -> io::Result<NamedPipeServer> {
  ServerOptions::new()
    .first_pipe_instance(first)
    .reject_remote_clients(true)
    .create(name)
}

/// Reads one link from `server` into `inbox`, waiting at most [`ANSWER_WAIT`] for it, and answers.
async fn take(mut server: NamedPipeServer, inbox: &LinkInbox) {
  let line = match timeout(ANSWER_WAIT, read_line(&mut server)).await {
    Ok(Ok(line)) => line,
    Ok(Err(error)) => {
      tracing::warn!(%error, "a link could not be read from its pipe");
      return;
    }
    Err(_) => {
      tracing::warn!("a program connected to the link pipe and sent no link");
      return;
    }
  };
  inbox.push(OpenLink::parse(&line));
  let answered = timeout(ANSWER_WAIT, server.write_all(format!("{ANSWER}\n").as_bytes())).await;
  if !matches!(answered, Ok(Ok(()))) {
    tracing::info!("the program that handed a link over left before the answer");
  }
}

/// The link as the one line the pipe carries: a link longer than [`MAX_LINE`] or over more than one line goes as an
/// empty one, which the holder reads as no link of Orchester's. Only its length and line breaks are looked at.
fn framed(link: &str) -> String {
  let one_line = link.len() <= MAX_LINE && !link.contains(['\n', '\r']);
  format!("{}\n", if one_line { link } else { "" })
}

/// One line from `stream`, without its line break: what came before the stream ended, or more than [`MAX_LINE`] bytes
/// once the line is longer, which reads as no link of Orchester's.
async fn read_line(stream: &mut (impl AsyncRead + Unpin)) -> io::Result<String> {
  let mut line = Vec::new();
  let mut byte = [0_u8; 1];
  while line.len() <= MAX_LINE {
    if stream.read(&mut byte).await? == 0 || byte[0] == b'\n' {
      break;
    }
    line.push(byte[0]);
  }
  Ok(String::from_utf8_lossy(&line).into_owned())
}
