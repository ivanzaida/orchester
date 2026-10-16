//! *Projects on this computer* in a team server's entry (`Projects` `hYXPh` in `SFaAp`): the person's projects on the
//! server, each with its role, its address and the checkout linked to it here with *Open*, or *not on this
//! computer*. While the server cannot be read, the projects linked here still show.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::MemberRole;
use orchester_workspace::{
  display::shorten_path,
  sharing::{LinkedProject, ServerProject},
};

use super::{
  Said,
  parts::{hint, section, section_title},
  server::{Projects, Props},
  words::{counted, shown_address},
};
use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route},
  theme::{AccountsSpacing, ExtraSpacing, ExtraTypography, IconSize, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The glyph tile of a project row (`ORDuC`).
const PROJECT_TILE: f32 = 28.0;

/// The most characters of a checkout's path a project row shows before it cuts the middle.
const CHECKOUT_PATH: usize = 40;

/// The section, as the server answered.
pub(super) fn projects(ctx: &mut Ctx, props: &Props, projects: Option<&Result<Projects, Said>>) -> Element {
  let meta = counted(
    ctx,
    ("cloud.server.projects.linked.one", "cloud.server.projects.linked.other"),
    props.linked.len(),
  );
  let mut parts: Vec<Element> = Vec::new();
  match projects {
    None => parts.push(
      hint(
        "orchester-server-projects-reading",
        &ctx.t("cloud.server.projects.reading"),
      )
      .into(),
    ),
    Some(Err(reason)) => {
      // Offline or signed out: the projects linked here are still known.
      parts.push(hint("orchester-server-projects-unread", &reason.text(ctx)).into());
      if !props.linked.is_empty() {
        parts.push(list(ctx, props.linked.iter().map(Source::Linked).collect()));
      }
    }
    Some(Ok(projects)) if projects.0.is_empty() => {
      parts.push(hint("orchester-server-projects-none", &ctx.t("cloud.server.projects.none")).into());
    }
    Some(Ok(projects)) => parts.push(list(ctx, projects.0.iter().map(Source::Server).collect())),
  }
  parts.push(
    Text::new(&ctx.t("cloud.server.projects.explanation"))
      .id("orchester-server-projects-explanation")
      .variant(ExtraTypography::Prose.role())
      .color(PaletteColor::TextMuted)
      .width(Dimension::Pct(100.0))
      .into(),
  );
  section(
    "orchester-server-projects",
    section_title(
      "orchester-server-projects-title",
      &ctx.t("cloud.server.projects.title"),
      Some(&meta),
    ),
    parts,
  )
  .into()
}

/// A project row's source: the server's list, or only what this computer has linked.
enum Source<'a> {
  Server(&'a ServerProject),
  Linked(&'a LinkedProject),
}

/// What a row shows: the name, the role, the address and the checkout here.
struct Shown<'a> {
  name: String,
  role: Option<MemberRole>,
  address: String,
  checkout: Option<&'a LinkedProject>,
}

impl<'a> Source<'a> {
  fn shown(&self) -> Shown<'a> {
    match *self {
      Self::Server(project) => Shown {
        name: project.project.name.clone(),
        role: project.project.role,
        address: project.project.address.clone(),
        checkout: project.checkout.as_ref(),
      },
      Self::Linked(linked) => Shown {
        name: linked.name.as_str().to_owned(),
        role: linked.link.role,
        address: linked.link.address.clone().unwrap_or_default(),
        checkout: Some(linked),
      },
    }
  }
}

fn list(ctx: &mut Ctx, rows: Vec<Source<'_>>) -> Element {
  let navigation = Navigation::of(ctx);
  let mut card = Column::new()
    .id("orchester-server-projects-list")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceBase)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip();
  for row in rows {
    card = card.child(project_row(ctx, &row.shown(), &navigation));
  }
  card.into()
}

fn project_row(ctx: &Ctx, shown: &Shown<'_>, navigation: &Navigation) -> Element {
  let id = format!(
    "orchester-server-project-{}",
    shown.address.rsplit('/').next().unwrap_or_default()
  );
  let action = match shown.checkout {
    Some(checkout) => {
      let (navigation, project) = (navigation.clone(), checkout.id);
      ActionButton::new(
        format!("{id}-open"),
        ButtonKind::Ghost,
        ctx.t("cloud.server.projects.open"),
      )
      .icon(Icon::ArrowUpRight)
      .on_click(move || navigation.go(Route::Project(project, ProjectPage::Overview(OverviewTab::default()))))
    }
    // Cloning and linking from here is a later phase's; the button stays, unavailable with what to do instead.
    None => ActionButton::new(
      format!("{id}-link"),
      ButtonKind::Secondary,
      ctx.t("cloud.server.projects.link"),
    )
    .icon(Icon::FolderPlus)
    .unavailable(ctx.t("cloud.server.projects.linkLater")),
  };
  let glyph = if shown.checkout.is_some() {
    Icon::FolderGit2
  } else {
    Icon::Folder
  };
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      AccountsSpacing::Edge.role(),
      ExtraSpacing::Inline.role(),
    ))
    .spacing(AccountsSpacing::Cell.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(tile(glyph))
    .child(lines(ctx, &id, shown))
    .child(action)
    .into()
}

/// The name with the role, and the address with the checkout here.
fn lines(ctx: &Ctx, id: &str, shown: &Shown<'_>) -> Column {
  let role = shown.role.map(|role| match role {
    MemberRole::Owner => ctx.t("cloud.server.projects.owner"),
    MemberRole::Member => ctx.t("cloud.server.projects.member"),
  });
  let place = match shown.checkout {
    // A long checkout path keeps its start and its last folders, cut in the middle (`H:\…\work\kontur`).
    Some(checkout) => Text::new(&shorten_path(&checkout.repo_root, CHECKOUT_PATH))
      .id(format!("{id}-checkout"))
      .variant(OrchesterText::Address.role())
      .color(PaletteColor::TextMuted)
      .nowrap()
      .text_overflow(TextOverflow::Elipsis),
    None => Text::new(&ctx.t("cloud.server.projects.notHere"))
      .variant(ExtraTypography::Note.role())
      .color(PaletteColor::Warning)
      .nowrap(),
  };
  let top = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&shown.name)
        .id(format!("{id}-name"))
        .variant(ExtraTypography::Strong.role())
        .nowrap(),
    )
    .with_children(role.map(|role| Text::new(&role).variant(ExtraTypography::Note.role()).nowrap()));
  let line = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(shown_address(&shown.address))
        .variant(OrchesterText::Address.role())
        .nowrap(),
    )
    .child(Text::new(&ctx.t("cloud.factSeparator")).variant(ExtraTypography::Note.role()))
    .child(place);
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(AccountsSpacing::Lines.role())
    .child(top)
    .child(line)
}

fn tile(glyph: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(PROJECT_TILE))
    .height(Dimension::Px(PROJECT_TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextSecondary))
}
