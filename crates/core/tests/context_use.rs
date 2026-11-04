//! How full each agent session's context is (ORC-304), read from what the CLIs print: Claude Code's stream-json
//! usage of each message against the `contextWindow` its result reports, Codex App Server's `thread/tokenUsage/updated`
//! against the `modelContextWindow` it names. A CLI that reports no tokens gives no reading: unknown, never 0%.

use orchester_core::connection::{
  app_server::AppServerStream,
  context_use::{ContextUse, Fullness, estimated_window},
  stream::{ClaudeStream, OutputStream},
};
use serde_json::json;

/// The reading after each line of `fixture`.
fn claude_readings(fixture: &str) -> Vec<Option<ContextUse>> {
  let mut stream = ClaudeStream::default();
  fixture
    .lines()
    .map(|line| {
      stream.parse(OutputStream::Stdout, line);
      stream.context()
    })
    .collect()
}

#[test]
fn claude_context_follows_each_message_and_estimates_the_window_until_a_result_reports_it() {
  let readings = claude_readings(include_str!("fixtures/claude/synthetic-tool-use.jsonl"));
  assert_eq!(readings[0], None, "the init line reports no tokens");
  // The first message: input 12, cache writes 5,000, cache reads 1,000. Its result's modelUsage is empty, so the
  // window is the table's for the Haiku model, as an estimate.
  assert_eq!(
    readings[1],
    Some(ContextUse::of_model(6_012, "claude-haiku-4-5-20251001"))
  );
  assert!(readings[1].is_some_and(|reading| reading.estimated() && reading.window() == Some(200_000)));
  // The second message: 8 + 300 + 6,000; the third: 5 + 0 + 6,300.
  assert_eq!(readings[5].map(|reading| reading.used()), Some(6_308));
  assert_eq!(
    readings.last().copied().flatten().map(|reading| reading.used()),
    Some(6_305)
  );
}

#[test]
fn claude_context_reads_against_the_window_the_result_reports() {
  let mut stream = ClaudeStream::default();
  let message = r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-opus-4-6","role":"assistant","type":"message","content":[{"type":"text","text":"Done."}],"usage":{"input_tokens":3,"output_tokens":9,"cache_creation_input_tokens":2000,"cache_read_input_tokens":40000}},"parent_tool_use_id":null,"session_id":"s"}"#;
  stream.parse(OutputStream::Stdout, message);
  assert_eq!(stream.context(), Some(ContextUse::of_model(42_003, "claude-opus-4-6")));
  let result = r#"{"type":"result","subtype":"success","is_error":false,"result":"Done.","usage":{"input_tokens":3,"output_tokens":9,"cache_creation_input_tokens":2000,"cache_read_input_tokens":40000},"modelUsage":{"claude-opus-4-6":{"inputTokens":3,"outputTokens":9,"cacheReadInputTokens":40000,"cacheCreationInputTokens":2000,"contextWindow":1000000,"maxOutputTokens":64000}},"permission_denials":[]}"#;
  stream.parse(OutputStream::Stdout, result);
  assert_eq!(stream.context(), Some(ContextUse::reported(42_003, 1_000_000)));
}

#[test]
fn claude_context_leaves_out_a_subagents_messages() {
  let mut stream = ClaudeStream::default();
  let main = r#"{"type":"assistant","message":{"id":"msg_1","model":"claude-opus-4-6","content":[{"type":"text","text":"Delegating."}],"usage":{"input_tokens":10,"output_tokens":1,"cache_read_input_tokens":90}},"parent_tool_use_id":null}"#;
  let subagent = r#"{"type":"assistant","message":{"id":"msg_2","model":"claude-haiku-4-5-20251001","content":[{"type":"text","text":"Searching."}],"usage":{"input_tokens":5000,"output_tokens":1}},"parent_tool_use_id":"toolu_9"}"#;
  stream.parse(OutputStream::Stdout, main);
  stream.parse(OutputStream::Stdout, subagent);
  assert_eq!(stream.context().map(|reading| reading.used()), Some(100));
}

