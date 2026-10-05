//! A local project's Sharing (10.11, 10.17): the task list *Local only*, the repository's identity
//! with what the chosen server has for it, *Share on…* and what sharing sends; above them the offer
//! of the repository's `orchester.toml`, whose address is contacted only once the person added it in
//! Settings › Servers.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::IdempotencyKey, repository::RepositoryIdentity};

use super::{Actions, Facts, ServerChoice, SharingDialog, SharingTab};
use crate::{
  navigation::route::{Page, ServerTab},
  screens::project::settings::general::{readback, sized_setting_row},
  sharing::{
    Connection, ServerEntry,
    feed::{AddRequest, ReturnTo, ServerFeed, SignInRequest},
    project::{IdentityProblem, discovery::Discovery, lookup::Lookup},
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton, field::select_style, icon::Icon, notice::Notice, property::property,
    status_label::status_label,
  },
  workspace::OpenProject,
};

/// A setting's value column (`PkhHM`), the select and button heights (`ZGwlv`, `u1XBP`).
pub(super) const VALUE_WIDTH: f32 = 300.0;
pub(super) const CONTROL_HEIGHT: f32 = 36.0;
/// A step's numbered mark in the discovery offer (`L93ZA`).
const STEP_MARK: f32 = 20.0;

impl SharingTab {
  pub(super) fn local(&self, ctx: &mut Ctx, open: &OpenProject, facts: &Facts, actions: &Actions) -> Vec<Element> {
    let list = ServerFeed::of(ctx).list.get();
    let signed_in: Vec<ServerEntry> = list
      .entries()
      .iter()
      .filter(|entry| entry.in_use() && entry.connection != Some(Connection::SignedOut))
      .cloned()
      .collect();
    let chosen = self
      .server
      .get()
      .0
      .and_then(|id| signed_in.iter().find(|entry| entry.stored.id == id))
      .or_else(|| signed_in.first())
      .cloned();
    let offer = self.discovery_offer(ctx, facts, list.entries());
    let lookup = chosen.as_ref().and_then(|entry| self.lookup(ctx, entry, facts));
    let mut rows = Vec::new();
    rows.extend(offer.clone());
    rows.push(task_list_row(ctx, open, facts, offer.is_some()));
    rows.push(repository_row(ctx, facts, chosen.as_ref(), lookup.as_ref()));
    rows.push(self.share_row(
      ctx,
      facts,
      (&signed_in, chosen.as_ref()),
      (list.entries().is_empty(), actions),
    ));
    if offer.is_none() {
      rows.push(sends_panel(ctx));
    }
    rows
  }

  /// What the chosen server has for the repository; `None` while it is asked or cannot say.
  fn lookup(&self, ctx: &mut Ctx, entry: &ServerEntry, facts: &Facts) -> Option<Result<Lookup, ()>> {
    let identity = facts.identity.clone().ok()?;
    let servers = ServerFeed::of(ctx).servers;
    let deps = (entry.clone(), identity, self.revision.get());
    let load = ctx.future(
      deps,
      move |(entry, identity, _): (ServerEntry, RepositoryIdentity, u64)| {
        let servers = servers.clone();
        async move {
          servers.lookup(&entry, identity).await.map_err(|error| {
            tracing::warn!(%error, "cannot ask the server for the repository");
            error.message(&entry.address)
          })
        }
      },
    );
    let state = load.state().get();
    if state.is_pending() {
      return None;
    }
    match (state.data, state.error) {
      (_, Some(_)) => Some(Err(())),
      (Some(lookup), None) => Some(Ok(lookup)),
      (None, None) => None,
    }
  }

