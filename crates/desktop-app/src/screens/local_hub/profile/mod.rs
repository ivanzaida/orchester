//! The panel of a local profile (`Agents · Profile` L1 `fFxoM`, `docs/design/team-configuration.md` rule 11): the head
//! with the version and its *Versions*, the chips (how it stands, who uses it), *Profile* (CLI, connection, model,
//! effort, MCP servers, secrets), *Ready?* with the fix of what is missing and, pinned at the bottom, the fix of the
//! first part: *Use Claude 2 for Builder* when its connection is gone or another was chosen. The parts are the team
//! profile's ([`crate::screens::team_hub::profile`]) over the local profile store: a profile still names its
//! connection, and choosing one publishes the next version on it
//! ([`use_connection`](orchester_workspace::agents::local_connection::use_connection)). The rest of the profile is
//! edited in its editor, which *Edit* opens: the local draft, *Publish vN* and *Versions* are the editor's.

mod ready;
mod table;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::Row,
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::ResourceKind,
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    local_connection::use_connection,
    local_hub::{LocalHub, LocalProfile},
    team_profile::ConnectionChoice,
  },
  background,
  local_archive::Archivable,
  messages::Message,
  readiness::{Group, ReadinessStanding},
  tasks::outside::Outside,
};

use self::ready::Fix;
use super::{
  archive,
  model::{Entry, view_of},
  panel::{chips_of, frame},
  words::kind_where_key,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{AccountsView, AgentTab, HubFilter, ProjectPage, Route},
  screens::{
    hub::versions::{VersionsProps, versions_button},
    team_hub::{
      actions::Busy,
      bar::bar,
      panel::{body, edit_button, head_named},
      profile::{
        fix::Offer,
        parts::{Part, State},
        ready::section,
      },
      words::{cli_key, date},
    },
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    field::field_error,
    icon::Icon,
    message::message_text,
    pressable::Press,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
  pub id: ResourceId,
  pub profile: LocalProfile,
  pub hub: LocalHub,
  /// The hub is read again: what shows is the last answer, and a fix would act on it.
  pub reading: bool,
}
impl DevtoolsInspectable for Props {}

/// The panel of the profile `props.entry`, keyed by it: a choice never carries over to another profile.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<ProfilePanel>(
    &format!("local-hub-profile-{}", super::row::token(props.entry.what)),
    props,
  )
}

struct ProfilePanel {
  /// The connection the person chose in the select, by id; empty while none is.
  chosen: Signal<String>,
  /// Makes the connection (second) the profile's (first) next version.
  take: FutureAction<(ResourceId, ConnectionId), (), Message>,
}

impl Component for ProfilePanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let project = ctx.props::<Props>().project.clone();
    let (root, id) = (project.open.repo_root.clone(), project.open.project.id());
    let services = Services::of(ctx);
    let take = ctx.future_action(move |(profile, connection): (ResourceId, ConnectionId)| {
      let (root, services) = (root.clone(), services.clone());
      async move {
        background::run(move || use_connection(&root, profile, connection).map(drop)).await??;
        services.changes.record(id, Outside::Profiles);
        services.changes.record(id, Outside::Pipelines);
        Ok(())
      }
    });
    Self {
      chosen: ctx.signal(String::new()),
      take,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      project,
      entry,
      id,
      profile,
      hub,
      reading,
    } = ctx.props::<Props>().clone();
    let profile = &profile;
    let parts = ready::parts(ctx, &hub, profile);
    let missing = ready::missing(&parts);
    let navigation = Navigation::of(ctx);
    let busy = self.busy(reading);
    let target = self.target(profile);
    let fix = pinned_fix(profile, target, &parts);
    let press = |fix: &Fix| self.press(fix, (&project, id), target.map(|choice| choice.id), &navigation);
    let head_end = head_end(ctx, &project, &entry, id);
    let body = body()
      .child(head_named(
        &entry.name,
        (
          ctx.t(kind_where_key(entry.kind)).to_string(),
          Some(version_line(ctx, profile, entry.number)),
        ),
        Some(head_end),
      ))
      .child(chips_of(chips(ctx, &entry, profile, &parts), used_by(ctx, profile)))
      .child(table::section(ctx, profile, &self.chosen))
      .child(section(ctx, &parts, missing, (&press, busy)))
      .with_children(self.refusal(ctx));
    let bar = fix.map(|fix| pinned(ctx, &entry, (profile, target, &fix), press(&fix), busy));
    frame(entry.what, body, bar)
  }
}

