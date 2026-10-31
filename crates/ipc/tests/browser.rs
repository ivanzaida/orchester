//! The *Browser* window's messages (protocol 37): every request, reply and event round-trips, a page's selection at
//! its limit fits a frame and stays out of the log, a screencast frame travels as a binary frame with its JPEG within
//! the frame limit, malformed binary frames are refused, and a frame crosses the platform's real endpoint (the Unix
//! socket on macOS, the named pipe on Windows).

use std::time::Duration;

use orchester_core::{id::RunId, time::Timestamp};
use orchester_ipc::{
  Command, Endpoint, Event, FrameError, PROTOCOL_VERSION, Rejection, Reply, Request, RequestId,
  browser::{
    AgentAction, AgentWaitEnd, BrowserCommand, BrowserDialog, BrowserEvent, BrowserMode, BrowserReply, BrowserState,
    BrowserTab, BrowserTabId, DialogKind, ElementBox, KeyAction, MAX_CLIPBOARD_TEXT, Modifiers, MouseAction,
    MouseButton, Navigation, PageCursor, PersonInput, ScreencastFrame, SelectId, SelectList, SelectOption, TabOwner,
    Viewport,
  },
  frame::{FrameReader, FrameWriter, MAX_FRAME_LEN},
};
use tokio::io::{AsyncWriteExt, DuplexStream, ReadHalf, WriteHalf};

type Reader = FrameReader<ReadHalf<DuplexStream>>;
type Writer = FrameWriter<WriteHalf<DuplexStream>>;

const PATIENCE: Duration = Duration::from_secs(10);

/// One end that reads and one that writes, of an in-memory connection.
fn connection() -> (Reader, Writer) {
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (read, _) = tokio::io::split(left);
  let (_, write) = tokio::io::split(right);
  (FrameReader::new(read), FrameWriter::new(write))
}

fn tab(id: &str) -> BrowserTabId {
  BrowserTabId::new(id)
}

fn viewport() -> Viewport {
  Viewport::new(1280, 800, 200).expect("in bounds")
}

fn frame(sequence: u64, jpeg: Vec<u8>) -> ScreencastFrame {
  ScreencastFrame {
    tab: tab("T1"),
    sequence,
    css_width: 1280,
    css_height: 800,
    jpeg,
  }
}

/// The person's input of every kind.
fn inputs() -> Vec<PersonInput> {
  let shift = Modifiers {
    shift: true,
    ..Modifiers::default()
  };
  vec![
    PersonInput::Mouse {
      action: MouseAction::Pressed,
      x: 120,
      y: -4,
      button: MouseButton::Left,
      clicks: 2,
      modifiers: shift,
    },
    PersonInput::Mouse {
      action: MouseAction::Moved,
      x: 0,
      y: 0,
      button: MouseButton::None,
      clicks: 0,
      modifiers: Modifiers::default(),
    },
    PersonInput::Wheel {
      x: 10,
      y: 20,
      delta_x: 0,
      delta_y: -120,
      modifiers: Modifiers::default(),
    },
    PersonInput::Key {
      action: KeyAction::Down,
      key: "A".to_owned(),
      code: "KeyA".to_owned(),
      text: Some("A".to_owned()),
      modifiers: shift,
    },
    PersonInput::Key {
      action: KeyAction::Up,
      key: "Enter".to_owned(),
      code: "Enter".to_owned(),
      text: None,
      modifiers: Modifiers::default(),
    },
    PersonInput::Text {
      text: "héllo \"quoted\"".to_owned(),
    },
    PersonInput::Composition {
      text: "にほ".to_owned(),
      selection_start: 2,
      selection_end: 2,
    },
  ]
}

/// One request of every browser command.
fn commands() -> Vec<BrowserCommand> {
  let mut commands = vec![
    BrowserCommand::Subscribe,
    BrowserCommand::Show {
      tab: tab("T1"),
      viewport: viewport(),
    },
    BrowserCommand::StopShowing,
    BrowserCommand::OpenTab {
      url: Some("http://localhost:5173/".to_owned()),
    },
    BrowserCommand::OpenTab { url: None },
    BrowserCommand::CloseTab { tab: tab("T2") },
    BrowserCommand::AnswerDialog {
      tab: tab("T2"),
      accept: true,
      text: Some("42".to_owned()),
    },
    BrowserCommand::ChooseOption {
      tab: tab("T2"),
      select: SelectId::new(77),
      index: 3,
    },
    BrowserCommand::CopySelection { tab: tab("T1") },
    BrowserCommand::CutSelection { tab: tab("T1") },
    BrowserCommand::CursorAt {
      tab: tab("T1"),
      x: 120,
      y: -4,
    },
    BrowserCommand::OpenInWindow,
    BrowserCommand::BackToOrchester,
  ];
  commands.extend(
    inputs()
      .into_iter()
      .map(|input| BrowserCommand::Input { tab: tab("T1"), input }),
  );
  commands.extend(
    [
      Navigation::Address {
        url: "https://docs.rs/".to_owned(),
      },
      Navigation::Back,
      Navigation::Forward,
      Navigation::Reload,
    ]
    .into_iter()
    .map(|to| BrowserCommand::Navigate { tab: tab("T1"), to }),
  );
  commands
}

