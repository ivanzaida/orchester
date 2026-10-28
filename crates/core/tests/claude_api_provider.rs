//! Claude Code's usage on an Anthropic-compatible third-party API (ORC-146). `deepseek-turn` is written by hand after
//! the events a run of Claude Code on DeepSeek's Anthropic API (`https://api.deepseek.com/anthropic`) recorded on
//! 2026-10-28: every assistant message carries the provider's `message_start` usage, whose `output_tokens` is 0, and
//! only the result that ends the turn reports what the model wrote; its `thinking_tokens` events are estimates, never
//! counted. No key or captured secret is in it.
//! `cargo test -p orchester-core --test claude_api_provider claude_api_provider`.

use orchester_core::{
  connection::stream::{AgentEvent, ClaudeStream, InvocationTracker, OutputStream},
  run::TokenUsage,
};

const TURN: &str = include_str!("fixtures/claude/deepseek-turn.jsonl");

/// The usage counted after each line of `lines`.
fn counted(lines: &[&str]) -> Vec<TokenUsage> {
  let (mut stream, mut tracker) = (ClaudeStream::default(), InvocationTracker::default());
  lines
    .iter()
    .map(|line| {
      for event in stream.parse(OutputStream::Stdout, line) {
        tracker.observe(&event);
      }
      tracker.usage()
    })
    .collect()
}

#[test]
fn claude_api_provider_output_arrives_with_the_result_of_the_turn() {
  let lines: Vec<&str> = TURN.lines().collect();
  let after = counted(&lines);
  let before_result = after[after.len() - 2];
  assert_eq!(
    before_result,
    TokenUsage {
      input: 13_883 + 3_187 + 14_080,
      cached_input: 14_080,
      output: 0,
    },
    "the messages report what the model read, nothing it wrote"
  );
  assert!(
    before_result.output_unreported(),
    "until the result, the output is unknown, not none"
  );
  let total = after[after.len() - 1];
  assert_eq!(
    total,
    TokenUsage {
      input: 17_070 + 14_080,
      cached_input: 14_080,
      output: 2_140,
    },
    "the result's usage holds the turn's output"
  );
  assert!(!total.output_unreported());
}

#[test]
fn claude_api_provider_thinking_token_estimates_are_never_counted() {
  let estimate = TURN
    .lines()
    .find(|line| line.contains("thinking_tokens"))
    .expect("the fixture has an estimate");
  let mut stream = ClaudeStream::default();
  let events = stream.parse(OutputStream::Stdout, estimate);
  assert!(
    events
      .iter()
      .all(|event| !matches!(event, AgentEvent::TurnCompleted { .. })),
    "{events:?}"
  );
}
