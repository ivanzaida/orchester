//! *Pick a project* (05 `v5qbWM`): the joined team's address, *Joined*, and the projects the person is invited to in
//! one box, each picked with its radio, the first until the person picks another; *Continue with Kontur* goes to *Link
//! a folder*, or opens the project where a checkout is linked to it already. With none yet (12 `NaEPI`), the empty
//! state says who to ask, with *Check again*.
//!
//! The rows draw no *212 tasks*: `GET /projects` has no count of a project's tasks, and `next_number` counts numbers
//! given, not tasks.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::MemberRole;
use orchester_core::id::{ProjectId, ServerId};
use orchester_store::JoinedWith;
use orchester_workspace::sharing::{ServerProject, cloud::SignIn};

use super::{ID, Team, join_crumb, read};
use crate::{
  navigation::Navigation,
  routes::{JoinTarget, JoinWay, OrchesterView, OverviewTab, ProjectPage, Route},
  screens::{
    join_team::frame::{StartStep, StepFrame, step_frame},
    settings::orchester::{connect::Shown, shown_address},
  },
  services::Services,
  theme::{ExtraColor, ExtraSpacing, IconSize, StartupSpacing, StartupText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, TagChip, badge},
    field::field_error,
    icon::{Icon, icon},
    pressable::{Focus, pressable},
    state::{EmptyState, load_error, loading},
    toggle::radio_mark,
  },
};

const PAGE: &str = "join-team-projects";
/// The empty state's width (`State/Empty` `RLNFt`).
const NONE_WIDTH: f32 = 560.0;

/// The page's screen for the team `server`.
pub(in crate::screens::join_team) fn screen(ctx: &mut Ctx, server: ServerId) -> Element {
  ctx.mount_keyed::<SharedProjects>(&format!("{PAGE}-{server}"), Props { server })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Props {
  server: ServerId,
}

impl DevtoolsInspectable for Props {}

/// The project the person picked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Picked(Option<ProjectId>);

impl DevtoolsInspectable for Picked {}

struct SharedProjects {
  picked: Signal<Picked>,
  /// Counts the reads: *Check again* and *Try again*.
  revision: Signal<u64>,
}

impl Component for SharedProjects {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      picked: ctx.signal(Picked::default()),
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { server } = *ctx.props::<Props>();
    let services = Services::of(ctx);
    let deps = (self.revision.get(), server, services.servers, services.cloud);
    let state = ctx
      .future(deps, |(_, server, servers, cloud)| read(servers, cloud, server))
      .state()
      .get();
    let reading = state.is_pending();
    match (state.data, state.error) {
      (Some(team), failed) => self.page(ctx, &team, (reading, failed.as_ref())),
      (None, Some(reason)) => {
        let (revision, text) = (self.revision.clone(), reason.text(ctx));
        let body = load_error(
          ctx,
          "join-team-projects-failed",
          (&ctx.t("startup.shared.readFailed"), &text),
          move || revision.update(|count| *count += 1),
        );
        waiting_page(ctx, body)
      }
      (None, None) => {
        let body = loading("join-team-projects-reading", &ctx.t("startup.shared.reading")).into();
        waiting_page(ctx, body)
      }
    }
  }
}

/// The page before its team is read: no name to put in its title or crumb yet.
fn waiting_page(ctx: &mut Ctx, body: Element) -> Element {
  let title = ctx.t("startup.shared.projects.titlePending").to_string();
  let frame = StepFrame {
    id: ID,
    ancestors: Vec::new(),
    crumb: ctx.t("startup.join.crumb").to_string(),
    step: StartStep::Project,
    title,
    lead: None,
    body,
    note: None,
  };
  step_frame(ctx, frame)
}

