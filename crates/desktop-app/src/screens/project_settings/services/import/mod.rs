//! Import draft under Project settings › Services (`05 · Import draft` `DFWRJ`; `docs/architecture/agent-testing.md`,
//! "Import"): the crumb back to Services and the head, then what the checkout suggests grouped by the file it came
//! from ([`files`]), each proposal with what was found, what could not be carried and Keep or Skip, and at the end
//! what the drafts start with, the tally and *Save N drafts*.
//!
//! The checkout is read once, off the UI thread, when the page opens (`orchester_workspace::agent_testing::import`),
//! and nothing of it runs. Saving makes the kept proposals new drafts as they were read, never published, and returns
//! to the tab. A person saves here, so the widening question automation is asked does not apply.

mod files;

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  agent_testing::import::{Choice, Draft, Imported, Proposal, Proposals, propose_in, save_in},
  background,
  messages::Message,
};

use self::files::{Pick, Skips, file_groups};
use super::super::{
  TabProps,
  kit::{back, page_head},
  page,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  theme::{ExtraSpacing, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    row_group::note,
    state::{EmptyState, load_error, loading},
  },
};

/// The page Import draft.
pub(super) fn import(ctx: &mut Ctx, props: &TabProps) -> Element {
  ctx.mount_keyed::<ImportDraft>("settings-import-draft", props.clone())
}

/// What the checkout suggests, as it was read when the page opened.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Arc<Proposals>);

impl DevtoolsInspectable for Read {}

/// What a save made.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saved(Imported);

impl DevtoolsInspectable for Saved {}

/// A save asked for: the project's folder, the proposals read and the ones kept.
type SaveAsk = (PathBuf, Arc<Proposals>, Choice);
type Save = FutureAction<SaveAsk, Saved, Message>;

struct ImportDraft {
  /// The proposals the person skipped; the others are kept, but for those the project has already.
  skipped: Signal<Skips>,
  /// Counts *Try again*s after a failed read.
  retry: Signal<u64>,
  save: Save,
}

impl Component for ImportDraft {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<TabProps>().clone();
    let page = Self {
      skipped: ctx.signal(Skips::default()),
      retry: ctx.signal(0),
      save: ctx.future_action(|(root, proposals, choice): SaveAsk| async move {
        background::run(move || {
          let chosen = proposals.chosen(&choice)?;
          save_in(&root, &chosen, Timestamp::now()).map(Saved)
        })
        .await?
      }),
    };
    // Once the drafts are saved the project's other screens read again and the page returns to the tab.
    let (navigation, feed) = (Navigation::of(ctx), props.project.feed.clone());
    let tab = services_tab(&props);
    ctx.watch(&page.save.state(), move |state| {
      if state.is_fulfilled() && state.data.is_some() {
        feed.saved();
        navigation.go(tab.clone());
      }
    });
    page
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let root = props.project.open.repo_root.clone();
    let read = ctx
      .future((root, self.retry.get()), |(root, _)| async move {
        background::run(move || propose_in(&root).map(|proposals| Read(Arc::new(proposals)))).await?
      })
      .state()
      .get();
    let mut shown = Column::new()
      .id("settings-import")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb(ctx, &props))
      .child(page_head(
        "settings-import-head",
        &ctx.t("project_settings.services.import.title"),
        Some(&ctx.t("project_settings.services.import.lead")),
      ));
    shown = match (&read.data, &read.error) {
      (Some(Read(proposals)), _) => shown.with_children(self.proposals(ctx, &props, proposals)),
      (None, Some(error)) if !read.is_pending() => {
        let retry = self.retry.clone();
        shown.child(load_error(
          ctx,
          "settings-import-load-error",
          (
            &ctx.t("project_settings.services.import.loadFailed"),
            &message_text(ctx, error),
          ),
          move || retry.update(|count| *count += 1),
        ))
      }
      _ => shown.child(loading(
        "settings-import-loading",
        &ctx.t("project_settings.services.import.reading"),
      )),
    };
    page::scroll(shown)
  }
}