/// One event of every browser kind, with the replies and rejections of browser requests.
fn events(run: RunId) -> Vec<Event> {
  let element = ElementBox {
    x: 10,
    y: -20,
    width: 300,
    height: 32,
  };
  let state = BrowserState {
    tabs: vec![
      BrowserTab {
        id: tab("T1"),
        owner: TabOwner::Run { run },
        title: "orders-sync · Kontur Admin".to_owned(),
        url: "http://localhost:5173/orders".to_owned(),
        loading: true,
      },
      BrowserTab {
        id: tab("T2"),
        owner: TabOwner::Person,
        title: String::new(),
        url: "about:blank".to_owned(),
        loading: false,
      },
    ],
    shown: Some(tab("T1")),
    mode: BrowserMode::Headed,
  };
  let reply = |reply| Event::Reply {
    id: RequestId::new(5),
    reply,
  };
  vec![
    Event::Browser(BrowserEvent::Changed(state)),
    Event::Browser(BrowserEvent::Changed(BrowserState::default())),
    Event::Browser(BrowserEvent::AgentStartedBrowsing { run }),
    Event::Browser(BrowserEvent::AgentActed {
      run,
      tab: tab("T1"),
      action: AgentAction {
        tool: "browser_type".to_owned(),
        summary: "typed 8 in Max attempts".to_owned(),
        element: Some(element),
      },
    }),
    Event::Browser(BrowserEvent::DialogOpened {
      tab: tab("T2"),
      dialog: BrowserDialog {
        kind: DialogKind::Prompt,
        message: "How many?".to_owned(),
        default_prompt: "3".to_owned(),
      },
    }),
    Event::Browser(BrowserEvent::DialogClosed { tab: tab("T2") }),
    Event::Browser(BrowserEvent::SelectOpened {
      tab: tab("T2"),
      list: SelectList {
        select: SelectId::new(77),
        options: vec![SelectOption {
          label: "Kontur".to_owned(),
          selected: true,
          disabled: false,
        }],
        element,
      },
    }),
    reply(Reply::Browser(BrowserReply::Done)),
    reply(Reply::Browser(BrowserReply::TabOpened { tab: tab("T3") })),
    reply(Reply::Browser(BrowserReply::Selection {
      text: "héllo \"quoted\"\nnext".to_owned(),
    })),
    reply(Reply::Browser(BrowserReply::Selection { text: String::new() })),
    reply(Reply::Rejected(Rejection::BrowserUnavailable {
      detail: "browser_unavailable: no Chromium-based browser is installed".to_owned(),
    })),
    reply(Reply::Rejected(Rejection::NoSuchTab)),
  ]
}

/// The reply of every shape of the mouse cursor.
fn cursor_replies() -> impl Iterator<Item = Event> {
  [PageCursor::Arrow, PageCursor::Hand, PageCursor::Text]
    .into_iter()
    .map(|cursor| Event::Reply {
      id: RequestId::new(6),
      reply: Reply::Browser(BrowserReply::Cursor { cursor }),
    })
}

/// The events of an agent waiting for the person came with protocol 35, a page's selection with 36 and the mouse
/// cursor's shape with 37, so an older runner is told apart at the handshake: where it waits and until when, and each
/// way its wait ends.
#[tokio::test]
async fn browser_agent_waiting_events_round_trip_under_a_raised_protocol() {
  assert_eq!(PROTOCOL_VERSION, 37);
  let (mut reader, mut writer) = connection();
  let run = RunId::generate();
  let ended = |end| {
    Event::Browser(BrowserEvent::AgentWaitEnded {
      run,
      tab: tab("T1"),
      end,
    })
  };
  let sent = [
    Event::Browser(BrowserEvent::AgentWaiting {
      run,
      tab: tab("T1"),
      until: Timestamp::from_unix_millis(1_793_372_042_138),
    }),
    ended(AgentWaitEnd::Ran),
    ended(AgentWaitEnd::Capped),
    ended(AgentWaitEnd::Cancelled),
  ];
  let written = async {
    for event in &sent {
      writer.write_event(event).await.expect("write event");
    }
    drop(writer);
  };
  let read = async {
    let mut events = Vec::new();
    while let Some(event) = reader.read_event().await.expect("read event") {
      events.push(event);
    }
    events
  };
  let ((), received) = tokio::join!(written, read);
  assert_eq!(received, sent);
}

