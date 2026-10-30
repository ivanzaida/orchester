//! The runner's DevTools Protocol client against a stand-in for the browser's debugging WebSocket, served by the test
//! on loopback: answers that come out of order reach the commands they answer by id, commands go out in the order of
//! their calls, an error answer becomes [`CdpError::Refused`], a command without an answer times out, events reach the
//! listeners of their own session, and a connection the browser drops fails the commands awaiting an answer and every
//! later one as closed. Run with `cargo test -p orchester-runner --test cdp cdp_`.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use orchester_runner::browser::cdp::{Cdp, CdpError, CdpEvent, SessionId, TargetId};
use serde_json::{Value, json};
use tokio::{
  net::{TcpListener, TcpStream},
  sync::mpsc,
};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};

/// How long a test waits for what must come.
const PATIENCE: Duration = Duration::from_secs(10);

/// The browser's side of the connection: the test reads each command the client sends and writes what the browser
/// would say.
struct FakeBrowser {
  socket: WebSocketStream<TcpStream>,
}

impl FakeBrowser {
  /// The next command the client sent.
  async fn command(&mut self) -> Value {
    loop {
      let message = tokio::time::timeout(PATIENCE, self.socket.next())
        .await
        .expect("a command in time")
        .expect("the client is connected")
        .expect("a message");
      if let Message::Text(text) = message {
        return serde_json::from_str(text.as_str()).expect("a command is JSON");
      }
    }
  }

  async fn send(&mut self, message: Value) {
    self
      .socket
      .send(Message::text(message.to_string()))
      .await
      .expect("sent to the client");
  }

  /// Answers the command `id` with `result`.
  async fn answer(&mut self, id: &Value, result: Value) {
    self.send(json!({ "id": id, "result": result })).await;
  }
}

/// A client connected to a fake browser.
async fn connected() -> (Cdp, FakeBrowser) {
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
  let address = listener.local_addr().expect("its address");
  let accepting = tokio::spawn(async move {
    let (stream, _) = listener.accept().await.expect("the client connects");
    accept_async(stream).await.expect("a WebSocket")
  });
  let cdp = Cdp::connect(&format!("ws://{address}/devtools/browser/fake"))
    .await
    .expect("connected");
  let socket = accepting.await.expect("accepted");
  (cdp, FakeBrowser { socket })
}

/// The session the fake browser gives the client for `target`.
async fn attached(cdp: &Cdp, browser: &mut FakeBrowser, target: &str) -> SessionId {
  let id = TargetId::from(target);
  let (session, ()) = tokio::join!(cdp.attach(&id), async {
    let command = browser.command().await;
    assert_eq!(command["method"], "Target.attachToTarget");
    assert_eq!(command["params"], json!({ "targetId": target, "flatten": true }));
    browser
      .answer(&command["id"], json!({ "sessionId": format!("session-{target}") }))
      .await;
  });
  session.expect("attached")
}

async fn next_event(events: &mut mpsc::UnboundedReceiver<CdpEvent>) -> CdpEvent {
  tokio::time::timeout(PATIENCE, events.recv())
    .await
    .expect("an event in time")
    .expect("still listening")
}

#[tokio::test]
async fn cdp_answers_that_come_out_of_order_reach_the_commands_they_answer() {
  let (cdp, mut browser) = connected().await;
  let session = attached(&cdp, &mut browser, "tab-1").await;

  let (first, second, ()) = tokio::join!(
    cdp.call(None, "Browser.getVersion", json!({})),
    cdp.call(Some(&session), "Runtime.evaluate", json!({ "expression": "1 + 1" })),
    async {
      let (version, evaluate) = (browser.command().await, browser.command().await);
      assert_ne!(version["id"], evaluate["id"], "each command has its own id");
      assert_eq!(
        version.get("sessionId"),
        None,
        "the browser's own command has no session"
      );
      assert_eq!(evaluate["sessionId"], "session-tab-1");
      assert_eq!(evaluate["params"], json!({ "expression": "1 + 1" }));
      browser
        .answer(&evaluate["id"], json!({ "result": { "value": 2 } }))
        .await;
      browser.answer(&version["id"], json!({ "product": "Fake/1.0" })).await;
    }
  );

  assert_eq!(first.expect("answered"), json!({ "product": "Fake/1.0" }));
  assert_eq!(second.expect("answered"), json!({ "result": { "value": 2 } }));
}

/// A command goes out when it is called, so commands whose futures run in any order reach the browser in the order of
/// their calls.
#[tokio::test]
async fn cdp_commands_go_out_in_the_order_of_their_calls() {
  let (cdp, mut browser) = connected().await;
  let (first, second) = (
    cdp.call(None, "Input.first", json!({})),
    cdp.call(None, "Input.second", json!({})),
  );

  let (second, first, ()) = tokio::join!(second, first, async {
    let (one, two) = (browser.command().await, browser.command().await);
    assert_eq!(one["method"], "Input.first");
    assert_eq!(two["method"], "Input.second");
    browser.answer(&two["id"], json!({ "second": true })).await;
    browser.answer(&one["id"], json!({ "first": true })).await;
  });

  assert_eq!(first.expect("answered"), json!({ "first": true }));
  assert_eq!(second.expect("answered"), json!({ "second": true }));
}