impl ImportDraft {
  /// The file groups, what the drafts start with, and the actions; with nothing to propose, that it found nothing.
  fn proposals(&self, ctx: &mut Ctx, props: &TabProps, proposals: &Arc<Proposals>) -> Vec<Element> {
    let skipped = self.skipped.get().0;
    let mut shown = file_groups(ctx, proposals, &self.skipped);
    if proposals.proposals.is_empty() && proposals.skipped.is_empty() {
      shown.insert(
        0,
        EmptyState::new(
          "settings-import-empty",
          Icon::FileInput,
          ctx.t("project_settings.services.import.empty.title"),
          ctx.t("project_settings.services.import.empty.explanation"),
        )
        .wide()
        .into(),
      );
    }
    if proposals.truncated {
      shown.push(
        note(
          "settings-import-truncated",
          &ctx.t("project_settings.services.import.truncated"),
        )
        .into(),
      );
    }
    if !proposals.proposals.is_empty() {
      shown.push(
        note(
          "settings-import-note",
          &ctx.t("project_settings.services.import.startsWith"),
        )
        .into(),
      );
    }
    shown.push(self.actions(ctx, props, proposals, &skipped));
    shown
  }

  /// The tally, *Cancel* and *Save N drafts* (`Actions` `F6Ma9g`), and under them why a save failed.
  fn actions(&self, ctx: &mut Ctx, props: &TabProps, proposals: &Arc<Proposals>, skipped: &BTreeSet<Pick>) -> Element {
    let kept: Vec<&Proposal> = proposals
      .proposals
      .iter()
      .filter(|proposal| files::kept(proposal, skipped))
      .collect();
    let rows = proposals.proposals.len() + proposals.skipped.len();
    let tally = ctx.t_args(
      "project_settings.services.import.tally",
      [
        ("kept", kept.len().to_string()),
        ("skipped", (rows - kept.len()).to_string()),
      ],
    );
    let cancel = ActionButton::new(
      "settings-import-cancel",
      ButtonKind::Ghost,
      ctx.t("project_settings.services.import.cancel"),
    )
    .opens(Navigation::of(ctx).to(services_tab(props)));
    let label = if kept.len() == 1 {
      ctx.t("project_settings.services.import.save.one")
    } else {
      ctx.t_args(
        "project_settings.services.import.save.other",
        [("count", kept.len().to_string())],
      )
    };
    let save = ActionButton::new("settings-import-save", ButtonKind::Primary, label).icon(Icon::Check);
    let state = self.save.state().get();
    let save = if kept.is_empty() {
      save.unavailable(ctx.t("project_settings.services.import.saveWaits"))
    } else if state.is_pending() {
      save.loading()
    } else {
      let choice = choice(&kept);
      let (action, root, proposals) = (
        self.save.clone(),
        props.project.open.repo_root.clone(),
        Arc::clone(proposals),
      );
      save.on_click(move || {
        if !action.state().get_untracked().is_pending() {
          action.run((root.clone(), Arc::clone(&proposals), choice.clone()));
        }
      })
    };
    let line = Row::new()
      .id("settings-import-actions")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Xs))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&tally)
          .id("settings-import-tally")
          .variant(ProjectSettingsText::Hint.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(cancel)
      .child(save);
    let failure = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|error| field_error("settings-import-save-error", &message_text(ctx, error)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(line)
      .with_children(failure)
      .into()
  }
}

/// The kept proposals by their names, services and stacks apart, as saving takes them.
fn choice(kept: &[&Proposal]) -> Choice {
  let mut choice = Choice::default();
  for proposal in kept {
    let name = proposal.draft.name().to_owned();
    match proposal.draft {
      Draft::Service(_) => choice.services.push(name),
      Draft::Stack(_) => choice.stacks.push(name),
    }
  }
  choice
}

fn services_tab(props: &TabProps) -> Route {
  Route::Project(
    props.project.open.project.id(),
    ProjectPage::Settings(ProjectSettingsTab::Services),
  )
}

/// The crumb back to Services with *Import draft* after it (`Crumb` `aGsE0`).
fn crumb(ctx: &mut Ctx, props: &TabProps) -> Element {
  back(
    "settings-import-crumb",
    (
      &ctx.t("project_settings.services.tab"),
      Some(&ctx.t("project_settings.services.importDraft")),
    ),
    Navigation::of(ctx).to(services_tab(props)),
  )
}