impl SharedProjects {
  /// The team's page; `reading` while *Check again* reads, `failed` when the read after the one shown failed.
  fn page(&self, ctx: &mut Ctx, team: &Team, (reading, failed): (bool, Option<&Shown>)) -> Element {
    let entry = team.entry();
    let name = entry.stored.name.clone();
    let address = shown_address(&entry.stored.url).to_owned();
    let mut parts = vec![server_row(ctx, &address)];
    let lead = if team.projects().is_empty() {
      parts.push(self.none(ctx, team, (&name, &address), reading));
      None
    } else {
      let chosen = self.chosen(team);
      parts.push(self.list(ctx, team, chosen));
      parts.push(actions(ctx, team, chosen));
      Some(
        ctx
          .t_args("startup.shared.projects.lead", [("address", address.as_str())])
          .to_string(),
      )
    };
    parts
      .extend(failed.map(|failed| Element::from(field_error("join-team-projects-recheck-failed", &failed.text(ctx)))));
    let frame = StepFrame {
      id: ID,
      ancestors: vec![join_crumb(ctx)],
      crumb: name.clone(),
      step: StartStep::Project,
      title: ctx
        .t_args("startup.shared.projects.title", [("team", name.as_str())])
        .to_string(),
      lead,
      body: Column::new()
        .id(PAGE)
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xl)
        .with_children(parts)
        .into(),
      note: None,
    };
    step_frame(ctx, frame)
  }

  /// The project picked while it is listed, else the first.
  fn chosen(&self, team: &Team) -> Option<ProjectId> {
    let listed = |id: &ProjectId| team.projects().iter().any(|listed| listed.project.id == *id);
    self
      .picked
      .get()
      .0
      .filter(listed)
      .or_else(|| team.projects().first().map(|first| first.project.id))
  }

  /// The projects in one box (`Project list` `VyAVu`).
  fn list(&self, ctx: &Ctx, team: &Team, chosen: Option<ProjectId>) -> Element {
    let count = team.projects().len();
    Column::new()
      .id(format!("{PAGE}-list"))
      .width(Dimension::Pct(100.0))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .with_children(
        team
          .projects()
          .iter()
          .enumerate()
          .map(|(index, listed)| self.row(ctx, listed, chosen == Some(listed.project.id), index + 1 == count)),
      )
      .into()
  }

  /// A project (`Row · Kontur` `k6rLkP`): its radio, its name and role, its address and repository; picked, on `hover`.
  fn row(&self, ctx: &Ctx, listed: &ServerProject, chosen: bool, last: bool) -> Element {
    let project = &listed.project;
    let id = format!("{PAGE}-{}", project.slug);
    let role = match project.role {
      Some(MemberRole::Owner) => "startup.shared.projects.owner",
      Some(MemberRole::Member) | None => "startup.shared.projects.member",
    };
    let title = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&project.name)
          .id(format!("{id}-title"))
          .variant(StartupText::RowTitle.role())
          .nowrap(),
      )
      .child(TagChip::new(format!("{id}-role"), ctx.t(role)));
    let address = shown_address(&project.address);
    let sub = match &project.remote_hint {
      Some(hint) => ctx
        .t_args(
          "startup.shared.projects.sub",
          [("address", address), ("hint", hint.as_str())],
        )
        .to_string(),
      None => address.to_owned(),
    };
    let (picked, project_id) = (self.picked.clone(), project.id);
    let row = Focus::Keyboard
      .apply(pressable(move || picked.set(Picked(Some(project_id)))))
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
      .spacing(SpacingSize::Md)
      .describe("a11y_name", project.name.clone())
      .describe("checked", chosen.to_string())
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(radio_mark(chosen))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Fine.role())
          .child(title)
          .child(
            Text::new(&sub)
              .id(format!("{id}-sub"))
              .variant(StartupText::Sub.role())
              .nowrap(),
          ),
      );
    let row = if chosen {
      row.background(ExtraColor::Hover.role())
    } else {
      row
    };
    if last {
      row.into()
    } else {
      row
        .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .into()
    }
  }

  /// No project yet (`State/Empty` `RLNFt`): who was not invited, *Check again* and *Back to start*. A team joined
  /// with Orchester Cloud invites the Cloud email; one joined with a code has no Cloud account behind it, so it is
  /// the account there, whoever is signed in to Cloud.
  fn none(&self, ctx: &mut Ctx, team: &Team, (name, address): (&str, &str), reading: bool) -> Element {
    let stored = &team.entry().stored;
    let who = match (stored.joined_with, Services::of(ctx).cloud.sign_in_state()) {
      (Some(JoinedWith::Cloud(_)), SignIn::SignedIn { email, .. }) => email,
      _ => stored
        .account
        .as_ref()
        .map(|account| account.username.clone())
        .unwrap_or_default(),
    };
    let check = ActionButton::new(
      "join-team-projects-check-again",
      ButtonKind::Secondary,
      ctx.t("startup.shared.projects.none.checkAgain"),
    )
    .icon(Icon::RefreshCw);
    let check = if reading {
      check.loading()
    } else {
      let revision = self.revision.clone();
      check.on_click(move || revision.update(|count| *count += 1))
    };
    let back = ActionButton::new(
      "join-team-projects-back-to-start",
      ButtonKind::Ghost,
      ctx.t("startup.shared.projects.none.backToStart"),
    )
    .opens(Navigation::of(ctx).to(Route::AllProjects));
    let state = EmptyState::new(
      "join-team-projects-none",
      Icon::FolderSearch,
      ctx.t_args("startup.shared.projects.none.title", [("team", name)]),
      ctx.t_args(
        "startup.shared.projects.none.body",
        [("address", address), ("who", who.as_str())],
      ),
    )
    .wide()
    .action(check)
    .action(back);
    Column::new()
      .width(Dimension::Px(NONE_WIDTH))
      .max_width(Dimension::Pct(100.0))
      .child(state)
      .into()
  }
}

