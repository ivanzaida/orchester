//! The Responses stream as servers frame it: keep-alive comments, an event's data over several lines,
//! and chunks split inside lines. The parser's own cases (characters split across chunks among them)
//! are in `anthropic_wire`.

use std::time::Duration;

use orchester_core::native::{item::Item, model::StopReason};
use serde_json::json;

use crate::{
  events, respond_once,
  server::{Part, Script},
  start,
};

#[tokio::test]
async fn responses_stream_reads_comments_multi_line_data_and_split_chunks() {
  let added = json!({"type": "response.output_item.added", "output_index": 0,
    "item": {"type": "message", "id": "msg_0", "role": "assistant", "status": "in_progress", "content": []}});
  let delta =
    r#"{"type":"response.output_text.delta","output_index":0,"item_id":"msg_0","content_index":0,"delta":"héllo ✓"}"#;
  // Data lines are joined with a line break, so the event's JSON is cut where whitespace may go.
  let (line_a, line_b) = delta.split_at(delta.find(r#""delta""#).expect("a delta field"));
  let done = json!({"type": "response.output_item.done", "output_index": 0,
    "item": {"type": "message", "id": "msg_0", "role": "assistant", "status": "completed",
      "content": [{"type": "output_text", "text": "héllo ✓", "annotations": []}]}});
  let parts = vec![
    Part::Raw(": keep-alive\n\n".to_owned()),
    events::created(),
    Part::Raw(format!("event: response.output_item.added\r\ndata: {}\r\n\r\n", added)),
    Part::Raw(format!("event: response.output_text.delta\ndata: {}", &line_a[..10])),
    Part::Pause(Duration::from_millis(20)),
    Part::Raw(format!("{}\ndata: {line_b}\n", &line_a[10..])),
    Part::Raw(": queued\n\n".to_owned()),
    Part::Event(done),
    events::completed(10, &json!(0), 5),
  ];
  let server = start(Script::new().reply_sse(parts)).await;
  let (result, _) = respond_once(&server).await;
  let response = result.expect("a response");
  assert_eq!(
    response.items,
    vec![Item::Assistant {
      text: "héllo ✓".to_owned()
    }]
  );
  assert_eq!(response.stop, StopReason::EndTurn);
  server.finish();
}
