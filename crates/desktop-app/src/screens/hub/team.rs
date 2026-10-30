//! The team's configuration in a shared project's five libraries and their editors (`Shared configuration · redesign`
//! `UC5xk`, `docs/design/shared-configuration.md`): the line saying the library is the team's, or that only admins
//! change it (01 `cHvkL`, 01b `P8tXS`, 03 `pjEjf`); each row's publisher and state (`Publisher` `RztmV`, `Status col`
//! `oIW8j`); an editor's head line with its version, publisher and state (`Sub` `Du8OB` in 02 `Sflw7`), and a member's
//! *Versions* (`EGuXw` in 03, [`super::versions`]); the draft bar's words and *Publish for the team* (02); and, in the
//! draft bar's place, a version published here that was not sent (02b `iLwm9`) or met one someone published meanwhile
//! (02c `JZFlA`, [`super::waiting_bar`]). A local project shows none of it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ResourceId, resource::VersionNumber, time::Timestamp};
use orchester_store::{PublishState, SharedStanding};
use orchester_workspace::{agents::data::VersionState, display, i18n::NAMESPACE, sharing::team::TeamConfiguration};

use super::{
  parts::{Tone, note, status},
  versions::{VersionsProps, versions_button},
};
use crate::{
  project::ShownProject,
  services::Services,
  theme::{ExtraSpacing, HubText, IconSize},
  ui::{
    avatar::{initials, person_avatar},
    icon::{Icon, icon},
  },
};

/// A row's publisher, its avatar and who and when (`Publisher` `RztmV`).
const PUBLISHER_WIDTH: f32 = 150.0;
/// A row's state at its end (`Status col` `oIW8j`).
const STATE_WIDTH: f32 = 160.0;

/// The draft a resource has on this computer: `Some(base)` with one, edited from version `base` (`None` before its
/// first version); `None` without.
pub(crate) type DraftOn = Option<Option<VersionNumber>>;

/// The draft a library entry in `state` has.
pub(crate) const fn draft_on(state: VersionState) -> DraftOn {
  match state {
    VersionState::Draft { published } => Some(published),
    VersionState::StaleDraft { based_on, .. } => Some(based_on),
    VersionState::Published(_) => None,
  }
}

/// Where one of the team's resources stands, as its row and its editor's head say it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Said {
  Version(VersionNumber),
  NotPublished,
  Draft(Option<VersionNumber>),
  NotSent,
  Conflict(VersionNumber),
  Refused,
}

impl Said {
  /// A version published here comes first: a later draft waits behind it.
  fn of(standing: &SharedStanding, draft: DraftOn) -> Self {
    match standing.waiting.first().map(|waiting| &waiting.state) {
      Some(PublishState::Waiting) => return Self::NotSent,
      Some(PublishState::Conflict { latest }) => return Self::Conflict(*latest),
      Some(PublishState::Refused { .. }) => return Self::Refused,
      None => {}
    }
    match (draft, &standing.confirmed) {
      (Some(base), _) => Self::Draft(base),
      (None, Some(confirmed)) => Self::Version(confirmed.number),
      (None, None) => Self::NotPublished,
    }
  }

  const fn tone(self) -> Tone {
    match self {
      Self::Version(_) => Tone::Success,
      Self::NotPublished | Self::Draft(None) => Tone::Muted,
      Self::Draft(Some(_)) => Tone::Warning,
      Self::NotSent => Tone::Info,
      Self::Conflict(_) | Self::Refused => Tone::Danger,
    }
  }
}

/// The team's configuration of `project` while it is shared; `None` while it is local or not read yet.
pub(crate) fn of(project: &ShownProject) -> Option<TeamConfiguration> {
  project.feed.team.get()
}

/// Whether the person may change `project`'s resources: always in a local project, an admin's in a shared one.
pub(crate) fn may_change(team: Option<&TeamConfiguration>) -> bool {
  team.is_none_or(|team| team.may_change)
}

/// *Versions* in a member's editor head (`Versions` `EGuXw` in 03 `pjEjf`), with the element id `<id>-versions`; `None`
/// for an admin, whose editor has the draft bar, and for a resource that is not the team's.
pub(crate) fn member_versions(
  ctx: &mut Ctx,
  id: &str,
  (project, team): (&ShownProject, Option<&TeamConfiguration>),
  resource: ResourceId,
) -> Option<Element> {
  let standing = team.filter(|team| !team.may_change)?.standing(resource)?;
  Some(versions_button(
    ctx,
    VersionsProps {
      id: id.into(),
      root: project.open.repo_root.clone(),
      resource: (standing.kind, resource),
    },
  ))
}

