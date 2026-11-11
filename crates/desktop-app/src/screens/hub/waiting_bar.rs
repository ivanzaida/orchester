//! The bar in an admin's editor in place of the draft bar while a version published here waits for the team server
//! (`Shared configuration · redesign` `UC5xk`): not sent, with *Try again* (`p2Qaq` in 02b `iLwm9`); met a version
//! someone published from the same base meanwhile, with what that version changed, *Compare with vN*, *Open my draft
//! on vN* and *Publish mine as vN+1* (`Conflict bar` `n9GgD` in 02c `JZFlA`); or refused by the server. Nothing is
//! merged. A pipeline holds no draft (D-032): its version is taken back instead, its steps handed to its editor as an
//! edit not saved yet ([`pipeline_waiting_bar`]), and it has *Take it back* where others have *Open my draft on vN*.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ProjectId, ResourceId, VersionId},
  resource::{Pipeline, ResourceKind, VersionNumber},
};
use orchester_store::{PublishState, SharedStanding, WaitingVersion};
use orchester_workspace::{
  background, display,
  messages::{self, Message},
  sharing::{compare, team::TeamConfiguration, team_edit::Refusal},
  tasks::outside::Outside,
};

use super::{
  draft_bar::{DraftBar, Standing},
  versions::{compare_panel, field_names, opened},
};
use crate::{
  project::ShownProject,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

/// What an editor does to show its resource anew from what is saved, e.g. after *Open my draft on vN* gave it a draft
/// it did not write itself.
pub(crate) type Reread = Arc<dyn Fn() + Send + Sync>;

/// What a pipeline's editor does with the steps of its version taken back: it holds them as its edit, not saved.
pub(crate) type HandBack = Arc<dyn Fn(Pipeline) + Send + Sync>;

/// The bar in an admin's editor in place of the draft bar while a version published here waits (02b, 02c); `None`
/// while none does. After *Open my draft on vN* the editor runs `reread`.
pub(crate) fn waiting_bar(
  ctx: &mut Ctx,
  project: &ShownProject,
  standing: (&TeamConfiguration, &SharedStanding),
  (id, kind): (&str, Outside),
  reread: Reread,
) -> Option<Element> {
  waiting_bar_in(
    ctx,
    (&project.open.repo_root, project.open.project.id()),
    standing,
    (id, kind),
    reread,
  )
}

/// [`waiting_bar`] in a pipeline's editor: taking its version back hands its steps to `hand_back`, then runs `reread`.
pub(crate) fn pipeline_waiting_bar(
  ctx: &mut Ctx,
  project: &ShownProject,
  standing: (&TeamConfiguration, &SharedStanding),
  (reread, hand_back): (Reread, HandBack),
) -> Option<Element> {
  bar(
    ctx,
    (&project.open.repo_root, project.open.project.id()),
    standing,
    ("pipeline", Outside::Pipelines),
    (reread, Some(hand_back)),
  )
}

/// [`waiting_bar`] for a panel that has the project's folder and id, not its shown project.
pub(crate) fn waiting_bar_in(
  ctx: &mut Ctx,
  (root, project): (&Path, ProjectId),
  (team, standing): (&TeamConfiguration, &SharedStanding),
  (id, kind): (&str, Outside),
  reread: Reread,
) -> Option<Element> {
  bar(ctx, (root, project), (team, standing), (id, kind), (reread, None))
}

fn bar(
  ctx: &mut Ctx,
  (root, project): (&Path, ProjectId),
  (team, standing): (&TeamConfiguration, &SharedStanding),
  (id, kind): (&str, Outside),
  (reread, hand_back): (Reread, Option<HandBack>),
) -> Option<Element> {
  let waiting = standing.waiting.first()?.clone();
  if !team.may_change {
    return None;
  }
  Some(
    ctx.mount_keyed::<WaitingBar>(
      &format!("{id}-waiting-{}", waiting.version),
      WaitingProps {
        id: id.into(),
        root: root.to_owned(),
        project,
        host: team.server.as_ref().map(|(_, host)| host.clone()),
        confirmed: standing
          .confirmed
          .as_ref()
          .map(|confirmed| (confirmed.number, confirmed.publisher.as_deref().map(Arc::from))),
        resource: (standing.kind, standing.resource),
        waiting,
        kind,
        reread,
        hand_back,
      },
    ),
  )
}

/// What the waiting bar does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
  /// *Try again*.
  Send,
  /// *Publish mine as vN*.
  PublishAgain(VersionId),
  /// *Open my draft on vN*.
  Reopen(VersionId),
  /// A pipeline's *Cancel sending* and *Take it back*: its steps go back to its editor, not saved.
  TakeBack(VersionId),
  /// *Discard it*: the new resource whose first version the server did not take is taken back as if never created.
  DiscardNew,
}

