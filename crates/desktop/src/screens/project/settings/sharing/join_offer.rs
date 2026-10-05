//! A local project whose repository is shared on a server already (`docs/architecture/server.md`,
//! "Finding the project"; 10.15, 10.16 A–C): the servers the person is signed in to are asked for
//! the repository's identity, and only those. Where one has it, Sharing offers *Review and join…*
//! instead of Share, or, for a person who is not a member, names the owners, the username to give
//! them and *Check again*; nothing is uploaded. A repository that cannot be shared says why.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ServerId, repository::RepositoryIdentity, time::Timestamp};

use super::{Facts, SharingTab, labels, local::CONTROL_HEIGHT};
use crate::{
  navigation::route::Page,
  sharing::{
    ServerEntry,
    feed::ServerFeed,
    project::{IdentityProblem, lookup::Lookup},
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, icon::Icon, notice::Notice, property::property},
  workspace::OpenProject,
};

/// What the signed-in servers have for the repository, as last asked, and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Lookups {
  pub at: Timestamp,
  pub found: Vec<(ServerId, Result<Lookup, ()>)>,
}

impl lurq::app::component::DevtoolsInspectable for Lookups {}

impl Lookups {
  pub fn of(&self, server: ServerId) -> Option<&Result<Lookup, ()>> {
    self.found.iter().find(|(id, _)| *id == server).map(|(_, found)| found)
  }

  /// The first server that has the repository as a project already.
  pub fn existing(&self) -> Option<(ServerId, &Lookup)> {
    self.found.iter().find_map(|(server, found)| match found {
      Ok(lookup @ (Lookup::Shared { .. } | Lookup::NotMember { .. })) => Some((*server, lookup)),
      _ => None,
    })
  }
}

impl SharingTab {
  /// Asks every server in `signed_in` for the repository's project; `None` while they are asked,
  /// or when the repository cannot be shared.
  pub(super) fn lookups(&self, ctx: &mut Ctx, signed_in: &[ServerEntry], facts: &Facts) -> Option<Lookups> {
    let identity = facts.identity.clone().ok()?;
    let servers = ServerFeed::of(ctx).servers;
    // By the servers' ids: an entry changes with every answer of its server (when it was last
    // reached), which must not ask again. A sign-in changes the ids, *Check again* the revision.
    let ids: Vec<ServerId> = signed_in.iter().map(|entry| entry.stored.id).collect();
    let deps = (ids, identity, self.revision.get());
    let load = ctx.future(
      deps,
      move |(ids, identity, _): (Vec<ServerId>, RepositoryIdentity, u64)| {
        let servers = servers.clone();
        async move {
          let read = servers.clone();
          let entries: Vec<ServerEntry> = crate::background::run(move || read.entries())
            .await
            .map_err(drop)?
            .map_err(|error| tracing::warn!(%error, "cannot read the servers"))?
            .into_iter()
            .filter(|entry| ids.contains(&entry.stored.id))
            .collect();
          let mut found = Vec::with_capacity(entries.len());
          for entry in entries {
            let answer = servers.lookup(&entry, identity.clone()).await.map_err(|error| {
              tracing::warn!(%error, "cannot ask the server for the repository");
            });
            found.push((entry.stored.id, answer));
          }
          Ok::<_, ()>(Lookups {
            at: Timestamp::now(),
            found,
          })
        }
      },
    );
    let state = load.state().get();
    let pending = state.is_pending();
    state.data.filter(|_| !pending)
  }

  /// The notice for a repository a server has already (10.15, 10.16 A, 10.16 C).
  pub(super) fn existing_notice(
    &self,
    ctx: &Ctx,
    (entry, lookup, at): (&ServerEntry, &Lookup, Timestamp),
    facts: &Facts,
  ) -> Option<Element> {
    match lookup {
      Lookup::NotShared => None,
      Lookup::Shared { key } => Some(self.join_notice(ctx, (entry, key), facts)),
      Lookup::NotMember { owners } => Some(self.not_member_notice(ctx, (entry, owners), at)),
    }
  }

