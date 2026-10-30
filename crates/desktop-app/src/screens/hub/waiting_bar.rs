//! The bar in an admin's editor in place of the draft bar while a version published here waits for the team server
//! (`Shared configuration · redesign` `UC5xk`): not sent, with *Try again* (`p2Qaq` in 02b `iLwm9`); met a version
//! someone published from the same base meanwhile, with what that version changed, *Compare with vN*, *Open my draft
//! on vN* and *Publish mine as vN+1* (`Conflict bar` `n9GgD` in 02c `JZFlA`); or refused by the server. Nothing is
//! merged.

use std::{path::PathBuf, sync::Arc};

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
  resource::{ResourceKind, VersionNumber},
};
use orchester_store::{PublishState, SharedStanding, WaitingVersion};
use orchester_workspace::{
  background, display,
  messages::{self, Message},
  sharing::{compare, team::TeamConfiguration},
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

/// The bar in an admin's editor in place of the draft bar while a version published here waits (02b, 02c); `None`
/// while none does. After *Open my draft on vN* the editor runs `reread`.
pub(crate) fn waiting_bar(
  ctx: &mut Ctx,
  project: &ShownProject,
  (team, standing): (&TeamConfiguration, &SharedStanding),
  (id, kind): (&str, Outside),
  reread: Reread,
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
        root: project.open.repo_root.clone(),
        project: project.open.project.id(),
        host: team.server.as_ref().map(|(_, host)| host.clone()),
        confirmed: standing
          .confirmed
          .as_ref()
          .map(|confirmed| (confirmed.number, confirmed.publisher.as_deref().map(Arc::from))),
        resource: (standing.kind, standing.resource),
        waiting,
        kind,
        reread,
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
}

impl DevtoolsInspectable for Act {}

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
}

impl PartialEq for WaitingProps {
  /// `reread` is made anew on every render of the editor, and only ever runs its reload.
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
  act: FutureAction<Act, Act, Message>,
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
    let reread = props.reread.clone();
    let act = ctx.future_action(move |act: Act| {
      let (services, root) = (services.clone(), props.root.clone());
      let (project, kind, resource) = (props.project, props.kind, props.resource.0);
      async move {
        match act {
          Act::Send => services.servers.send_published(&root).await?,
          Act::PublishAgain(version) => services.servers.publish_again(&root, version).await?,
          Act::Reopen(version) => services.servers.reopen_draft(&root, resource, version).await?,
        }
        // The library's screens read the resource again: with the number the server gave it, or its draft.
        services.changes.record(project, kind);
        Ok(act)
      }
    });
    // The draft the editor shows is not the one it wrote: it shows the saved one anew.
    ctx.watch(&act.state(), move |state| {
      if state.is_fulfilled() && matches!(state.data, Some(Act::Reopen(_))) {
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
        DraftBar::new(
          format!("{id}-not-sent"),
          Standing::NotSent,
          ctx.t_args("sharing.config.bar.notSent", [("time", time)]),
        )
        .sub(ctx.t_args("sharing.config.bar.notSentBody", [("host", host)]))
        .action(retry)
        .into()
      }
      PublishState::Conflict { latest } => self.conflict(ctx, &props, *latest),
      PublishState::Refused { .. } => DraftBar::new(
        format!("{id}-refused"),
        Standing::Attention,
        ctx.t_args("sharing.config.bar.refused", [("host", host), ("time", time)]),
      )
      .sub(ctx.t("sharing.config.bar.refusedBody"))
      .into(),
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

  /// *olha published v5 while you edited v4*, what v5 changed, with *Compare with v5*, *Open my draft on v5* and
  /// *Publish mine as v6* (`n9GgD` in 02c). Nothing is merged.
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
    let mut bar = DraftBar::new(format!("{id}-conflict"), Standing::Conflict, status)
      .sub(ctx.t_args(
        "sharing.config.bar.conflictBody",
        [("latest", latest_text.clone()), ("next", next.clone())],
      ))
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
    bar
      .action(self.button(
        (format!("{id}-open-draft"), ButtonKind::Secondary, Icon::GitMerge),
        ctx.t_args("sharing.config.bar.openDraft", [("latest", latest_text)]),
        Act::Reopen(version),
      ))
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