#[tokio::test]
async fn browser_requests_replies_and_events_round_trip() {
  let (mut reader, mut writer) = connection();
  let requests: Vec<Request> = commands()
    .into_iter()
    .enumerate()
    .map(|(index, command)| Request {
      id: RequestId::new(index as u64 + 1),
      command: Command::Browser(command),
    })
    .collect();
  let mut events = events(RunId::generate());
  events.extend(cursor_replies());
  let written = async {
    for request in &requests {
      writer.write(request).await.expect("write request");
    }
    for event in &events {
      writer.write_event(event).await.expect("write event");
    }
    drop(writer);
  };
  let read = async {
    let mut read_requests = Vec::new();
    for _ in 0..requests.len() {
      read_requests.push(
        reader
          .read::<Request>()
          .await
          .expect("read request")
          .expect("a request"),
      );
    }
    let mut read_events = Vec::new();
    while let Some(event) = reader.read_event().await.expect("read event") {
      read_events.push(event);
    }
    (read_requests, read_events)
  };
  let ((), (read_requests, read_events)) = tokio::join!(written, read);
  assert_eq!(read_requests, requests);
  assert_eq!(read_events, events);
}

/// A frame's JPEG travels after its JSON head, between JSON events, and arrives byte for byte.
#[tokio::test]
async fn browser_frame_is_a_binary_frame_between_json_events() {
  let (mut reader, mut writer) = connection();
  let jpeg: Vec<u8> = (0..=255).cycle().take(200_000).collect();
  let sent = [
    Event::Browser(BrowserEvent::DialogClosed { tab: tab("T1") }),
    Event::BrowserFrame(frame(41, jpeg)),
    Event::BrowserFrame(frame(42, Vec::new())),
    Event::Browser(BrowserEvent::DialogClosed { tab: tab("T1") }),
  ];
  let written = async {
    for event in &sent {
      writer.write_event(event).await.expect("write event");
    }
    drop(writer);
  };
  let read = async {
    let mut events = Vec::new();
    while let Some(event) = reader.read_event().await.expect("read event") {
      events.push(event);
    }
    events
  };
  let ((), received) = tokio::join!(written, read);
  assert_eq!(received, sent);
}

/// A frame whose JPEG would take the body past the limit is refused before anything is written.
#[tokio::test]
async fn browser_frame_over_the_limit_is_refused_and_nothing_is_written() {
  let (mut reader, mut writer) = connection();
  let jpeg = vec![0xFF; MAX_FRAME_LEN as usize];
  let refused = writer.write_event(&Event::BrowserFrame(frame(1, jpeg))).await;
  assert!(
    matches!(refused, Err(FrameError::TooLarge { max: MAX_FRAME_LEN, .. })),
    "{refused:?}"
  );
  let fits = vec![0xFF; MAX_FRAME_LEN as usize - 1024];
  let sent = Event::BrowserFrame(frame(2, fits));
  let written = async {
    writer.write_event(&sent).await.expect("a frame within the limit");
    drop(writer);
  };
  let read = async {
    let first = reader.read_event().await.expect("read").expect("an event");
    let end = reader.read_event().await.expect("clean end");
    (first, end)
  };
  let ((), (first, end)) = tokio::join!(written, read);
  assert_eq!(first, sent, "only the frame that fits arrives");
  assert_eq!(end, None);
}

/// Writes `header` and `body` as they are, then ends the connection; what reading one event gives.
async fn read_raw(header: u32, body: &[u8], json_only: bool) -> Result<Option<Event>, FrameError> {
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (read, _keep) = tokio::io::split(left);
  let mut reader = FrameReader::new(read);
  let (_, mut raw) = tokio::io::split(right);
  raw.write_all(&header.to_be_bytes()).await.expect("write header");
  raw.write_all(body).await.expect("write body");
  drop(raw);
  if json_only {
    reader.read::<Event>().await
  } else {
    reader.read_event().await
  }
}

fn binary_body(head: &[u8], bytes: &[u8]) -> Vec<u8> {
  let mut body = u32::try_from(head.len()).expect("small").to_be_bytes().to_vec();
  body.extend_from_slice(head);
  body.extend_from_slice(bytes);
  body
}

const BINARY: u32 = 1 << 31;