  /// *This repository is shared on Kontur team as ORC* with *Review and join…* (10.15); right
  /// after Share met the project, *Kontur team already has this repository as ORC* (10.16 C).
  fn join_notice(&self, ctx: &Ctx, (entry, key): (&ServerEntry, &str), facts: &Facts) -> Element {
    let server = entry.stored.name.as_str();
    let taken = self.taken.get().0.filter(|(id, _)| *id == entry.stored.id);
    let (heading, detail) = match &taken {
      Some((_, owners)) => (
        ctx.t_args("sharing.join.takenHeading", [("server", server), ("key", key)]),
        ctx.t_args(
          "sharing.join.takenDetail",
          [("owners", &*labels::keys_or(ctx, owners, "sharing.join.someone"))],
        ),
      ),
      None => (
        ctx.t_args("sharing.join.offerHeading", [("server", server), ("key", key)]),
        offer_detail(ctx, facts.tasks, key),
      ),
    };
    let nav = self.nav.clone();
    let id = entry.stored.id;
    let mut buttons = Row::new()
      .padding_top(ExtraSpacing::Tight.role())
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new(ctx.t("sharing.join.review"))
          .icon(Icon::Users)
          .id("sharing-review-join")
          .in_form()
          .height(Dimension::Px(CONTROL_HEIGHT))
          .on_click(move || nav.show(Page::Join(id)))
          .render(ctx),
      );
    if taken.is_some() {
      let taken = self.taken.clone();
      buttons = buttons.child(
        ActionButton::new(ctx.t("sharing.discovery.notNow"))
          .id("sharing-taken-dismiss")
          .in_form()
          .height(Dimension::Px(CONTROL_HEIGHT))
          .on_click(move || taken.set(super::Taken(None)))
          .render(ctx),
      );
    }
    Notice::new(Icon::Cloud, heading, detail)
      .emphasized()
      .id("sharing-join-offer")
      .below(buttons)
      .into()
  }

  /// *Kontur team has this repository, but you are not a member* (10.16 A): the owners, the
  /// person's username with Copy, and *Check again* with when it last asked. The server names no
  /// key to someone who is not a member.
  fn not_member_notice(&self, ctx: &Ctx, (entry, owners): (&ServerEntry, &[String]), at: Timestamp) -> Element {
    let server = entry.stored.name.as_str();
    let username = entry
      .stored
      .account
      .as_ref()
      .map(|account| account.username.clone())
      .unwrap_or_default();
    let copied = self.copied.clone();
    let copy_text = username.clone();
    let copy = ActionButton::new(if copied.get() {
      ctx.t("common.copied")
    } else {
      ctx.t("common.copy")
    })
    .icon(Icon::Copy)
    .id("sharing-copy-username")
    .in_form()
    .on_click(move || {
      let success = lurq::clipboard::copy_to_clipboard(&copy_text);
      if !success {
        tracing::warn!("the clipboard did not take the username");
      }
      copied.set(success);
    })
    .render(ctx);
    let revision = self.revision.clone();
    let check = ActionButton::new(ctx.t("sharing.join.checkAgain"))
      .icon(Icon::RefreshCw)
      .id("sharing-check-again")
      .in_form()
      .height(Dimension::Px(CONTROL_HEIGHT))
      .on_click(move || revision.update(|value| *value += 1))
      .render(ctx);
    let owners = labels::keys_or(ctx, owners, "sharing.join.someone");
    Notice::new(
      Icon::Lock,
      ctx.t_args("sharing.join.notMemberHeading", [("server", server)]),
      ctx.t("sharing.join.notMemberDetail"),
    )
    .emphasized()
    .id("sharing-not-member")
    .below(property(&ctx.t("sharing.join.owners"), &owners))
    .below(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("sharing.join.yourUsername")).variant(TypographyStyle::Caption))
        .child(
          Row::new()
            .padding_horizontal(SpacingSize::Sm)
            .padding_vertical(ExtraSpacing::Tight.role())
            .background(PaletteColor::SurfaceBase)
            .rounded(ExtraRadius::Notice.role())
            .child(
              Text::new(&username)
                .id("sharing-username")
                .variant(ExtraTypography::MonoField.role())
                .color(PaletteColor::TextPrimary),
            ),
        )
        .child(copy),
    )
    .below(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(check)
        .child(
          Text::new(&ctx.t_args("sharing.join.checked", [("time", labels::time(at).as_str())]))
            .variant(ExtraTypography::Footnote.role())
            .color(PaletteColor::TextMuted),
        ),
    )
    .into()
  }
}

/// What joining does, before the review (`k8dBok`).
fn offer_detail(ctx: &Ctx, tasks: usize, key: &str) -> Arc<str> {
  let count = tasks.to_string();
  match tasks {
    0 => ctx.t_args("sharing.join.offerDetailNone", [("key", key)]),
    1 => ctx.t_args("sharing.join.offerDetail.one", [("key", key)]),
    _ => ctx.t_args(
      "sharing.join.offerDetail.other",
      [("key", key), ("count", count.as_str())],
    ),
  }
}

/// Why the repository cannot be shared (10.16 B).
pub(super) fn identity_notice(ctx: &Ctx, open: &OpenProject, problem: &IdentityProblem) -> Element {
  let project = open.project.name().as_str();
  let (heading, detail) = match problem {
    IdentityProblem::LocalOrigin { origin: Some(origin) } => (
      "sharing.cantShare.localOrigin",
      ctx.t_args("sharing.cantShare.localOriginPath", [("origin", origin.as_str())]),
    ),
    IdentityProblem::LocalOrigin { origin: None } => (
      "sharing.cantShare.localOrigin",
      ctx.t("sharing.cantShare.localOriginDetail"),
    ),
    IdentityProblem::NoOrigin => ("sharing.cantShare.noOrigin", ctx.t("sharing.cantShare.noOriginDetail")),
    IdentityProblem::Unreadable => (
      "sharing.cantShare.unreadable",
      ctx.t("sharing.cantShare.unreadableDetail"),
    ),
  };
  Notice::new(Icon::GitBranch, ctx.t_args(heading, [("project", project)]), detail)
    .emphasized()
    .id("sharing-cant-share")
    .into()
}