impl ProfilePanel {
  fn busy(&self, reading: bool) -> Busy {
    if reading {
      Busy::Reading
    } else if self.take.state().get().is_pending() {
      Busy::Taking
    } else {
      Busy::Idle
    }
  }

  /// The connection the select holds, else the first one listed: the one *Use* names and keeps.
  fn target<'a>(&self, profile: &'a LocalProfile) -> Option<&'a ConnectionChoice> {
    let chosen = self.chosen.get();
    profile
      .connections
      .iter()
      .find(|choice| choice.id.to_string() == chosen)
      .or_else(|| profile.connections.first())
  }

  /// What a press on `fix` does.
  fn press(
    &self,
    fix: &Fix,
    (project, profile): (&ShownProject, ResourceId),
    target: Option<ConnectionId>,
    navigation: &Navigation,
  ) -> Press {
    match fix {
      Fix::UseConnection => {
        let take = self.take.clone();
        Press::run(move || {
          if let Some(connection) = target {
            take.run((profile, connection));
          }
        })
      }
      Fix::AddConnection => Press::Open(navigation.to(Route::Accounts(AccountsView::default()))),
      Fix::OpenServer(server) => Press::Open(navigation.to(Route::Project(
        project.open.project.id(),
        ProjectPage::hub(view_of(HubFilter::McpServers, Some(Archivable::McpServer(*server)))),
      ))),
    }
  }

  /// Why the last choice did nothing, as the workspace said.
  fn refusal(&self, ctx: &Ctx) -> Option<Element> {
    let state = self.take.state().get();
    let error = state.error.as_ref().filter(|_| !state.is_pending())?;
    Some(field_error("local-hub-profile-error", &message_text(ctx, error)).into())
  }
}

/// The first fix the panel pins: the connection (*Use* when it is gone or another was chosen, *Add* when the project
/// has none of its CLI), else the first server to fix.
fn pinned_fix(profile: &LocalProfile, target: Option<&ConnectionChoice>, parts: &[Part<Fix>]) -> Option<Fix> {
  let changed = target.is_some_and(|choice| Some(choice.id) != profile.current);
  let connection = match (profile.cli, profile.connections.is_empty(), profile.current) {
    (None, ..) => None,
    (Some(_), true, None) => Some(Fix::AddConnection),
    (Some(_), false, None) => Some(Fix::UseConnection),
    (Some(_), false, Some(_)) if changed => Some(Fix::UseConnection),
    (Some(_), ..) => None,
  };
  connection.or_else(|| {
    parts.iter().find_map(|part| match &part.state {
      State::Missing {
        fix: Some(Fix::OpenServer(server)),
        ..
      } => Some(Fix::OpenServer(*server)),
      _ => None,
    })
  })
}

/// The pinned bar of `fix`, which `press` does: loading while it is taken, disabled while the hub is read again.
fn pinned(
  ctx: &Ctx,
  entry: &Entry,
  (profile, target, fix): (&LocalProfile, Option<&ConnectionChoice>, &Fix),
  press: Press,
  busy: Busy,
) -> Element {
  let connection = target.map_or("", |choice| choice.name.as_str());
  let (words, glyph, hint) = match fix {
    Fix::UseConnection => (
      ctx.t_args(
        "team_hub.fix.action.use",
        [("connection", connection), ("profile", entry.name.as_str())],
      ),
      Icon::Check,
      ctx.t("local_hub.profile.useHint"),
    ),
    Fix::AddConnection => (
      ctx.t_args(
        "team_hub.fix.action.add",
        [(
          "cli",
          &*profile
            .cli
            .map_or_else(|| ctx.t("local_hub.ready.ready"), |cli| ctx.t(cli_key(cli))),
        )],
      ),
      Icon::Plus,
      ctx.t("team_hub.fix.hint.add"),
    ),
    Fix::OpenServer(server) => {
      let name = profile
        .mcp_servers
        .iter()
        .find_map(|(id, name)| (id == server).then_some(name.as_str()))
        .unwrap_or_default();
      (
        ctx.t_args("team_hub.fix.action.setPath", [("server", name)]),
        Icon::FolderOpen,
        ctx.t("local_hub.profile.openHint"),
      )
    }
  };
  let button = ActionButton::new(
    format!("local-hub-fix-{}", fix.name()),
    ButtonKind::Primary,
    words.to_string(),
  )
  .icon(glyph)
  .pinned();
  let button = match busy {
    Busy::Idle => button.press(press),
    Busy::Taking => button.loading(),
    Busy::Reading => button.disabled(),
  };
  bar(vec![button.into()], Some(hint.to_string()))
}