impl DevtoolsInspectable for Act {}

/// What an action did: the action, and the steps of a pipeline's version it took back.
#[derive(Clone, Debug, PartialEq)]
struct Acted {
  act: Act,
  steps: Option<Pipeline>,
}

impl DevtoolsInspectable for Acted {}

#[derive(Clone)]
pub(crate) struct WaitingProps {
  id: Arc<str>,
  root: PathBuf,
  project: ProjectId,
  host: Option<Arc<str>>,
  /// The latest version the server numbered and its publisher.
  confirmed: Option<(VersionNumber, Option<Arc<str>>)>,
  /// The resource the version is of, and its kind.
  resource: (ResourceKind, ResourceId),
  waiting: WaitingVersion,
  /// The library whose screens read their resources again after an action.
  kind: Outside,
  reread: Reread,
  /// The editor of a pipeline, which holds the steps of its version taken back.
  hand_back: Option<HandBack>,
}

impl PartialEq for WaitingProps {
  /// `reread` and `hand_back` are made anew on every render of the editor, and only ever run its reload.
  fn eq(&self, other: &Self) -> bool {
    (
      &self.id,
      &self.root,
      self.project,
      &self.host,
      &self.confirmed,
      self.resource,
      &self.waiting,
      self.kind,
    ) == (
      &other.id,
      &other.root,
      other.project,
      &other.host,
      &other.confirmed,
      other.resource,
      &other.waiting,
      other.kind,
    )
  }
}

impl DevtoolsInspectable for WaitingProps {}

pub(crate) struct WaitingBar {
  act: FutureAction<Act, Acted, Message>,
  /// The action pressed last, which shows it works while it does.
  pressed: Signal<Option<Act>>,
  /// *Compare with vN*'s popover is open.
  comparing: Signal<bool>,
  compare_anchor: ElementRef,
}

