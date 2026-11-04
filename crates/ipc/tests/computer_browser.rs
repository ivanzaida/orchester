//! This computer's Settings › Browser, which the desktop passes to a runner as it attaches and again when they change
//! (protocol 47, ORC-675): a program chosen or none, each part read or why not, round-trip as requests, and the
//! runner's answer too. Run with `cargo test -p orchester-ipc --test computer_browser`.

use std::{num::NonZeroU32, path::PathBuf};

use orchester_ipc::{
  AttachSession, Command, Event, Reply, Request, RequestId,
  browser::ComputerBrowser,
  frame::{FrameReader, FrameWriter},
};

#[tokio::test]
async fn computer_browser_settings_round_trip_with_attach_and_their_changes() {
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (mut reader, mut writer) = (FrameReader::new(left), FrameWriter::new(right));
  let profiles = PathBuf::from("data").join("browser");
  let unreadable = ComputerBrowser {
    program: Err("Settings › Browser cannot be read: the database is locked".to_owned()),
    profiles: Err("the application database has no folder".to_owned()),
  };
  let chosen = ComputerBrowser {
    program: Ok(Some(PathBuf::from("Chromium").join("chrome.exe"))),
    profiles: Ok(profiles.clone()),
  };
  let commands = [
    Command::Attach {
      session: AttachSession::Reconnect,
      agent_slots: NonZeroU32::MIN,
      browser: unreadable,
    },
    Command::SetBrowser { browser: chosen },
    Command::SetBrowser {
      browser: ComputerBrowser::installed(profiles),
    },
  ];
  let requests: Vec<Request> = (1..)
    .zip(commands)
    .map(|(id, command)| Request {
      id: RequestId::new(id),
      command,
    })
    .collect();
  let reply = Event::Reply {
    id: RequestId::new(2),
    reply: Reply::BrowserSet,
  };

  for request in &requests {
    writer.write(request).await.expect("write request");
  }
  writer.write(&reply).await.expect("write reply");
  drop(writer);
  for request in requests {
    assert_eq!(reader.read::<Request>().await.expect("read request"), Some(request));
  }
  assert_eq!(reader.read::<Event>().await.expect("read reply"), Some(reply));
}