#[tokio::test]
async fn browser_malformed_frames_are_refused() {
  let frame_head = serde_json::to_vec(&Event::BrowserFrame(frame(1, Vec::new()))).expect("JSON");
  let other_head = serde_json::to_vec(&Event::Browser(BrowserEvent::DialogClosed { tab: tab("T1") })).expect("JSON");
  let oversized = read_raw(BINARY | (MAX_FRAME_LEN + 1), &[], false).await;
  assert!(matches!(oversized, Err(FrameError::TooLarge { .. })), "{oversized:?}");

  let body = binary_body(&frame_head, b"jpeg");
  let len = u32::try_from(body.len()).expect("small");
  let json_only = read_raw(BINARY | len, &body, true).await;
  assert!(matches!(json_only, Err(FrameError::Malformed(_))), "{json_only:?}");

  let without_bytes = read_raw(u32::try_from(frame_head.len()).expect("small"), &frame_head, false).await;
  assert!(
    matches!(without_bytes, Err(FrameError::Malformed(_))),
    "{without_bytes:?}"
  );

  let body = binary_body(&other_head, b"stray");
  let len = u32::try_from(body.len()).expect("small");
  let stray = read_raw(BINARY | len, &body, false).await;
  assert!(matches!(stray, Err(FrameError::Malformed(_))), "{stray:?}");

  let mut body = 1000_u32.to_be_bytes().to_vec();
  body.extend_from_slice(&frame_head);
  let len = u32::try_from(body.len()).expect("small");
  let long_head = read_raw(BINARY | len, &body, false).await;
  assert!(matches!(long_head, Err(FrameError::Malformed(_))), "{long_head:?}");

  let no_head_len = read_raw(BINARY | 2, &[0, 0], false).await;
  assert!(matches!(no_head_len, Err(FrameError::Malformed(_))), "{no_head_len:?}");
}

/// Each character of the worst text for JSON takes six bytes, and the limit's text still fits a frame.
#[tokio::test]
async fn browser_selection_at_its_limit_fits_a_frame() {
  let (mut reader, mut writer) = connection();
  let sent = Event::Reply {
    id: RequestId::new(1),
    reply: Reply::Browser(BrowserReply::Selection {
      text: "\u{1f}".repeat(MAX_CLIPBOARD_TEXT),
    }),
  };
  let written = async {
    writer.write_event(&sent).await.expect("the limit's text fits a frame");
    drop(writer);
  };
  let (_, received) = tokio::join!(written, reader.read_event());
  assert_eq!(received.expect("read").expect("an event"), sent);
}

#[test]
fn browser_selection_stays_out_of_a_replys_log_line() {
  let reply = BrowserReply::Selection {
    text: "hunter2".to_owned(),
  };
  assert_eq!(format!("{reply:?}"), "Selection { bytes: 7 }");
}
#[test]
fn browser_viewport_outside_its_bounds_is_refused() {
  assert!(Viewport::new(0, 800, 100).is_none());
  assert!(Viewport::new(1280, Viewport::MAX_SIDE + 1, 100).is_none());
  assert!(Viewport::new(1280, 800, Viewport::MAX_SCALE_PERCENT + 1).is_none());
  assert!(Viewport::new(1280, 800, Viewport::MIN_SCALE_PERCENT - 1).is_none());
  let edge = Viewport::new(Viewport::MAX_SIDE, 1, Viewport::MAX_SCALE_PERCENT).expect("at its bounds");
  assert!((edge.scale() - 4.0).abs() < f64::EPSILON);

  let refused = serde_json::from_str::<Viewport>(r#"{"width":0,"height":800,"scale_percent":100}"#);
  assert!(refused.is_err());
  let read: Viewport = serde_json::from_str(r#"{"width":1280,"height":800,"scale_percent":200}"#).expect("in bounds");
  assert_eq!(read, viewport());
}

/// The platform's real endpoint carries a frame's bytes as the in-memory connection does.
#[tokio::test]
async fn browser_frame_crosses_the_platforms_endpoint() {
  let project = tempfile::tempdir().expect("temporary directory");
  let endpoint = Endpoint::for_project(project.path()).expect("endpoint");
  let accepting = {
    let endpoint = endpoint.clone();
    tokio::spawn(async move { endpoint.accept().await })
  };
  let client = tokio::time::timeout(PATIENCE, async {
    loop {
      match endpoint.connect().await {
        Ok(stream) => return stream,
        Err(error) if Endpoint::not_ready(&error) => tokio::time::sleep(Duration::from_millis(10)).await,
        Err(error) => panic!("cannot connect: {error}"),
      }
    }
  })
  .await
  .expect("connected in time");
  let server = tokio::time::timeout(PATIENCE, accepting)
    .await
    .expect("accepted in time")
    .expect("accept task")
    .expect("accept");

  let jpeg: Vec<u8> = (0..=255).rev().cycle().take(3 * 1024 * 1024).collect();
  let sent = Event::BrowserFrame(frame(7, jpeg));
  let (_, write) = tokio::io::split(server);
  let (read, _keep) = tokio::io::split(client);
  let mut writer = FrameWriter::new(write);
  let mut reader = FrameReader::new(read);
  let written = async {
    writer.write_event(&sent).await.expect("write the frame");
  };
  let (_, received) = tokio::join!(written, reader.read_event());
  assert_eq!(received.expect("read").expect("an event"), sent);
}
