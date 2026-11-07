//! The review page's words that several of its parts say: the lines a thread is about, the agent that answers, and a
//! count in its one or other form.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::pull_request::feedback::LineSpan;
use orchester_workspace::tasks::review_threads::page::ReviewPage;

/// *line 46* or *lines 52–53*.
pub(super) fn lines(ctx: &Ctx, span: LineSpan) -> Arc<str> {
  if span.start() == span.end() {
    ctx.t_args("review_page.thread.line", [("line", span.start().to_string())])
  } else {
    ctx.t_args(
      "review_page.thread.lines",
      [("start", span.start().to_string()), ("end", span.end().to_string())],
    )
  }
}

/// *Implement · Builder*: the implementing step and its agent; *Implement* while the run has none.
pub(super) fn agent(ctx: &Ctx, page: &ReviewPage) -> Arc<str> {
  match page.agent() {
    Some((step, Some(agent))) => ctx.t_args("review_page.thread.agent", [("step", step), ("agent", agent)]),
    Some((step, None)) => step.into(),
    None => ctx.t("review_page.thread.implement"),
  }
}

/// `count` in the words of `one` when it is one, else of `other` with `{{count}}`.
pub(super) fn counted(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> Arc<str> {
  if count == 1 {
    ctx.t(one)
  } else {
    ctx.t_args(other, [("count", count.to_string())])
  }
}
