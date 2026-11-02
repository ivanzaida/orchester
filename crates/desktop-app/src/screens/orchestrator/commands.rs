//! The Orchestrator chat's own commands in its composer's slash menu (ORC-321, `07` `jAiyu`): *New conversation*,
//! which the header's *New* starts, and *History*, which opens the History panel as the header's toggle does. Compact
//! and Model and effort join them once their controls exist (ORC-309, ORC-319); until then they are not offered. The
//! project's published skills follow them (ORC-325), greyed while the Orchestrator's connection cannot take one.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::connection::AgentProvider;
use orchester_workspace::skills::offered::taken_by;

use super::{View, send_menu::Press};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  ui::{
    icon::Icon,
    slash::{self, SkillsOffer, SlashCommand, SlashMenuProps},
  },
};

/// The slash menu over the composer; `fresh` while the conversation shown is a new one, which *New conversation* has
/// nothing to leave; `send` sends the draft as typed when no command matches; `provider` is the connection the
/// Orchestrator runs on, when it is known.
pub(super) fn menu(
  ctx: &mut Ctx,
  view: &View,
  project: &ShownProject,
  (fresh, send, provider): (bool, Option<Press>, Option<AgentProvider>),
) -> Element {
  let commands = vec![new_conversation(ctx, project, fresh), history(ctx, view)];
  let providers: Vec<AgentProvider> = provider.into_iter().collect();
  let skills = SkillsOffer {
    root: project.open.repo_root.clone(),
    revision: project.feed.saves.get(),
    refused: (!taken_by(&providers)).then(|| ctx.t("composer.skill.notTaken")),
  };
  slash::menu(
    ctx,
    SlashMenuProps {
      id: "orchestrator-composer",
      field: view.slash.clone(),
      anchor: view.mention_anchor.clone(),
      commands,
      skills,
      note: None,
      send,
    },
  )
}

fn new_conversation(ctx: &mut Ctx, project: &ShownProject, fresh: bool) -> SlashCommand {
  let label = ctx.t("composer.slash.command.newConversation.label");
  if fresh {
    return SlashCommand {
      name: "new-conversation",
      icon: Icon::MessageSquarePlus,
      label,
      detail: ctx.t("composer.slash.command.newConversation.already"),
      run: None,
    };
  }
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    project.open.project.id(),
    ProjectPage::Orchestrator(OrchestratorChat::New),
  );
  SlashCommand {
    name: "new-conversation",
    icon: Icon::MessageSquarePlus,
    label,
    detail: ctx.t("composer.slash.command.newConversation.detail"),
    run: Some(Arc::new(move || navigation.go(route.clone()))),
  }
}

fn history(ctx: &Ctx, view: &View) -> SlashCommand {
  let open = view.history.clone();
  SlashCommand {
    name: "history",
    icon: Icon::History,
    label: ctx.t("composer.slash.command.history.label"),
    detail: ctx.t("composer.slash.command.history.detail"),
    run: Some(Arc::new(move || open.set(true))),
  }
}
