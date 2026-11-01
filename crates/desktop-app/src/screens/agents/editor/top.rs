//! The top of a profile's page: the crumb, the page head with the … menu, and the draft bar or what is asked in its
//! place. In a shared project the head says the team's version and where it stands here (`Sub` `Du8OB` in 02
//! `Sflw7`), a member reads who may change the profile in the bar's place (03 `pjEjf`), and an admin sees a version
//! published here that waits (02b `iLwm9`, 02c `JZFlA`).
use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{AgentProfile, ResourceKind};
use orchester_workspace::{agents::data::ProfileEntry, sharing::team::TeamConfiguration, tasks::outside::Outside};

use super::{
  super::{
    entries, page_head,
    questions::{self, Asking},
    shared::ConnectionHere,
  },
  Ask, Editor, Props, State,
};
use crate::{
  screens::hub::{discard, removal, team},
  theme::HubSpacing,
  ui::icon::Icon,
};

impl Editor {
  /// `here` is what the head's line says of this computer and `bars` follow the head: what is missing on this
  /// computer, a version that widens (04, 02d, 06); `connection`, the team's connection as this computer has it, gives
  /// the head its mark while it is missing here.
  pub(super) fn top(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (state, team): (&State, Option<&TeamConfiguration>),
    (bars, here, connection): (Vec<Element>, Option<team::Here>, Option<&ConnectionHere>),
  ) -> Column {
    // A member of a shared project removes none of the team's profiles.
    let more = state
      .saved
      .as_ref()
      .filter(|_| team::may_change(team))
      .map(|_| page_head::More {
        open: self.more.clone(),
        anchor: self.anchor.clone(),
        remove: {
          let ask = self.ask.clone();
          std::sync::Arc::new(move || ask.set(Ask::Remove))
        },
      });
    let shared = team.zip(state.saved.as_ref()).and_then(|(team, entry)| {
      let standing = team.standing(entry.id)?;
      Some(team::head_line_with(
        ctx,
        "agent-team-state",
        (team, standing, team::draft_on(entry.state)),
        here,
      ))
    });
    let versions = state
      .saved
      .as_ref()
      .and_then(|entry| team::member_versions(ctx, "agent", (&props.project, team), entry.id));
    // The team's connection missing here still shows its CLI's mark, or an API's (`FNW1M` in 04, `qQCQN` in 06c).
    let glyph = connection
      .filter(|connection| connection.local.is_none())
      .map(|connection| connection.cli.map_or(Icon::Cloud, entries::cli_glyph));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Row.role())
      .child(page_head::crumb(ctx, &props.nav))
      .child(page_head::head(
        ctx,
        (&self.fields, &props.listed),
        (more, glyph),
        (shared, versions),
      ))
      .with_children(bars)
      .with_children(self.remove_question(ctx, props, state))
      .child(self.bar_or_question(ctx, props, (state, team)))
      .with_children(self.notices(ctx, props, state))
  }

  fn remove_question(&self, ctx: &mut Ctx, props: &Props, state: &State) -> Option<Element> {
    let entry = state.saved.as_ref().filter(|_| self.ask.get() == Ask::Remove)?;
    let (keep, nav) = (self.ask.clone(), props.nav.clone());
    Some(removal::question(
      ctx,
      removal::RemoveProps {
        project: props.project.clone(),
        resource: entry.id,
        kind: ResourceKind::AgentProfile,
        name: entry.profile.role().as_str().into(),
        on_keep: std::sync::Arc::new(move || keep.set(Ask::Nothing)),
        on_removed: std::sync::Arc::new(move || nav.removed()),
      },
    ))
  }

  /// The draft bar, or what is asked about the draft in its place.
  fn bar_or_question(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (state, team): (&State, Option<&TeamConfiguration>),
  ) -> Element {
    if let Some(team) = team.filter(|team| !team.may_change) {
      return team::line(ctx, "agent-team-line", team).into();
    }
    let asking = Asking {
      pending: self.pending.clone(),
      allowed: self.allowed.clone(),
      work: self.work.clone(),
    };
    if let Some(question) = questions::stale(ctx, &asking) {
      return question;
    }
    if let Some(entry) = state.saved.as_ref().filter(|_| self.ask.get() == Ask::Discard) {
      return self.discard_question(ctx, props, entry);
    }
    let standing = team
      .zip(state.saved.as_ref())
      .and_then(|(team, entry)| Some((team, team.standing(entry.id)?)));
    let revision = props.revision.clone();
    // The page reads the saved profile anew into its fields, as after Discard draft.
    let reread: team::Reread = std::sync::Arc::new(move || revision.update(|n| *n += 1));
    if let Some(bar) =
      standing.and_then(|shared| team::waiting_bar(ctx, &props.project, shared, ("agent", Outside::Profiles), reread))
    {
      return bar;
    }
    let published = state
      .saved
      .as_ref()
      .and_then(|entry| props.listed.published.iter().find(|p| p.id == entry.id))
      .map(|p| &p.profile);
    let actions = self.bar_actions(ctx, props, (state, team.is_some()));
    page_head::bar(
      ctx,
      &self.fields,
      &props.listed,
      page_head::Bar {
        entry: state.saved.as_ref(),
        published,
        saved_at: state
          .saved
          .as_ref()
          .and_then(|entry| props.listed.draft_times.get(&entry.id).copied()),
        publishable: self.publishable(props),
        autopilot_waits: questions::autopilot_waits(&self.pending.get()),
        shared: team.is_some(),
        actions,
      },
    )
  }

  fn discard_question(&self, ctx: &mut Ctx, props: &Props, entry: &ProfileEntry) -> Element {
    let published = entry.state.published().map_or(0, |v| v.get());
    let (root, id, expected) = (props.project.open.repo_root.clone(), entry.id, entry.profile.clone());
    let (keep, done, revision) = (self.ask.clone(), self.ask.clone(), props.revision.clone());
    discard::question(
      ctx,
      discard::DiscardProps {
        project: props.project.clone(),
        versions: (published + 1, published),
        discard: std::sync::Arc::new(move || {
          orchester_workspace::removal::discard_in::<AgentProfile>(&root, id, &expected)
        }),
        change: Outside::Profiles,
        on_keep: std::sync::Arc::new(move || keep.set(Ask::Nothing)),
        on_discarded: std::sync::Arc::new(move || {
          done.set(Ask::Nothing);
          // The page reads the published version anew into its fields.
          revision.update(|n| *n += 1);
        }),
      },
    )
  }
}
