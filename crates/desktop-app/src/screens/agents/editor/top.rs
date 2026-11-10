//! The top of a profile's page: the crumb, the page head with the … menu, and the draft bar or what is asked in its
//! place.
use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{AgentProfile, ResourceKind};
use orchester_workspace::{agents::data::ProfileEntry, local_archive::Archivable, tasks::outside::Outside};

use super::{
  super::{
    page_head,
    questions::{self, Asking},
  },
  Ask, Editor, Props, State,
};
use crate::{
  screens::{hub::discard, local_hub::archive},
  theme::HubSpacing,
};

impl Editor {
  pub(super) fn top(&self, ctx: &mut Ctx, props: &Props, state: &State) -> Column {
    let more = state.saved.as_ref().map(|_| page_head::More {
      open: self.more.clone(),
      anchor: self.anchor.clone(),
      archive: {
        let ask = self.ask.clone();
        std::sync::Arc::new(move || ask.set(Ask::Archive))
      },
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Row.role())
      .child(page_head::crumb(ctx, &props.nav))
      .child(page_head::head(ctx, (&self.fields, &props.listed), more))
      .with_children(self.archive_question(ctx, props, state))
      .child(self.bar_or_question(ctx, props, state))
      .with_children(self.notices(ctx, props, state))
  }

  fn archive_question(&self, ctx: &mut Ctx, props: &Props, state: &State) -> Option<Element> {
    let entry = state.saved.as_ref().filter(|_| self.ask.get() == Ask::Archive)?;
    let (keep, nav) = (self.ask.clone(), props.nav.clone());
    Some(archive::confirm::question(
      ctx,
      archive::confirm::Props {
        root: props.project.open.repo_root.clone(),
        project: props.project.open.project.id(),
        what: Archivable::Resource(entry.id),
        kind: ResourceKind::AgentProfile,
        name: entry.profile.role().as_str().into(),
        on_keep: std::sync::Arc::new(move || keep.set(Ask::Nothing)),
        on_archived: std::sync::Arc::new(move || nav.archived()),
      },
    ))
  }

  /// The draft bar, or what is asked about the draft in its place.
  fn bar_or_question(&self, ctx: &mut Ctx, props: &Props, state: &State) -> Element {
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
    let published = state
      .saved
      .as_ref()
      .and_then(|entry| props.listed.published.iter().find(|p| p.id == entry.id))
      .map(|p| &p.profile);
    let actions = self.bar_actions(ctx, props, state);
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
