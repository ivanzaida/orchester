//! The SSE parser: comments, multi-line data, CRLF and chunks split anywhere.

use orchester_agent::sse::{SseEvent, SseParser};

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
  let events = parser.feed(stream.as_bytes());
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
    events.extend(parser.feed(&[*byte]));
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
  assert!(parser.feed(b"event: a\ndata: 1\n").is_empty());
  assert_eq!(parser.feed(b"\n"), vec![event(Some("a"), "1")]);
}
