//! *Copy details* (`ke67r` in 09b): the run, how it stands, the stage and cause of its last attempt, its branch, the
//! branch it merges into and the revisions, one per line, for an issue or a message. A cause holds what the OS, Git or
//! Orchester said, with a URL's user part replaced (`orchester_core::integration::preflight`): Orchester gives no
//! secret value to the Git it runs, so none is copied.

use lurq::app::ctx::Ctx;
use orchester_core::integration::attempt::StageResult;
use orchester_workspace::tasks::merge_attempts::page::AttemptPage;

use super::{
  lines::cause,
  words::{asked, attempt_time, stage_name},
};

/// The details of `page`, its status line's words `status`.
pub(super) fn details(ctx: &Ctx, page: &AttemptPage, status: &str) -> String {
  let mut lines = vec![
    ctx
      .t_args(
        "merges.page.details.title",
        [
          ("key", page.run.key.to_string()),
          ("run", page.run.number.get().to_string()),
          ("title", page.run.title.to_string()),
        ],
      )
      .to_string(),
    ctx
      .t_args("merges.page.details.status", [("status", status)])
      .to_string(),
  ];
  if let Some(last) = page.last() {
    let attempt = &last.attempt;
    let refused = attempt
      .stages()
      .iter()
      .find(|(_, result)| *result == StageResult::Refused);
    if let Some((stage, _)) = refused {
      lines.push(
        ctx
          .t_args("merges.page.details.stage", [("stage", stage_name(ctx, *stage))])
          .to_string(),
      );
    }
    if attempt.cause().is_some() {
      lines.push(
        ctx
          .t_args("merges.page.details.cause", [("cause", cause(ctx, attempt.cause()))])
          .to_string(),
      );
    }
    lines.push(
      ctx
        .t_args(
          "merges.page.details.attempt",
          [
            ("number", last.sequence.to_string()),
            ("time", attempt_time(ctx, attempt.at(), true).to_string()),
            ("by", asked(ctx, attempt).to_string()),
          ],
        )
        .to_string(),
    );
  }
  lines.push(
    ctx
      .t_args("merges.page.details.branch", [("branch", page.branch.as_str())])
      .to_string(),
  );
  if let Some(base) = &page.run.base {
    let into = match &page.into_head {
      Some(head) => ctx.t_args(
        "merges.page.properties.intoValue",
        [("branch", base.as_str()), ("revision", head.as_str())],
      ),
      None => base.as_str().into(),
    };
    lines.push(ctx.t_args("merges.page.details.into", [("into", into)]).to_string());
  }
  if let Some(reviewed) = &page.reviewed {
    lines.push(
      ctx
        .t_args("merges.page.details.reviewed", [("revision", reviewed.as_str())])
        .to_string(),
    );
  }
  if let Some(merge) = &page.merge {
    lines.push(
      ctx
        .t_args("merges.page.details.merged", [("revision", merge.revision.as_str())])
        .to_string(),
    );
  }
  lines.join("\n")
}