#[test]
fn a_claude_cli_without_numbers_gives_unknown() {
  let readings = claude_readings(include_str!("fixtures/claude/not-signed-in-clean-env.jsonl"));
  assert!(readings.iter().all(Option::is_none), "{readings:?}");
}

#[test]
fn codex_context_reads_the_latest_response_against_the_window_codex_names() {
  let mut stream = AppServerStream::default();
  assert_eq!(stream.context(), None);
  let first = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"t","turnId":"u",
    "tokenUsage":{"last":{"inputTokens":9000,"cachedInputTokens":8000,"outputTokens":300,"reasoningOutputTokens":100,"totalTokens":9300},
      "total":{"inputTokens":9000,"cachedInputTokens":8000,"outputTokens":300,"reasoningOutputTokens":100,"totalTokens":9300},
      "modelContextWindow":258400}}});
  stream.parse(&first.to_string()).expect("usage");
  assert_eq!(stream.context(), Some(ContextUse::reported(9_300, 258_400)));
  let second = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"t","turnId":"u",
    "tokenUsage":{"last":{"inputTokens":12000,"cachedInputTokens":9000,"outputTokens":50,"reasoningOutputTokens":0,"totalTokens":12050},
      "total":{"inputTokens":21000,"cachedInputTokens":17000,"outputTokens":350,"reasoningOutputTokens":100,"totalTokens":21350},
      "modelContextWindow":258400}}});
  stream.parse(&second.to_string()).expect("usage");
  assert_eq!(stream.context(), Some(ContextUse::reported(12_050, 258_400)));
}

#[test]
fn codex_context_without_a_named_window_has_no_window_and_without_tokens_is_unknown() {
  let mut stream = AppServerStream::default();
  let empty = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"t","turnId":"u",
    "tokenUsage":{"last":{"inputTokens":0,"outputTokens":0,"totalTokens":0},
      "total":{"inputTokens":0,"outputTokens":0,"totalTokens":0},"modelContextWindow":null}}});
  stream.parse(&empty.to_string()).expect("usage");
  assert_eq!(stream.context(), None, "no tokens: unknown, not 0%");
  let unnamed = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"t","turnId":"u",
    "tokenUsage":{"last":{"inputTokens":700,"outputTokens":20},
      "total":{"inputTokens":700,"outputTokens":20},"modelContextWindow":null}}});
  stream.parse(&unnamed.to_string()).expect("usage");
  assert_eq!(stream.context(), Some(ContextUse::unknown_window(720)));
}

#[test]
fn the_table_of_model_limits_knows_claude_models_only() {
  assert_eq!(estimated_window("claude-sonnet-4-5-20250929"), Some(200_000));
  assert_eq!(estimated_window("opus[1m]"), Some(1_000_000));
  assert_eq!(estimated_window("deepseek-v4-pro"), None);
  let unknown = ContextUse::of_model(10, "deepseek-v4-pro");
  assert_eq!((unknown.window(), unknown.estimated()), (None, false));
}

#[test]
fn fullness_is_near_full_from_80_percent_and_full_at_the_window() {
  let of = |used| ContextUse::reported(used, 200_000);
  assert_eq!(
    (of(159_999).share(), of(159_999).fullness()),
    (Some(79), Fullness::Plenty)
  );
  assert_eq!(
    (of(160_000).share(), of(160_000).fullness()),
    (Some(80), Fullness::NearFull)
  );
  assert_eq!(
    (of(199_999).share(), of(199_999).fullness()),
    (Some(99), Fullness::NearFull)
  );
  assert_eq!(of(200_000).fullness(), Fullness::Full);
  assert_eq!(
    (of(230_000).share(), of(230_000).fullness()),
    (Some(115), Fullness::Full)
  );
  let unknown = ContextUse::unknown_window(720);
  assert_eq!((unknown.share(), unknown.fullness()), (None, Fullness::Unknown));
}
