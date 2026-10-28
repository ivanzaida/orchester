//! The landing's body (`xeFRB`, `d8oDsO`): what the invite was to (project address, server, repository hint, role),
//! then *Checkout on this computer*: each checkout that could hold the project, picked with its radio, with how its
//! `origin` compares with the hint and how many local tasks it has; one linked to another project can't be picked and
//! says why. Without one, the `git clone` to run first, with *Copy*. *Add a checkout folder…* adds a folder Orchester
//! doesn't know yet; one that isn't a git checkout is refused.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_api::MemberRole;
use orchester_workspace::{
  display::shorten_path,
  sharing::{
    RemoteMatch,
    project::landing::{Checkout, Landing, Linked},
  },
};

use super::{Added, Picked, Said, adding};
use crate::{
  screens::settings::orchester::shown_address,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchesterText},
  ui::{
    chip::{BadgeTone, badge},
    code_box::CodeBox,
    detail_row::{DetailValue, detail_row},
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};

/// The most characters of a checkout's path a row shows before it cuts the middle.
const PATH: usize = 48;

/// What the header says under the title: who this computer is on the server, and what to do here.
pub(super) fn lead(ctx: &Ctx, landing: &Landing) -> Option<Arc<str>> {
  let username = landing.entry.stored.account.as_ref()?.username.clone();
  Some(ctx.t_args(
    "sharing.landing.lead",
    [
      ("server", landing.entry.stored.name.clone()),
      ("username", username),
      ("project", landing.project.name.clone()),
    ],
  ))
}

/// The facts of the project, then its checkouts on this computer, and under them *Add a checkout folder…* with the
/// page's `actions` at its end.
pub(super) fn body(
  ctx: &mut Ctx,
  landing: &Landing,
  (picked, add): (&Signal<Picked>, &FutureAction<Arc<str>, Added, Said>),
  actions: [Element; 2],
) -> Element {
  let added = match add.state().get().data {
    Some(Added::Project(project)) => Some(project),
    _ => None,
  };
  let mut shown: Vec<&Checkout> = landing.candidates().collect();
  // A folder just added shows whatever its `origin`, so the person sees why it can or can't be picked.
  let extra = landing
    .checkouts
    .iter()
    .find(|checkout| Some(checkout.id) == added && !shown.iter().any(|one| one.id == checkout.id));
  shown.extend(extra);
  let chosen = picked.get().0.or_else(|| first_match(landing));
  let mut list = Column::new()
    .id("orchester-landing-checkouts")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm);
  let shown_choosable = shown.iter().any(|checkout| checkout.choosable());
  if !shown_choosable {
    list = list.child(no_checkout(ctx, landing));
  }
  for checkout in shown {
    let added = Some(checkout.id) == added;
    list = list.child(checkout_row(
      ctx,
      landing,
      checkout,
      (chosen == Some(checkout.id) && checkout.choosable(), added),
      picked,
    ));
  }
  let none = !shown_choosable;
  list = list
    .with_children(adding::refused(ctx, landing, add))
    .child(adding::add_row(ctx, landing, (add, none), actions));
  Column::new()
    .id("orchester-landing")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(facts(ctx, landing))
    .child(
      Text::new(&ctx.t("sharing.landing.checkouts"))
        .id("orchester-landing-checkouts-title")
        .variant(ExtraTypography::FormSection.role()),
    )
    .child(list)
    .into()
}

/// The checkout picked until the person picks one: the first that matches the project's hint (`xeFRB`).
pub(super) fn first_match(landing: &Landing) -> Option<orchester_core::id::ProjectId> {
  landing
    .candidates()
    .find(|checkout| checkout.choosable() && checkout.remote == RemoteMatch::Same)
    .map(|checkout| checkout.id)
}