impl Component for WaitingBar {
  type Props = WaitingProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<WaitingProps>().clone();
    let services = Services::of(ctx);
    let (reread, hand_back) = (props.reread.clone(), props.hand_back.clone());
    let act = ctx.future_action(move |act: Act| {
      let (services, root) = (services.clone(), props.root.clone());
      let (project, kind, resource) = (props.project, props.kind, props.resource.0);
      async move {
        let mut steps = None;
        match act {
          Act::Send => services.servers.send_published(&root).await?,
          Act::PublishAgain(version) => services.servers.publish_again(&root, version).await?,
          Act::Reopen(version) => services.servers.reopen_draft(&root, resource, version).await?,
          Act::TakeBack(version) => steps = Some(services.servers.take_back_pipeline(&root, version).await?),
          Act::DiscardNew => services.servers.cancel_created(&root, props.resource.1).await?,
        }
        // The library's screens read the resource again: with the number the server gave it, or its draft.
        services.changes.record(project, kind);
        Ok(Acted { act, steps })
      }
    });
    // The draft the editor shows is not the one it wrote, or a pipeline's steps came back: it shows the saved one
    // anew, a pipeline holding the steps taken back as its edit.
    ctx.watch(&act.state(), move |state| {
      let Some(acted) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
        return;
      };
      if let (Some(steps), Some(hand_back)) = (&acted.steps, &hand_back) {
        hand_back(steps.clone());
      }
      if matches!(acted.act, Act::Reopen(_) | Act::TakeBack(_)) {
        reread();
      }
    });
    Self {
      act,
      pressed: ctx.signal(None),
      comparing: ctx.signal(false),
      compare_anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<WaitingProps>().clone();
    let state = self.act.state().get();
    let id = props.id.as_ref();
    let host = props
      .host
      .as_deref()
      .map_or_else(|| ctx.t("sharing.config.bar.theServer").to_string(), str::to_owned);
    let time = display::clock(props.waiting.published_at);
    let bar: Element = match &props.waiting.state {
      PublishState::Waiting => {
        let retry = self.button(
          (format!("{id}-try-again"), ButtonKind::Secondary, Icon::RefreshCw),
          ctx.t("sharing.config.bar.tryAgain"),
          Act::Send,
        );
        // A version that edited an earlier one goes back to being the draft; a first version has none to go back to.
        let cancel = props.waiting.based_on.is_some().then(|| {
          self.button(
            (format!("{id}-cancel-sending"), ButtonKind::Secondary, Icon::X),
            ctx.t("sharing.config.bar.cancelSending"),
            take_back(&props),
          )
        });
        DraftBar::new(
          format!("{id}-not-sent"),
          Standing::NotSent,
          ctx.t_args("sharing.config.bar.notSent", [("time", time)]),
        )
        .sub(ctx.t_args("sharing.config.bar.notSentBody", [("host", host)]))
        .action(retry)
        .actions(cancel)
        .into()
      }
      PublishState::Conflict { latest } => self.conflict(ctx, &props, *latest),
      PublishState::Refused { code } => {
        // What was said, in words: the draft comes back to be changed, and then published again.
        let refusal = Refusal::of_code(code);
        let reason = message_text(ctx, &refusal.message());
        // A version that edited an earlier one goes back to being the draft; a new resource's first has none, so the
        // way out is to take the resource back as if never created.
        let (way_out, way_out_words) = match props.waiting.based_on {
          Some(_) if props.hand_back.is_some() => (
            self.take_back_button(ctx, &props),
            "sharing.config.bar.pipeline.refusedWhy",
          ),
          Some(_) => (self.take_back_button(ctx, &props), "sharing.config.bar.refusedWhy"),
          None => (
            self.button(
              (format!("{id}-discard-new"), ButtonKind::Secondary, Icon::Trash2),
              ctx.t("sharing.config.bar.discardNew"),
              Act::DiscardNew,
            ),
            "sharing.config.bar.refusedNewWhy",
          ),
        };
        // This computer found the team cannot take it: nothing reached the server, so it did not refuse anything.
        let title = if refusal.is_local() {
          ctx.t_args("sharing.config.bar.refusedHere", [("time", time)])
        } else {
          ctx.t_args("sharing.config.bar.refused", [("host", host), ("time", time)])
        };
        DraftBar::new(format!("{id}-refused"), Standing::Attention, title)
          .sub(ctx.t_args(way_out_words, [("reason", reason.to_string())]))
          .actions(Some(way_out))
          .into()
      }
    };
    let failure = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|error| field_error(&format!("{id}-send-error"), &message_text(ctx, error)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(bar)
      .with_children(failure)
  }
}

impl WaitingBar {
  /// One of the bar's actions: it runs `act`, and shows it works while `act` does.
  fn button(&self, (id, kind, glyph): (String, ButtonKind, Icon), label: Arc<str>, act: Act) -> ActionButton {
    let button = ActionButton::new(id, kind, label).icon(glyph);
    if self.act.state().get().is_pending() {
      return if self.pressed.get() == Some(act) {
        button.loading()
      } else {
        button.disabled()
      };
    }
    let (run, pressed) = (self.act.clone(), self.pressed.clone());
    button.on_click(move || {
      pressed.set(Some(act));
      run.run(act);
    })
  }

  /// *Take it back as a draft*, or a pipeline's *Take it back*, whose steps go back to its editor (D-032).
  fn take_back_button(&self, ctx: &Ctx, props: &WaitingProps) -> ActionButton {
    let label = if props.hand_back.is_some() {
      "sharing.config.bar.pipeline.takeBack"
    } else {
      "sharing.config.bar.takeBack"
    };
    self.button(
      (format!("{}-take-back", props.id), ButtonKind::Secondary, Icon::Undo2),
      ctx.t(label),
      take_back(props),
    )
  }