#[tokio::test]
async fn cdp_error_answer_becomes_the_refusal_with_its_code_and_message() {
  let (cdp, mut browser) = connected().await;

  let (refused, ()) = tokio::join!(cdp.call(None, "Nope.method", json!({})), async {
    let command = browser.command().await;
    browser
      .send(json!({
        "id": command["id"],
        "error": { "code": -32601, "message": "'Nope.method' wasn't found" },
      }))
      .await;
  });

  assert_eq!(
    refused,
    Err(CdpError::Refused {
      method: "Nope.method".to_owned(),
      code: -32601,
      message: "'Nope.method' wasn't found".to_owned(),
    })
  );
}

/// A command the browser does not answer within its limit times out; its answer, coming late, is dropped and never
/// taken for the answer of a later command.
#[tokio::test]
async fn cdp_command_without_an_answer_times_out_and_its_late_answer_is_dropped() {
  let (cdp, mut browser) = connected().await;
  let limit = Duration::from_millis(200);

  let (timed_out, unanswered) = tokio::join!(cdp.call_within(None, "Page.slow", json!({}), limit), browser.command());
  assert_eq!(
    timed_out,
    Err(CdpError::TimedOut {
      method: "Page.slow".to_owned(),
      limit,
    })
  );

  let (later, ()) = tokio::join!(cdp.call(None, "Page.fast", json!({})), async {
    let command = browser.command().await;
    browser.answer(&unanswered["id"], json!({ "late": true })).await;
    browser.answer(&command["id"], json!({ "fast": true })).await;
  });
  assert_eq!(later.expect("answered"), json!({ "fast": true }));
  assert!(!cdp.is_closed());
}

/// Each event goes to the listeners of the session it carries, or of the browser's own without one; a session no longer
/// followed ends its listeners.
#[tokio::test]
async fn cdp_events_reach_the_listeners_of_their_own_session() {
  let (cdp, mut browser) = connected().await;
  let (first, second) = (
    attached(&cdp, &mut browser, "tab-1").await,
    attached(&cdp, &mut browser, "tab-2").await,
  );
  let mut firsts = cdp.subscribe(Some(first.clone()));
  let mut seconds = cdp.subscribe(Some(second.clone()));
  let mut browsers = cdp.subscribe(None);
  assert_eq!(cdp.listeners(), 3);

  let event = |method: &str, session: Option<&SessionId>| {
    let mut event = json!({ "method": method, "params": { "of": method } });
    if let Some(session) = session {
      event["sessionId"] = json!(session.as_str());
    }
    event
  };
  browser.send(event("Page.loadEventFired", Some(&first))).await;
  browser.send(event("Page.frameNavigated", Some(&second))).await;
  browser.send(event("Target.targetCreated", None)).await;
  browser.send(event("Page.domContentEventFired", Some(&first))).await;

  let methods = |events: &[CdpEvent]| events.iter().map(|event| event.method.clone()).collect::<Vec<_>>();
  let of_first = [next_event(&mut firsts).await, next_event(&mut firsts).await];
  assert_eq!(methods(&of_first), ["Page.loadEventFired", "Page.domContentEventFired"]);
  assert_eq!(of_first[0].session.as_ref(), Some(&first));
  assert_eq!(of_first[0].params, json!({ "of": "Page.loadEventFired" }));
  // The events came in order on one connection: the last one routed means every earlier one was.
  let of_second = next_event(&mut seconds).await;
  assert_eq!(
    (of_second.method.as_str(), of_second.session.as_ref()),
    ("Page.frameNavigated", Some(&second))
  );
  let of_browser = next_event(&mut browsers).await;
  assert_eq!(
    (of_browser.method.as_str(), of_browser.session),
    ("Target.targetCreated", None)
  );
  for events in [&mut firsts, &mut seconds, &mut browsers] {
    assert!(events.try_recv().is_err(), "no event of another session");
  }

  cdp.unsubscribe(&first);
  assert_eq!(firsts.recv().await, None, "the session is no longer followed");
  assert_eq!(cdp.listeners(), 2);
}

/// The browser drops the connection: the command awaiting an answer fails as closed, every listener ends, and every
/// later command fails as closed at once.
#[tokio::test]
async fn cdp_dropped_connection_fails_pending_and_later_calls_as_closed() {
  let (cdp, mut browser) = connected().await;
  let mut events = cdp.subscribe(None);

  let (pending, ()) = tokio::join!(cdp.call(None, "Page.never", json!({})), async move {
    browser.command().await;
    drop(browser);
  });

  assert_eq!(pending, Err(CdpError::Closed));
  assert!(cdp.is_closed());
  let ended = tokio::time::timeout(PATIENCE, events.recv())
    .await
    .expect("the listener ends in time");
  assert_eq!(ended, None);
  assert_eq!(cdp.call(None, "Page.later", json!({})).await, Err(CdpError::Closed));
  assert_eq!(
    cdp.subscribe(None).recv().await,
    None,
    "a closed connection reports nothing"
  );
  assert_eq!(cdp.listeners(), 0);
}