/// *v4 · published Oct 27*.
fn version_line(ctx: &Ctx, profile: &LocalProfile, number: Option<orchester_core::resource::VersionNumber>) -> String {
  match number {
    Some(number) => ctx
      .t_args(
        "local_hub.profile.version",
        [
          ("version", number.to_string()),
          ("date", date(ctx, Timestamp::now(), profile.published_at).to_string()),
        ],
      )
      .to_string(),
    None => ctx.t("local_hub.line.draft").to_string(),
  }
}

/// The chips: the version, then how the profile stands, then that it has edits that are not published.
fn chips(ctx: &Ctx, entry: &Entry, profile: &LocalProfile, parts: &[Part<Fix>]) -> Vec<(Arc<str>, BadgeTone)> {
  let mut chips = Vec::new();
  if let Some(number) = entry.number {
    chips.push((Arc::from(number.to_string()), BadgeTone::Neutral));
  }
  chips.push(match (profile_group(entry), entry.standing.as_ref()) {
    (Group::NeedsYouHere, Some(standing)) => (stands(ctx, entry, profile, parts, standing), BadgeTone::Warning),
    _ => (ctx.t("local_hub.chip.ready"), BadgeTone::Success),
  });
  if profile.draft {
    chips.push((ctx.t("local_hub.chip.draft"), BadgeTone::Info));
  }
  chips
}

const fn profile_group(entry: &Entry) -> Group {
  entry.group
}

/// What the person has to do first, short enough for a chip.
fn stands(
  ctx: &Ctx,
  entry: &Entry,
  profile: &LocalProfile,
  parts: &[Part<Fix>],
  standing: &ReadinessStanding,
) -> Arc<str> {
  let connection = parts
    .first()
    .is_some_and(|part| matches!(part.state, State::Missing { .. }));
  match (connection, profile.connections.len()) {
    (true, 0) => ctx.t("local_hub.chip.addConnection"),
    (true, _) => ctx.t("local_hub.chip.chooseConnection"),
    (false, _) => super::words::needs(ctx, entry, standing),
  }
}

fn used_by(ctx: &Ctx, profile: &LocalProfile) -> Option<Arc<str>> {
  (!profile.pipelines.is_empty())
    .then(|| ctx.t_args("local_hub.panel.usedBy", [("names", profile.pipelines.join(", "))]))
}

/// *Edit*, *Versions* and the … menu, 8 apart.
fn head_end(ctx: &mut Ctx, project: &ShownProject, entry: &Entry, id: ResourceId) -> Element {
  let navigation = Navigation::of(ctx);
  let edit = edit_button(
    ctx,
    navigation.to(Route::Project(
      project.open.project.id(),
      ProjectPage::Agent(id, AgentTab::default()),
    )),
  );
  let versions = versions_button(
    ctx,
    VersionsProps {
      id: "local-hub-profile".into(),
      root: project.open.repo_root.clone(),
      resource: (ResourceKind::AgentProfile, id),
      local: true,
    },
  );
  let more = archive::menu(
    ctx,
    archive::Target {
      root: project.open.repo_root.clone(),
      project: project.open.project.id(),
      what: entry.what,
      kind: entry.kind,
      name: Arc::from(entry.name.as_str()),
    },
  );
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(edit)
    .child(versions)
    .child(more)
    .into()
}