  /// The offer of the repository's `orchester.toml` (10.17), while its server is not one the person
  /// is signed in to and they did not set it aside.
  fn discovery_offer(&self, ctx: &mut Ctx, facts: &Facts, entries: &[ServerEntry]) -> Option<Element> {
    let Discovery::Server(address) = &facts.discovery.discovery else {
      return None;
    };
    let known = entries.iter().find(|entry| entry.address == *address);
    if self.dismissed.get()
      || known.is_some_and(|entry| entry.in_use() && entry.connection != Some(Connection::SignedOut))
    {
      return None;
    }
    let feed = ServerFeed::of(ctx);
    let (nav, address_text, known_id) = (
      self.nav.clone(),
      address.as_str().to_owned(),
      known.map(|entry| entry.stored.id),
    );
    let start = move || {
      // Signed in, the person comes back here to share.
      feed.return_to.set(ReturnTo(Some(Page::Settings(
        crate::navigation::route::SettingsTab::Sharing,
      ))));
      match known_id {
        // Added already: only signing in is left.
        Some(server) => {
          nav.show(Page::Servers(Some(server), ServerTab::Account));
          feed.sign_in.set(SignInRequest(Some(server)));
        }
        None => {
          nav.show(Page::Servers(None, ServerTab::Account));
          feed.add.set(AddRequest(Some(address_text.clone())));
        }
      }
    };
    let dismissed = self.dismissed.clone();
    let steps = offer_steps(ctx, facts, address.host());
    let buttons = Row::new()
      .padding_top(ExtraSpacing::Tight.role())
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new(ctx.t("sharing.discovery.start"))
          .primary()
          .icon(Icon::LogIn)
          .id("sharing-discovery-start")
          .in_form()
          .height(Dimension::Px(CONTROL_HEIGHT))
          .on_click(start)
          .render(ctx),
      )
      .child(
        ActionButton::new(ctx.t("sharing.discovery.notNow"))
          .id("sharing-discovery-dismiss")
          .in_form()
          .height(Dimension::Px(CONTROL_HEIGHT))
          .on_click(move || dismissed.set(true))
          .render(ctx),
      );
    Some(
      Notice::new(
        Icon::FileSymlink,
        ctx.t("sharing.discovery.heading"),
        ctx.t_args("sharing.discovery.detail", [("address", address.as_str())]),
      )
      .emphasized()
      .id("sharing-discovery-offer")
      .below(steps)
      .below(buttons)
      .into(),
    )
  }

  /// *Share on* (`O5Xr7P`): the signed-in servers and *Share on …*, or *Add server…* without one.
  fn share_row(
    &self,
    ctx: &mut Ctx,
    facts: &Facts,
    (signed_in, chosen): (&[ServerEntry], Option<&ServerEntry>),
    (no_servers, actions): (bool, &Actions),
  ) -> Element {
    let Some(chosen) = chosen else {
      let nav = self.nav.clone();
      let text = if no_servers {
        "sharing.shareOn.noServer"
      } else {
        "sharing.shareOn.notSignedIn"
      };
      return sized_setting_row(
        VALUE_WIDTH,
        &ctx.t("sharing.shareOn.name"),
        &ctx.t("sharing.shareOn.detailNoServer"),
        vec![
          Text::new(&ctx.t(text)).variant(TypographyStyle::FieldLabel).into(),
          ActionButton::new(ctx.t("sharing.shareOn.addServer"))
            .icon(Icon::Plus)
            .id("sharing-add-server")
            .in_form()
            .height(Dimension::Px(CONTROL_HEIGHT))
            .on_click(move || nav.show(Page::Servers(None, ServerTab::Account)))
            .render(ctx),
        ],
      );
    };
    let options = signed_in.iter().map(|entry| {
      let label = ctx.t_args(
        "sharing.shareOn.option",
        [("server", entry.stored.name.as_str()), ("host", entry.address.host())],
      );
      (ServerChoice(Some(entry.stored.id)), label)
    });
    if self.server.get_untracked().0 != Some(chosen.stored.id) {
      self.server.set(ServerChoice(Some(chosen.stored.id)));
    }
    let select = Element::from(
      Select::new(self.server.clone())
        .options(options)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(CONTROL_HEIGHT))
        .style(select_style(ctx, CONTROL_HEIGHT)),
    )
    .id("sharing-server-select");
    let pending = actions.share.state().get().is_pending();
    let (dialog, server) = (self.dialog.clone(), chosen.stored.id);
    let button = ActionButton::new(ctx.t_args("sharing.shareOn.action", [("server", chosen.stored.name.as_str())]))
      .icon(Icon::CloudUpload)
      .id("sharing-share")
      .in_form()
      .height(Dimension::Px(CONTROL_HEIGHT));
    let button = if facts.plan.is_some() && !pending {
      button.on_click(move || {
        dialog.set(SharingDialog::Share {
          server,
          key: IdempotencyKey::generate(),
        });
      })
    } else {
      button.on_click(|| {}).enabled(false)
    };
    sized_setting_row(
      VALUE_WIDTH,
      &ctx.t("sharing.shareOn.name"),
      &ctx.t("sharing.shareOn.detail"),
      vec![select, readback(&ctx.t("sharing.shareOn.readback")), button.render(ctx)],
    )
  }
}

/// *Task list* (`r6GRLR`): *Local only*, with the key and the count while the discovery offer shows.
fn task_list_row(ctx: &Ctx, open: &OpenProject, facts: &Facts, offer: bool) -> Element {
  let project = open.project.name().as_str();
  let key = open.project.key().as_str();
  let (description, back) = if offer {
    let count = if facts.tasks == 1 {
      "sharing.taskList.keyCount.one"
    } else {
      "sharing.taskList.keyCount.other"
    };
    (
      ctx.t_args("sharing.taskList.localKey", [("project", project), ("key", key)]),
      ctx.t_args(count, [("key", key), ("count", facts.tasks.to_string().as_str())]),
    )
  } else {
    (
      ctx.t_args("sharing.taskList.local", [("project", project)]),
      ctx.t("sharing.taskList.localReadback"),
    )
  };
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("sharing.taskList.name"),
    &description,
    vec![
      Row::new()
        .id("sharing-task-list")
        .child(status_label(Icon::HardDrive, &ctx.t("sharing.taskList.localLabel")))
        .into(),
      readback(&back),
    ],
  )
}

