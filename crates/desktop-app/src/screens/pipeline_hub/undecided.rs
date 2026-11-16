//! What a row, a chip and *Ready?* say of a pipeline whose profiles and tools cannot all be told to be ready
//! (`Pipelines · v2` P8 `vujkk`): the line the list shows ([`Said`]), and the words for a profile that lost its
//! connection, which the Agents hub fixes on the profile.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_workspace::{pipelines::ready::UndecidedMember, readiness::Undecided};

use crate::screens::local_hub::words::undecided;

/// What a row says and the color it says it in.
pub(super) struct Said {
  pub text: Arc<str>,
  pub color: PaletteColor,
}

/// Why `pipeline`, standing undecided for `why`, cannot be told to be ready, as the list line and the chip say it: the
/// first profile that lost its connection and how many more did (*Explorer and 3 more have no connection*); otherwise
/// the archived resource it names, else that nobody can tell yet. `members` are all the profiles and tools that cannot
/// be told ([`UndecidedMember`]).
pub(super) fn undecided_words(ctx: &Ctx, why: &Undecided, members: &[UndecidedMember]) -> Arc<str> {
  let first_is_gone = matches!(why, Undecided::Member { why, .. } if matches!(**why, Undecided::ConnectionGone(_)));
  let mut gone = members
    .iter()
    .filter(|member| matches!(member.why, Undecided::ConnectionGone(_)));
  match gone.next() {
    Some(first) if first_is_gone => {
      let more = gone.count();
      if more == 0 {
        ctx.t_args("pipeline_hub.line.connectionGone", [("name", first.name.to_string())])
      } else {
        ctx.t_args(
          "pipeline_hub.line.connectionGoneMany",
          [("name", first.name.to_string()), ("count", more.to_string())],
        )
      }
    }
    _ => undecided(ctx, why),
  }
}

/// What *Ready?* says in the row of one profile or tool that cannot be told: *Its connection is gone*.
pub(super) fn member_words(ctx: &Ctx, why: &Undecided) -> Arc<str> {
  match why {
    Undecided::ConnectionGone(_) => ctx.t("pipeline_hub.ready.connectionGone"),
    other => undecided(ctx, other),
  }
}
