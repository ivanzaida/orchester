//! Who merges a team task in no module (the person's decision of 2026-11-13). Its run lands on the base branch
//! itself, where a run on a module's branch would go through the module's delivery on an Admin's authorization or
//! the team's Automatic merge; so an Admin merges it, and a member only while Automatic merge is on. The rule is told
//! on the pages that offer the merge: the work and the branches are this computer's, and no server allows a merge
//! here.

use lurq::app::ctx::Ctx;

use super::{
  TeamSource,
  queries::team_policy,
  view::{self, Standing},
};
use crate::project::ShownProject;

/// Whether the person with `standing` in `source`'s project waits for an Admin or Automatic merge before such a run
/// merges: a member, while the team's policy is not read or has Automatic merge off.
pub fn member_waits(ctx: &mut Ctx, source: &TeamSource, standing: &Standing) -> bool {
  if !standing.member() {
    return false;
  }
  let policy = ctx.query(team_policy(source.clone()));
  !policy.data().is_some_and(|policy| policy.policy.automatic_merge)
}

/// [`member_waits`] on a page that has not read the team's side of `project`. While the person's standing is not
/// read, or cannot be, nobody waits: an Admin away from the server still merges what this computer made.
pub fn member_waits_in(ctx: &mut Ctx, project: &ShownProject) -> bool {
  view::provide_client(ctx);
  let Ok(Some(source)) = view::source(ctx, project) else {
    return false;
  };
  let Ok(Some(standing)) = view::standing(ctx, &source) else {
    return false;
  };
  member_waits(ctx, &source, &standing)
}