/// *Repository* (`jzqJw`): the identity a server knows the project by, and what the chosen server
/// has for it.
fn repository_row(
  ctx: &Ctx,
  facts: &Facts,
  chosen: Option<&ServerEntry>,
  lookup: Option<&Result<Lookup, ()>>,
) -> Element {
  let value: Element = match &facts.identity {
    Ok(identity) => Text::new(identity.as_str())
      .id("sharing-identity")
      .variant(ExtraTypography::MonoField.role())
      .into(),
    Err(problem) => Text::new(&ctx.t(problem_key(*problem)))
      .id("sharing-identity-problem")
      .variant(TypographyStyle::FieldLabel)
      .color(PaletteColor::TextPrimary)
      .into(),
  };
  let back = match (&facts.identity, chosen, lookup) {
    (Err(_), ..) => None,
    (Ok(_), None, _) => Some(ctx.t("sharing.repository.noServer")),
    (Ok(_), Some(entry), None) => {
      Some(ctx.t_args("sharing.repository.checking", [("server", entry.stored.name.as_str())]))
    }
    (Ok(_), Some(entry), Some(found)) => Some(lookup_text(ctx, entry, found)),
  };
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("sharing.repository.name"),
    &ctx.t("sharing.repository.detail"),
    std::iter::once(value).chain(back.map(|back| readback(&back))).collect(),
  )
}

fn lookup_text(ctx: &Ctx, entry: &ServerEntry, found: &Result<Lookup, ()>) -> Arc<str> {
  let server = entry.stored.name.as_str();
  match found {
    Ok(Lookup::NotShared) => ctx.t_args("sharing.repository.notShared", [("server", server)]),
    Ok(Lookup::Shared { key }) => ctx.t_args("sharing.repository.shared", [("server", server), ("key", key.as_str())]),
    Ok(Lookup::NotMember { .. }) => ctx.t_args("sharing.repository.notMember", [("server", server)]),
    Err(()) => ctx.t_args("sharing.repository.unreachable", [("server", server)]),
  }
}

const fn problem_key(problem: IdentityProblem) -> &'static str {
  match problem {
    IdentityProblem::NoOrigin => "sharing.repository.noOrigin",
    IdentityProblem::LocalOrigin => "sharing.repository.localOrigin",
    IdentityProblem::Unreadable => "sharing.repository.unreadable",
  }
}

/// *What sharing sends* (`boHko`).
fn sends_panel(ctx: &Ctx) -> Element {
  Column::new()
    .id("sharing-sends")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(Text::new(&ctx.t("sharing.sends.title")).variant(ExtraTypography::NoticeHeading.role()))
    .child(property(
      &ctx.t("sharing.sends.shared"),
      &ctx.t("sharing.sends.sharedValue"),
    ))
    .child(property(
      &ctx.t("sharing.sends.local"),
      &ctx.t("sharing.sends.localValue"),
    ))
    .child(
      Text::new(&ctx.t("sharing.sends.note"))
        .variant(ExtraTypography::Detail.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// The offer's three steps: add the server, sign in, review and join.
fn offer_steps(ctx: &Ctx, facts: &Facts, host: &str) -> Column {
  let count = if facts.tasks == 1 {
    "sharing.discovery.step3.one"
  } else {
    "sharing.discovery.step3.other"
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Sm)
    .spacing(ExtraSpacing::Stack.role())
    .child(step(1, &ctx.t_args("sharing.discovery.step1", [("host", host)]), true))
    .child(step(2, &ctx.t("sharing.discovery.step2"), false))
    .child(step(
      3,
      &ctx.t_args(count, [("count", facts.tasks.to_string().as_str())]),
      false,
    ))
}

/// A numbered step of the discovery offer; the one to take next in the text color.
fn step(number: u32, text: &str, next: bool) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .width(Dimension::Px(STEP_MARK))
        .height(Dimension::Px(STEP_MARK))
        .align_items(Alignment::Center)
        .justify(lurq::layout::layout_kind::Justify::Center)
        .rounded(ExtraRadius::Round.role())
        .border_inside(lurq::app::theme::BorderSize::Sm, PaletteColor::Border)
        .child(Text::new(&number.to_string()).variant(TypographyStyle::Mono)),
    )
    .child(
      Text::new(text)
        .variant(ExtraTypography::Criterion.role())
        .color(if next {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        })
        .flex(1.0),
    )
    .into()
}