/// The team's address, *Joined* and what that means (`Server` `yAjlo`).
fn server_row(ctx: &Ctx, address: &str) -> Element {
  Row::new()
    .id("join-team-projects-server")
    .width(Dimension::Pct(100.0))
    .spacing(StartupSpacing::Row.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Server, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(address)
        .id("join-team-projects-address")
        .variant(StartupText::ServerAddress.role())
        .nowrap(),
    )
    .child(badge(
      "join-team-projects-joined",
      BadgeTone::Success,
      &ctx.t("startup.shared.joined"),
    ))
    .child(Text::new(&ctx.t("startup.shared.signedIn")).variant(StartupText::Sub.role()))
    .into()
}

/// *Continue with Kontur* and *Back* (`Actions` `O1Q6WX`).
fn actions(ctx: &mut Ctx, team: &Team, chosen: Option<ProjectId>) -> Element {
  let navigation = Navigation::of(ctx);
  let server = team.entry().stored.id;
  let listed = team.projects().iter().find(|listed| Some(listed.project.id) == chosen);
  let go_on = listed.map(|listed| {
    ActionButton::new(
      "join-team-projects-continue",
      ButtonKind::Primary,
      ctx.t_args(
        "startup.shared.projects.continue",
        [("project", listed.project.name.as_str())],
      ),
    )
    .icon(Icon::ArrowRight)
    .large()
    .opens(navigation.to(next_place(server, listed)))
  });
  let back = ActionButton::new(
    "join-team-projects-back",
    ButtonKind::Ghost,
    ctx.t("startup.shared.back"),
  )
  .large()
  .roomy()
  .opens(navigation.to(Route::JoinTeam(JoinWay::Cloud)));
  Row::new()
    .id("join-team-projects-actions")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(go_on)
    .child(back)
    .into()
}

/// Where going on with `listed` leads: its checkout here when it has one, else *Link a folder*. A server names every
/// project by a slug; one without has no page of its own, so its server's page lists it.
fn next_place(server: ServerId, listed: &ServerProject) -> Route {
  if let Some(checkout) = &listed.checkout {
    return Route::Project(checkout.id, ProjectPage::Overview(OverviewTab::default()));
  }
  JoinTarget::new(server, &listed.project.slug).map_or_else(
    || Route::Orchester(OrchesterView::server(server)),
    |target| Route::JoinTeam(JoinWay::Folder(target)),
  )
}
