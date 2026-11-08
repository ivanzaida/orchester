//! The panel of a profile, instruction, skill or tool in a local project (`Detail` in L1;
//! `docs/design/team-configuration.md` rule 11): the editor the kind has, hosted in the hub's panel ([`Hosted`]), so
//! every field is edited where the list is, with the local draft, *Publish vN*, *Versions* and *Archive* as the editor
//! has them; under it, *Ready?* says what the entry still lacks on this computer.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_core::resource::ResourceKind;
use orchester_workspace::local_archive::Archivable;

use super::{model::Entry, panel};
use crate::{
  project::ShownProject,
  routes::{AgentTab, InstructionsPanel, InstructionsTab, InstructionsView, ProjectPage, ToolTab},
  screens::{agents, hub::hosted::Hosted, instructions, skills, team_hub::panel::body, tools},
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
}
impl DevtoolsInspectable for Props {}

/// The panel of `entry`, keyed by it: what is typed in one never carries over to another.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("local-hub-editing-{}", super::row::token(props.entry.what));
  ctx.mount_keyed::<Editing>(&key, props)
}

struct Editing;

impl Component for Editing {
  type Props = Props;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, entry } = ctx.props::<Props>().clone();
    ctx.provide_if_changed(Hosted);
    let Archivable::Resource(id) = entry.what else {
      unreachable!("an MCP server has its own panel");
    };
    let editor = match entry.kind {
      ResourceKind::AgentProfile => agents::screen(ctx, &project, agents::Shown::Edit(id, AgentTab::Profile)),
      ResourceKind::Tool => tools::screen(ctx, &project, &ProjectPage::Tool(id, ToolTab::default())),
      ResourceKind::Instruction => instructions::screen(
        ctx,
        &project,
        InstructionsView::Rule(InstructionsTab::default(), Some(id), InstructionsPanel::Hidden),
      ),
      _ => skills::screen(ctx, &project, &ProjectPage::Skill(id)),
    };
    let sections = body().child(editor).with_children(panel::readiness(ctx, &entry));
    panel::frame(entry.what, sections, None)
  }
}
