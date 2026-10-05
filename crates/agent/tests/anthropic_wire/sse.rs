//! The SSE parser: comments, multi-line data, CRLF and chunks split anywhere.

use orchester_agent::sse::{MAX_EVENT, SseError, SseEvent, SseParser};

fn event(name: Option<&str>, data: &str) -> SseEvent {
  SseEvent {
    name: name.map(str::to_owned),
    data: data.to_owned(),
  }
}

#[test]
fn anthropic_sse_skips_comments_and_joins_data_lines() {
  let mut parser = SseParser::new();
  let stream = ": keep-alive\n\nevent: message_start\ndata: {\"a\":\ndata: 1}\n\n:ping\ndata: plain\n\n";
  let events = parser.feed(stream.as_bytes()).expect("events");
  assert_eq!(
    events,
    vec![event(Some("message_start"), "{\"a\":\n1}"), event(None, "plain")]
  );
}

#[test]
fn anthropic_sse_reads_events_split_at_every_byte() {
  let stream = "event: delta\r\ndata: {\"text\":\"héllo ✓\"}\r\n\r\nevent: stop\ndata:{}\n\n";
  let mut parser = SseParser::new();
  let mut events = Vec::new();
  for byte in stream.as_bytes() {
    events.extend(parser.feed(&[*byte]).expect("events"));
  }
  assert_eq!(
    events,
    vec![
      event(Some("delta"), "{\"text\":\"héllo ✓\"}"),
      event(Some("stop"), "{}")
    ]
  );
}

#[test]
fn anthropic_sse_waits_for_the_empty_line() {
  let mut parser = SseParser::new();
  assert!(parser.feed(b"event: a\ndata: 1\n").expect("events").is_empty());
  assert_eq!(parser.feed(b"\n"), Ok(vec![event(Some("a"), "1")]));
}

#[test]
fn anthropic_sse_reads_lone_cr_and_drops_a_bom() {
  let stream = "\u{feff}event: a\rdata: 1\r\rdata: 2\r";
  let mut parser = SseParser::new();
  let mut events = Vec::new();
  for byte in stream.as_bytes() {
    events.extend(parser.feed(&[*byte]).expect("events"));
  }
  events.extend(parser.feed(b"\n\n").expect("events"));
  assert_eq!(events, vec![event(Some("a"), "1"), event(None, "2")]);
}

#[test]
fn anthropic_sse_refuses_an_endless_event() {
  let mut parser = SseParser::new();
  let chunk = vec![b'x'; 1024 * 1024];
  let mut fed = 0;
  let error = loop {
    match parser.feed(&chunk) {
      Ok(_) => fed += chunk.len(),
      Err(error) => break error,
    }
  };
  assert_eq!(error, SseError::TooLong);
  assert!(fed <= MAX_EVENT, "{fed}");
}