/// The line over a library or a member's editor (`Shared line` `t3ABe9` in 01, `Read-only line` `z8Triu` in 03): an
/// admin's says what publishing does, a member's who may change it.
pub(crate) fn line(ctx: &Ctx, id: &str, team: &TeamConfiguration) -> Row {
  let (glyph, text) = if team.may_change {
    let text = match &team.server {
      Some((name, _)) => ctx.t_args("sharing.config.shared", [("team", name.to_string())]),
      None => ctx.t("sharing.config.sharedNoServer"),
    };
    (Icon::Users, text)
  } else {
    (
      Icon::Lock,
      ctx.t_args("sharing.config.readOnly", [("admins", admins(ctx, team))]),
    )
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("may-change", team.may_change.to_string())
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&text)
        .id(format!("{id}-text"))
        .variant(HubText::TeamLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// `dana-k (owner) and olha (server admin)`: the owners and the server's admins as last read. A server that does not
/// name its admins leaves them by their role: `dana-k (owner) and the server's admins`.
fn admins(ctx: &Ctx, team: &TeamConfiguration) -> String {
  let named = |key: &str, names: &[Arc<str>]| {
    names
      .iter()
      .map(|name| ctx.t_args(key, [("name", name.to_string())]).to_string())
      .collect::<Vec<_>>()
  };
  let owners = named("sharing.config.owner", &team.owners);
  if team.admins.is_empty() {
    if owners.is_empty() {
      return ctx.t("sharing.config.noOwners").to_string();
    }
    let owners = owners.join(&ctx.t("sharing.config.separator"));
    return ctx
      .t_args("sharing.config.ownersAndAdmins", [("owners", owners)])
      .to_string();
  }
  let mut all = owners;
  all.extend(named("sharing.config.serverAdmin", &team.admins));
  match all.split_last() {
    Some((last, rest)) if !rest.is_empty() => ctx
      .t_args(
        "sharing.config.and",
        [
          ("first", rest.join(&ctx.t("sharing.config.separator"))),
          ("last", last.clone()),
        ],
      )
      .to_string(),
    _ => all.concat(),
  }
}

/// The note under a library's rows (`Note` `w1TQp` in 01): what publishing sends, or for a member what runs here use.
pub(crate) fn foot(ctx: &Ctx, id: &str, team: &TeamConfiguration) -> Row {
  let text = match (&team.server, team.may_change) {
    (_, false) => ctx.t("sharing.config.memberNote"),
    (Some((_, host)), true) => ctx.t_args("sharing.config.publishNote", [("host", host.to_string())]),
    (None, true) => ctx.t("sharing.config.publishNoteNoServer"),
  };
  note(id, Icon::History, &text)
}

/// When a version was published, as a row says it: `today 10:12`, or its day, `Oct 27`.
pub(crate) fn when(ctx: &Ctx, at: Timestamp) -> String {
  if at.local_day_start() == Timestamp::now().local_day_start() {
    return ctx
      .t_args("sharing.config.today", [("time", display::clock(at))])
      .to_string();
  }
  let (month, day) = display::month_day(at);
  let month = ctx.t_ns(NAMESPACE, display::MONTH_KEYS[month]).to_string();
  ctx
    .t_args("sharing.config.day", [("month", month), ("day", day.to_string())])
    .to_string()
}

/// Who published the version a row stands for and when: the person for a version published here that waits, else
/// the publisher of the latest version the server numbered.
pub(crate) fn publisher(ctx: &Ctx, id: &str, team: &TeamConfiguration, standing: &SharedStanding) -> Element {
  let waiting = standing.waiting.last();
  let (name, at) = match (waiting, &standing.confirmed) {
    (Some(waiting), _) => (team.me.clone(), Some(waiting.published_at)),
    (None, Some(confirmed)) => (
      confirmed.publisher.as_deref().map(Arc::from),
      Some(confirmed.published_at),
    ),
    (None, None) => (None, None),
  };
  let mut cell = Row::new()
    .id(id.to_owned())
    .width(Dimension::Px(PUBLISHER_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  let Some(at) = at else {
    return cell.into();
  };
  let shown = name.clone().unwrap_or_else(|| ctx.t("sharing.config.here"));
  if let Some(name) = &name {
    // A username's parts are its words: `dana-k` is DK (`CEBR2`).
    let words = name.replace(['-', '_', '.'], " ");
    cell = cell.child(person_avatar(&format!("{id}-avatar"), &initials(&words)));
  }
  cell
    .child(
      Text::new(&ctx.t_args(
        "sharing.config.publisher",
        [("name", shown.to_string()), ("when", when(ctx, at))],
      ))
      .id(format!("{id}-text"))
      .variant(HubText::Hint.role())
      .nowrap(),
    )
    .into()
}

/// A row's state at its end (`Status` `AqhGP` in 01): *v4*, *v4 · your draft*, *Published here, not sent*.
pub(crate) fn row_state(ctx: &Ctx, id: &str, standing: &SharedStanding, draft: DraftOn) -> Element {
  let said = Said::of(standing, draft);
  let text = match said {
    Said::Version(version) => ctx.t_args("sharing.config.state.version", [("version", version.get().to_string())]),
    Said::NotPublished => ctx.t("sharing.config.head.notPublished"),
    Said::Draft(Some(version)) => ctx.t_args("sharing.config.state.draft", [("version", version.get().to_string())]),
    Said::Draft(None) => ctx.t("sharing.config.state.firstDraft"),
    Said::NotSent => ctx.t("sharing.config.state.notSent"),
    Said::Conflict(latest) => ctx.t_args("sharing.config.state.conflict", [("latest", latest.get().to_string())]),
    Said::Refused => ctx.t("sharing.config.state.refused"),
  };
  Row::new()
    .width(Dimension::Px(STATE_WIDTH))
    .flex_shrink(0.0)
    .justify(Justify::End)
    .child(status(id, said.tone(), &text))
    .into()
}

/// An editor's line under its title (`Sub` `Du8OB` in 02): the latest version the server numbered with its publisher,
/// then where the resource stands here; a member's says it is the one used here (`V9rPvj` in 03).
pub(crate) fn head_line(
  ctx: &Ctx,
  id: &str,
  team: &TeamConfiguration,
  standing: &SharedStanding,
  draft: DraftOn,
) -> Row {
  let summary = match &standing.confirmed {
    Some(confirmed) => {
      let (version, when) = (confirmed.number.get().to_string(), when(ctx, confirmed.published_at));
      match &confirmed.publisher {
        Some(name) => ctx.t_args(
          "sharing.config.head.published",
          [("version", version), ("name", name.clone()), ("when", when)],
        ),
        None => ctx.t_args(
          "sharing.config.head.publishedHere",
          [("version", version), ("when", when)],
        ),
      }
    }
    None => ctx.t("sharing.config.head.notPublished"),
  };
  let said = Said::of(standing, draft);
  let state = match said {
    Said::Version(_) if team.may_change => None,
    Said::Version(_) => Some(ctx.t("sharing.config.head.used")),
    Said::NotPublished => None,
    Said::Draft(Some(version)) => {
      Some(ctx.t_args("sharing.config.head.draft", [("version", version.get().to_string())]))
    }
    Said::Draft(None) => Some(ctx.t("sharing.config.head.firstDraft")),
    Said::NotSent => Some(ctx.t("sharing.config.head.notSent")),
    Said::Conflict(_) => Some(ctx.t("sharing.config.head.conflict")),
    Said::Refused => Some(ctx.t("sharing.config.head.refused")),
  };
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&summary)
        .id(format!("{id}-summary"))
        .variant(HubText::Hint.role()),
    )
    .with_children(state.map(|state| {
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("sharing.config.dot")).variant(HubText::Hint.role()))
        .child(status(&format!("{id}-state"), said.tone(), &state))
    }))
}