  /// *olha published v5 while you edited v4*, what v5 changed, with *Compare with v5*, *Open my draft on v5* (a
  /// pipeline's *Take it back*) and *Publish mine as v6* (`n9GgD` in 02c). Nothing is merged.
  fn conflict(&self, ctx: &mut Ctx, props: &WaitingProps, latest: VersionNumber) -> Element {
    let id = props.id.as_ref();
    let name = props
      .confirmed
      .as_ref()
      .and_then(|(_, name)| name.clone())
      .unwrap_or_else(|| ctx.t("sharing.config.bar.someone"));
    let (latest_text, next) = (latest.get().to_string(), latest.get().saturating_add(1).to_string());
    let status = match props.waiting.based_on {
      Some(base) => ctx.t_args(
        "sharing.config.bar.conflict",
        [
          ("name", name.to_string()),
          ("latest", latest_text.clone()),
          ("base", base.get().to_string()),
        ],
      ),
      None => ctx.t_args(
        "sharing.config.bar.conflictFirst",
        [("name", name.to_string()), ("latest", latest_text.clone())],
      ),
    };
    let version = props.waiting.version;
    let comparison = self.comparison(ctx, props, latest);
    let body = if props.hand_back.is_some() {
      "sharing.config.bar.pipeline.conflictBody"
    } else {
      "sharing.config.bar.conflictBody"
    };
    let mut bar = DraftBar::new(format!("{id}-conflict"), Standing::Conflict, status)
      .sub(ctx.t_args(body, [("latest", latest_text.clone()), ("next", next.clone())]))
      .actions_below();
    if let Some(comparison) = &comparison {
      if !comparison.latest_changed.is_empty() {
        bar = bar.foot(ctx.t_args(
          "sharing.config.bar.latestChanged",
          [
            ("latest", latest_text.clone()),
            ("fields", field_names(ctx, &comparison.latest_changed)),
          ],
        ));
      }
      let open = self.comparing.clone();
      let compare = ActionButton::new(
        format!("{id}-compare"),
        ButtonKind::Ghost,
        ctx.t_args("sharing.config.bar.compare", [("latest", latest_text.clone())]),
      )
      .icon(Icon::GitCompareArrows)
      .held(self.comparing.get())
      .element_ref(self.compare_anchor.clone())
      .on_click(move || open.update(|open| *open = !*open));
      let panel = self
        .comparing
        .get()
        .then(|| compare_panel(ctx, &format!("{id}-compare-panel"), latest, &comparison.against_latest));
      bar = bar.action(
        Row::new()
          .child(compare)
          .child(opened(ctx, &self.compare_anchor, &self.comparing, panel)),
      );
    }
    // A pipeline holds no draft to open on vN (D-032): its version is taken back, to be changed on top of vN.
    let back = if props.hand_back.is_some() {
      self.take_back_button(ctx, props)
    } else {
      self.button(
        (format!("{id}-open-draft"), ButtonKind::Secondary, Icon::GitMerge),
        ctx.t_args("sharing.config.bar.openDraft", [("latest", latest_text)]),
        Act::Reopen(version),
      )
    };
    bar
      .action(back)
      .action(self.button(
        (format!("{id}-publish-again"), ButtonKind::Primary, Icon::Upload),
        ctx.t_args("sharing.config.bar.publishMine", [("next", next)]),
        Act::PublishAgain(version),
      ))
      .into()
  }

  /// What `latest` changed and the version published here against it, once read; `None` while it is read, when it
  /// could not be (logged), and while `latest` is not kept here.
  fn comparison(
    &self,
    ctx: &mut Ctx,
    props: &WaitingProps,
    latest: VersionNumber,
  ) -> Option<compare::ConflictComparison> {
    let deps = (
      props.root.clone(),
      props.resource,
      props.waiting.version,
      (props.waiting.based_on, latest),
    );
    let read = ctx.future(deps, |(root, resource, mine, versions)| async move {
      background::run(move || compare::conflict(&root, resource, mine, versions))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot compare a version published here with the newer one");
          messages::store_error(&error)
        })
    });
    read.state().get().data.flatten()
  }
}

/// What takes the version back: a pipeline's steps go back to its editor (D-032), another resource's to its draft.
const fn take_back(props: &WaitingProps) -> Act {
  if props.hand_back.is_some() {
    Act::TakeBack(props.waiting.version)
  } else {
    Act::Reopen(props.waiting.version)
  }
}
