//! How full a native conversation's context is (ORC-304): the input the provider counted for the latest response,
//! against the model's window, an estimate when nobody named one.

use orchester_core::{
  connection::context_use::ContextUse,
  native::{model::StopReason, session::LoopInput},
  permission::RepositoryAccess,
};

use super::support::{Layout, assistant, response, settings, start_with};

#[test]
fn native_context_is_unknown_until_a_response_counts_it_then_reads_against_the_window() {
  let layout = Layout::new();
  let (mut native, _) = start_with(
    &layout,
    RepositoryAccess::Write,
    settings(RepositoryAccess::Write, 64_000),
    &[],
  );
  assert_eq!(native.context_use(), None, "no response has counted anything yet");
  native
    .handle(LoopInput::Responded(response(
      vec![assistant("Working.")],
      StopReason::EndTurn,
      12_345,
    )))
    .expect("response");
  assert_eq!(native.context_use(), Some(ContextUse::reported(12_345, 64_000)));
}

#[test]
fn native_context_reads_against_an_assumed_window_as_an_estimate() {
  let layout = Layout::new();
  let mut assumed = settings(RepositoryAccess::Write, 200_000);
  assumed.context_window_estimated = true;
  let (mut native, _) = start_with(&layout, RepositoryAccess::Write, assumed, &[]);
  native
    .handle(LoopInput::Responded(response(
      vec![assistant("Working.")],
      StopReason::EndTurn,
      900,
    )))
    .expect("response");
  let context = native.context_use().expect("a reading");
  assert_eq!(
    (context.used(), context.window(), context.estimated()),
    (900, Some(200_000), true)
  );
}