/// What a shared draft bar says first (`g9yfm` in 02): *Draft on v4 · saved 16:40 · only on this computer*.
pub(crate) fn draft_status(ctx: &Ctx, base: Option<VersionNumber>, saved: &str) -> Arc<str> {
  match base {
    Some(version) => ctx.t_args(
      "sharing.config.bar.draft",
      [("version", version.get().to_string()), ("saved", saved.to_owned())],
    ),
    None => ctx.t_args("sharing.config.bar.firstDraft", [("saved", saved.to_owned())]),
  }
}

/// What a shared draft bar says under it (`MI3ni` in 02): what changed, and that publishing makes `next` for the
/// whole team.
pub(crate) fn draft_sub(ctx: &Ctx, changes: Option<&str>, next: u32) -> Arc<str> {
  match changes {
    Some(changes) => ctx.t_args(
      "sharing.config.bar.makes",
      [("changes", changes.to_owned()), ("next", next.to_string())],
    ),
    None => ctx.t_args("sharing.config.bar.makesOnly", [("next", next.to_string())]),
  }
}

/// *Publish for the team*, the Publish button's words in a shared project (`oG6rJ` in 02).
pub(crate) fn publish_label(ctx: &Ctx) -> Arc<str> {
  ctx.t("sharing.config.bar.publish")
}

/// Sends what was just published in the project at `root` to its team server; nothing for a local project. What
/// could not be sent shows on the editor's bar, so it is only logged here.
pub(crate) async fn send_published(services: &Services, root: &std::path::Path) {
  if let Err(error) = services.servers.send_published(root).await {
    tracing::warn!(?error, "a version published here was not sent to the team server");
  }
}

pub(crate) use super::waiting_bar::{Reread, waiting_bar};
