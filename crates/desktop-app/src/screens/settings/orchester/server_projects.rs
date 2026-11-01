//! *Projects on this computer* on a team server's page (`Projects on this computer` `SkLi4` in `C6qnY`): the person's
//! projects on the server, a row each with its role and the checkout linked to it here and *Open*, or *not on this
//! computer* and *Link a checkout…*. While the server cannot be read, the projects linked here still show.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_api::MemberRole;
use orchester_core::id::ServerId;
use orchester_workspace::{
  display::shorten_path,
  sharing::{LinkedProject, ServerProject},
};

use super::{
  Said,
  parts::hint,
  server::{Projects, Props},
  words::{counted, shown_address},
};
use crate::{
  navigation::Navigation,
  routes::{JoinTarget, OverviewTab, ProjectPage, Route},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    setting_rows::{SettingGroup, SettingRow, group_note},
  },
};

/// The most characters of a checkout's path a project row shows before it cuts the middle.
const CHECKOUT_PATH: usize = 40;

/// The group, as the server answered.
pub(super) fn projects(ctx: &mut Ctx, props: &Props, projects: Option<&Result<Projects, Said>>) -> Element {
  let meta = counted(
    ctx,
    ("cloud.server.projects.linked.one", "cloud.server.projects.linked.other"),
    props.linked.len(),
  );
  let mut group = SettingGroup::new("orchester-server-projects", ctx.t("cloud.server.projects.title")).meta(meta);
  let mut lines: Vec<Element> = Vec::new();
  match projects {
    None => lines.push(
      hint(
        "orchester-server-projects-reading",
        &ctx.t("cloud.server.projects.reading"),
      )
      .into(),
    ),
    Some(Err(reason)) => {
      // Offline or signed out: the projects linked here are still known.
      lines.push(hint("orchester-server-projects-unread", &reason.text(ctx)).into());
      group = group.rows(rows(ctx, props, props.linked.iter().map(Source::Linked).collect()));
    }
    Some(Ok(projects)) if projects.0.is_empty() => {
      lines.push(hint("orchester-server-projects-none", &ctx.t("cloud.server.projects.none")).into());
    }
    Some(Ok(projects)) => group = group.rows(rows(ctx, props, projects.0.iter().map(Source::Server).collect())),
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(group)
    .with_children(lines)
    .child(group_note(
      "orchester-server-projects-explanation",
      &ctx.t("cloud.server.projects.explanation"),
    ))
    .into()
}

/// A project row's source: the server's list, or only what this computer has linked.
enum Source<'a> {
  Server(&'a ServerProject),
  Linked(&'a LinkedProject),
}

/// What a row shows: the name, the role, the address and its slug, and the checkout here.
struct Shown<'a> {
  name: String,
  role: Option<MemberRole>,
  address: String,
  slug: String,
  checkout: Option<&'a LinkedProject>,
}

impl<'a> Source<'a> {
  fn shown(&self) -> Shown<'a> {
    match *self {
      Self::Server(project) => Shown {
        name: project.project.name.clone(),
        role: project.project.role,
        address: project.project.address.clone(),
        slug: project.project.slug.clone(),
        checkout: project.checkout.as_ref(),
      },
      Self::Linked(linked) => {
        let address = linked.link.address.clone().unwrap_or_default();
        Shown {
          name: linked.name.as_str().to_owned(),
          role: linked.link.role,
          slug: address.rsplit('/').next().unwrap_or_default().to_owned(),
          address,
          checkout: Some(linked),
        }
      }
    }
  }
}

fn rows(ctx: &mut Ctx, props: &Props, sources: Vec<Source<'_>>) -> Vec<Element> {
  let navigation = Navigation::of(ctx);
  sources
    .into_iter()
    .map(|source| project_row(ctx, &source.shown(), (&navigation, props.entry.stored.id)))
    .collect()
}

/// A project (`kontur` `GMSQV`): its name, its role and the checkout here, and *Open* or *Link a checkout…*; its
/// address in the tooltip.
fn project_row(ctx: &mut Ctx, shown: &Shown<'_>, (navigation, server): (&Navigation, ServerId)) -> Element {
  let id = format!("orchester-server-project-{}", shown.slug);
  let action = match shown.checkout {
    Some(checkout) => ActionButton::new(
      format!("{id}-open"),
      ButtonKind::Ghost,
      ctx.t("cloud.server.projects.open"),
    )
    .icon(Icon::ArrowUpRight)
    .opens(navigation.to(Route::Project(
      checkout.id,
      ProjectPage::Overview(OverviewTab::default()),
    ))),
    // Which checkout to link, or the `git clone` to run first (`xeFRB`); nothing is sent before the join review.
    None => {
      let button = ActionButton::new(
        format!("{id}-link"),
        ButtonKind::Ghost,
        ctx.t("cloud.server.projects.link"),
      )
      .icon(Icon::FolderPlus);
      match JoinTarget::new(server, &shown.slug) {
        Some(target) => button.opens(navigation.to(Route::Invite(target))),
        None => button.disabled(),
      }
    }
  };
  let role = shown.role.map(|role| match role {
    MemberRole::Owner => ctx.t("cloud.server.projects.owner"),
    MemberRole::Member => ctx.t("cloud.server.projects.member"),
  });
  // A long checkout path keeps its start and its last folders, cut in the middle (`H:\…\work\kontur`).
  let line = match (shown.checkout, role) {
    (Some(checkout), Some(role)) => ctx.t_args(
      "cloud.server.projects.linkedRole",
      [
        ("role", role.to_string()),
        ("path", shorten_path(&checkout.repo_root, CHECKOUT_PATH)),
      ],
    ),
    (Some(checkout), None) => ctx.t_args(
      "cloud.server.projects.linkedTo",
      [("path", shorten_path(&checkout.repo_root, CHECKOUT_PATH))],
    ),
    (None, Some(role)) => ctx.t_args("cloud.server.projects.notHereRole", [("role", role)]),
    (None, None) => ctx.t("cloud.server.projects.notHere"),
  };
  let row = SettingRow::new(format!("{id}-entry"), shown.name.clone())
    .hint(line.to_string())
    .info(shown_address(&shown.address).to_owned())
    .control(action)
    .render(ctx);
  Column::new().id(id).width(Dimension::Pct(100.0)).child(row).into()
}