/// Project (by its name, `w2O9xk` in `q7dWu`), Server, Repository hint and Your role.
fn facts(ctx: &Ctx, landing: &Landing) -> Element {
  let project = &landing.project;
  let server = ctx.t_args(
    "sharing.landing.serverValue",
    [("server", landing.entry.stored.name.as_str())],
  );
  let hint = project
    .remote_hint
    .as_ref()
    .map_or_else(|| ctx.t("sharing.landing.noHint"), |hint| Arc::from(hint.as_str()));
  let role = match project.role {
    Some(MemberRole::Owner) => ctx.t("sharing.landing.owner"),
    Some(MemberRole::Member) | None => ctx.t("sharing.landing.member"),
  };
  Column::new()
    .id("orchester-landing-facts")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Inset.role())
    .spacing(SpacingSize::Xs)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(detail_row(
      "orchester-landing-project",
      &ctx.t("sharing.landing.project"),
      &project.name,
      DetailValue::Text,
    ))
    .child(detail_row(
      "orchester-landing-server",
      &ctx.t("sharing.landing.server"),
      &server,
      DetailValue::Text,
    ))
    .child(detail_row(
      "orchester-landing-hint",
      &ctx.t("sharing.landing.hint"),
      &hint,
      DetailValue::Text,
    ))
    .child(detail_row(
      "orchester-landing-role",
      &ctx.t("sharing.landing.role"),
      &role,
      DetailValue::Text,
    ))
    .into()
}

/// A checkout: its radio, name and path, how its `origin` compares, and what the review will show.
fn checkout_row(
  ctx: &Ctx,
  landing: &Landing,
  checkout: &Checkout,
  (chosen, added): (bool, bool),
  picked: &Signal<Picked>,
) -> Element {
  let id = format!("orchester-landing-checkout-{}", checkout.id);
  let mut radio = Toggle::new(
    format!("{id}-pick"),
    ToggleKind::Radio,
    chosen,
    ctx.t_args("sharing.landing.pick", [("project", checkout.name.as_str())]),
  )
  .hide_label();
  if checkout.choosable() {
    let (picked, project) = (picked.clone(), checkout.id);
    radio = radio.on_change(move |_| picked.set(Picked(Some(project))));
  }
  let lines = lines(ctx, (landing, checkout), (&id, added));
  let card = Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Inset.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .background(if chosen {
      PaletteColor::SurfaceRaised
    } else {
      PaletteColor::SurfaceBase
    })
    .border(Border::inside(
      BorderSize::Sm,
      if chosen {
        PaletteColor::BorderStrong
      } else {
        PaletteColor::Border
      },
    ))
    .rounded(ExtraRadius::Tile.role())
    .child(radio)
    .child(lines);
  if checkout.choosable() {
    card.into()
  } else {
    card
      .opacity(crate::theme::DISABLED_OPACITY)
      .describe("state", "unavailable")
      .into()
  }
}

