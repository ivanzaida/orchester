//! The Orchestrator chat's own commands in its composer's slash menu (ORC-321, `07` `jAiyu`): *New conversation*,
//! which the header's *New* starts, and *History*, which opens the History panel as the header's toggle does. Compact
//! and Model and effort join them once their controls exist (ORC-309, ORC-319); until then they are not offered. The
//! watch commands follow them (ORC-771, `01 · Watch commands` `h1KHAx`): *Watch modules* (`/watch-module`) and *Watch
//! tasks* (`/watch-task`) take an argument, chosen in their pickers; *Stop watching* (`/unwatch`) and *What I watch*
//! (`/watching`, ORC-775) are faded while the conversation watches nothing. The project's published skills follow them
//! (ORC-325), greyed while the Orchestrator's connection cannot take one.

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
    slash::{self, Does, SkillsOffer, SlashCommand, SlashMenuProps},
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
  let mut commands = vec![new_conversation(ctx, project, fresh), history(ctx, view)];
  commands.extend(watch_commands(ctx, view, project));
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
      does: None,
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
    does: Some(Does::Run(Arc::new(move || navigation.go(route.clone())))),
  }
}

fn history(ctx: &Ctx, view: &View) -> SlashCommand {
  let open = view.history.clone();
  SlashCommand {
    name: "history",
    icon: Icon::History,
    label: ctx.t("composer.slash.command.history.label"),
    detail: ctx.t("composer.slash.command.history.detail"),
    does: Some(Does::Run(Arc::new(move || open.set(true)))),
  }
}

/// The watch commands: *What I watch* ([`super::watching`], ORC-775), offered while the conversation leads something;
/// faded with why, *Watch modules* and *Watch tasks* until their pickers and the subscriptions they make exist
/// (ORC-772 and ORC-773 give them `Does::Take` with their `ui::slash::ArgumentPicker`), and *Stop watching* while the
/// conversation watches nothing, which it says until its command exists.
fn watch_commands(ctx: &Ctx, view: &View, project: &ShownProject) -> [SlashCommand; 4] {
  let faded = |name, icon, label: Arc<str>, detail: Arc<str>| SlashCommand {
    name,
    icon,
    label,
    detail,
    does: None,
  };
  let not_yet = ctx.t("composer.slash.command.watch.notYet");
  let nothing = ctx.t("composer.slash.command.watch.nothing");
  [
    faded(
      "watch-module",
      Icon::Eye,
      ctx.t("composer.slash.command.watchModule.label"),
      not_yet.clone(),
    ),
    faded(
      "watch-task",
      Icon::ScanEye,
      ctx.t("composer.slash.command.watchTask.label"),
      not_yet,
    ),
    faded(
      "unwatch",
      Icon::EyeOff,
      ctx.t("composer.slash.command.unwatch.label"),
      nothing,
    ),
    view
      .what_i_watch
      .command(ctx, project.open.repo_root.clone(), &view.follow),
  ]
}
