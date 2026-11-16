//! The words of the starter a new pipeline can begin from (`Starter · Feature delivery`): each step's name and
//! instructions, and the name of the profile or tool it looks for in the project, e.g. "Explorer" for Research.

use lurq::app::ctx::Ctx;
use orchester_core::{
  name::Name,
  resource::{PromptText, StarterStep, StarterStepText},
};
use orchester_workspace::{i18n::NAMESPACE, pipelines::library::StarterText};

pub(super) fn starter(ctx: &Ctx) -> StarterText {
  StarterText::new(|step| {
    let area = match step {
      StarterStep::Research => "research",
      StarterStep::Plan => "plan",
      StarterStep::Implement => "implement",
      StarterStep::RunChecks => "runChecks",
      StarterStep::Review => "review",
      StarterStep::CreatePullRequest => "createPullRequest",
    };
    let word = |part| ctx.t_ns(NAMESPACE, &format!("pipelines.starter.{area}.{part}"));
    (
      StarterStepText {
        name: Name::new(&word("name")).expect("translated starter name"),
        instructions: PromptText::new(&word("instructions")).expect("translated starter prompt"),
      },
      word("uses").to_string(),
    )
  })
}