/// The checkout's name and path, *Added* after a folder was added, how its `origin` compares, and what is next.
fn lines(ctx: &Ctx, (landing, checkout): (&Landing, &Checkout), (id, added): (&str, bool)) -> Column {
  let top = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(checkout.name.as_str())
        .id(format!("{id}-name"))
        .variant(ExtraTypography::Strong.role())
        .nowrap(),
    )
    .child(
      Text::new(&shorten_path(&checkout.repo_root, PATH))
        .variant(OrchesterText::Address.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    // One folder is added at a time.
    .with_children(added.then(|| badge("orchester-landing-added", BadgeTone::Info, &ctx.t("sharing.landing.added"))));
  let (glyph, color, line) = compare(ctx, landing, checkout);
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(top)
    .child(
      Row::new()
        .id(format!("{id}-origin"))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Sm, color.clone()))
        .child(
          Text::new(&line)
            .variant(ExtraTypography::Note.role())
            .color(color)
            .flex(1.0),
        ),
    )
    .child(
      Text::new(&what_next(ctx, landing, checkout))
        .id(format!("{id}-next"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
}

/// How the checkout's `origin` compares with the project's hint, or what it is linked to.
fn compare(ctx: &Ctx, landing: &Landing, checkout: &Checkout) -> (Icon, PaletteColor, Arc<str>) {
  if let Some(Linked::Other(address)) = &checkout.linked {
    let address = address.as_deref().map_or_else(
      || ctx.t("sharing.landing.anotherProject"),
      |address| Arc::from(shown_address(address)),
    );
    return (
      Icon::Link,
      PaletteColor::TextMuted,
      ctx.t_args("sharing.landing.linkedElsewhere", [("address", address)]),
    );
  }
  let hint = landing
    .project
    .remote_hint
    .as_ref()
    .map(|hint| hint.as_str().to_owned())
    .unwrap_or_default();
  match &checkout.remote {
    RemoteMatch::Same => (
      Icon::CircleCheck,
      PaletteColor::Success,
      ctx.t_args("sharing.landing.matches", [("origin", hint)]),
    ),
    RemoteMatch::Different { checkout: origin, hint } => (
      Icon::TriangleAlert,
      PaletteColor::Warning,
      ctx.t_args(
        "sharing.landing.differs",
        [("origin", origin.as_str()), ("hint", hint.as_str())],
      ),
    ),
    RemoteMatch::NoOrigin => (
      Icon::CircleDashed,
      PaletteColor::TextMuted,
      ctx.t("sharing.landing.noOrigin"),
    ),
    RemoteMatch::NoHint => (
      Icon::CircleDashed,
      PaletteColor::TextMuted,
      ctx.t("sharing.landing.nothingToCompare"),
    ),
  }
}

/// What picking the checkout leads to.
fn what_next(ctx: &Ctx, landing: &Landing, checkout: &Checkout) -> Arc<str> {
  match (&checkout.linked, &checkout.remote, checkout.tasks.as_slice()) {
    (Some(Linked::Other(_)), ..) => ctx.t_args("sharing.landing.linkedNote", [("project", checkout.name.as_str())]),
    (_, RemoteMatch::Different { .. }, _) => ctx.t("sharing.landing.differsNote"),
    (_, _, []) => ctx.t_args("sharing.landing.noTasks", [("project", landing.project.name.as_str())]),
    (_, _, [one]) => ctx.t_args("sharing.landing.oneTask", [("key", one.to_string())]),
    (_, _, [first, .., last]) => ctx.t_args(
      "sharing.landing.tasks",
      [
        ("count", checkout.tasks.len().to_string()),
        ("first", first.to_string()),
        ("last", last.to_string()),
      ],
    ),
  }
}

/// No local project here has the project's repository: clone it first, then add the folder (`No checkout` in
/// `d8oDsO`).
fn no_checkout(ctx: &mut Ctx, landing: &Landing) -> Element {
  let hint = landing
    .project
    .remote_hint
    .as_ref()
    .map(|hint| hint.as_str().to_owned());
  let title = match &hint {
    Some(hint) => ctx.t_args("sharing.landing.noCheckout", [("hint", hint.as_str())]),
    None => ctx.t("sharing.landing.noCheckoutAny"),
  };
  let command = landing
    .clone_command()
    .map(|command| CodeBox::new("orchester-landing-clone", ctx.t("sharing.landing.cloneLabel"), command).render(ctx));
  Column::new()
    .id("orchester-landing-none")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Inset.role())
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Start)
        .child(icon(Icon::FolderSearch, IconSize::Md, PaletteColor::TextSecondary))
        .child(
          Column::new()
            .flex(1.0)
            .spacing(ExtraSpacing::Fine.role())
            .child(
              Text::new(&title)
                .id("orchester-landing-none-title")
                .variant(ExtraTypography::Strong.role())
                .width(Dimension::Pct(100.0)),
            )
            .child(
              Text::new(&ctx.t("sharing.landing.noCheckoutBody"))
                .variant(TypographyStyle::Description)
                .width(Dimension::Pct(100.0)),
            ),
        ),
    )
    .with_children(command)
    .into()
}
