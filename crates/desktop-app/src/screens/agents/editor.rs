//! Profile editor gTrvs: header, persistent field values, tabs and independently scrolling body.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::agents::{
  data::{ProfileEntry, ProjectAgents, VersionState},
  editor::{Action, Request},
};

use super::{
  View,
  actions::{self, Pending},
  fields::Fields,
  words,
};
use crate::{
  project::{ShownProject, own::OwnChanges},
  theme::{ExtraSpacing, IconSize, agents::page_spacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    tabs::{PageTab, page_tabs},
  },
};

#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub listed: ProjectAgents,
  pub entry: Option<ProfileEntry>,
  pub view: Signal<View>,
  pub revision: Signal<u64>,
  pub retry: Signal<u64>,
  /// Where a save records its change of the profiles.
  pub profiles: OwnChanges,
  pub load_error: Option<orchester_workspace::messages::Message>,
  pub reading: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.listed == other.listed
      && self.entry == other.entry
      && self.load_error == other.load_error
      && self.reading == other.reading
  }
}
impl DevtoolsInspectable for Props {}
pub struct Editor {
  work: actions::Work,
  fields: Fields,
  original: Option<ProfileEntry>,
  tab: Signal<usize>,
  pending: Signal<Pending>,
  busy: Signal<bool>,
  tried: Signal<bool>,
}
impl Component for Editor {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let editor = Self {
      work: actions::work(ctx),
      fields: Fields::new(ctx, &props.listed, props.entry.as_ref()),
      original: props.entry.clone(),
      tab: ctx.signal(0),
      pending: ctx.signal(Pending(None)),
      busy: ctx.signal(false),
      tried: ctx.signal(false),
    };
    editor
      .fields
      .watch_changes(ctx, props.listed.clone(), editor.pending.clone());
    let work = editor.work.clone();
    actions::follow(ctx, &work, editor.pending.clone(), editor.busy.clone());
    editor
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let mut props = ctx.props::<Props>().clone();
    let changed_outside = props.entry != self.original;
    props.entry = self.original.clone();
    let work = self.work.clone();
    let checked = self.fields.profile(&props.listed);
    let dirty = props
      .entry
      .as_ref()
      .is_none_or(|entry| checked.as_ref() != Ok(&entry.profile));
    let head = self.head(ctx, &props, dirty, &work, checked.as_ref().ok());
    let mut page = Column::new()
      .id("agent-editor")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(head);
    page = page.with_children(read_failure(ctx, &props));
    if self.tried.get()
      && let Err(errors) = &checked
    {
      for error in [&errors.role, &errors.instructions, &errors.connection, &errors.model]
        .into_iter()
        .flatten()
      {
        page = page.child(field_error("agent-validation-error", &message_text(ctx, error)));
      }
    }
    if changed_outside {
      let revision = props.revision.clone();
      page = page
        .child(field_error("agent-outside-change", &ctx.t("agents.outsideChange")))
        .child(
          ActionButton::new("agent-reload-outside", ButtonKind::Secondary, ctx.t("agents.reload"))
            .on_click(move || revision.update(|n| *n += 1)),
        );
    }
    let state = work.state().get();
    if let Some(error) = state.error.clone().filter(|_| state.is_rejected()) {
      let revision = props.revision.clone();
      page = page
        .child(field_error("agent-save-error", &message_text(ctx, &error)))
        .child(
          ActionButton::new("agent-reload", ButtonKind::Secondary, ctx.t("agents.reload"))
            .on_click(move || revision.update(|n| *n += 1)),
        );
    }
    if props.load_error.is_none()
      && !props.reading
      && let Some(question) = actions::confirm(ctx, &self.pending, &self.busy, &work)
    {
      page = page.child(question);
    }
    let body = if self.busy.get() {
      crate::ui::state::loading("agent-saving", &ctx.t("agents.saving")).into()
    } else {
      self.body(ctx, &props, self.pipeline_link_blocked(&props, dirty, changed_outside))
    };
    page.child(
      ScrollVertical::new(body)
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .width(Dimension::Pct(100.0)),
    )
  }
}
impl Editor {
  fn head(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    dirty: bool,
    work: &actions::Work,
    profile: Option<&orchester_core::resource::AgentProfile>,
  ) -> Element {
    let role = self.fields.role.get();
    let title = if role.trim().is_empty() {
      ctx.t("agents.new")
    } else {
      role.as_str().into()
    };
    let list = agents_crumb(ctx, props);
    let crumbs = breadcrumb(
      ctx,
      "agent-crumbs",
      CrumbSeparator::Chevron,
      vec![list, Crumb::here(title.clone())],
    );
    let state = props
      .entry
      .as_ref()
      .map_or(VersionState::Draft { published: None }, |e| e.state);
    let actions = self.buttons(ctx, props, dirty, work, profile);
    let tabs = self.tabs(ctx, props);
    let status = if self.busy.get() {
      ctx.t("agents.saving")
    } else {
      words::version(ctx, state, dirty)
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .left(page_spacing())
          .right(page_spacing())
          .top(SpacingSize::Xl),
      )
      .spacing(SpacingSize::Md)
      .child(crumbs)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .child(
            Column::new()
              .flex(1.0)
              .spacing(SpacingSize::Xs)
              .child(Text::new(&title).variant(TypographyStyle::Heading))
              .child(words::note(&self.fields.responsibility.get())),
          )
          .child(
            Column::new()
              .align_items(Alignment::End)
              .spacing(ExtraSpacing::Snug.role())
              .child(
                Row::new()
                  .spacing(SpacingSize::Md)
                  .align_items(Alignment::Center)
                  .child(save_status(&status, state, dirty))
                  .child(actions),
              )
              .with_children(
                (dirty || !matches!(state, VersionState::Draft { published: Some(_) }))
                  .then(|| Text::new(&words::state_note(ctx, state, dirty)).variant(TypographyStyle::Caption)),
              ),
          ),
      )
      .child(super::enforcement::summary(ctx, &self.fields, &props.listed))
      .child(page_tabs(ctx, "agent-tabs", tabs))
      .into()
  }
  fn tabs(&self, ctx: &Ctx, props: &Props) -> Vec<PageTab> {
    [
      ("agent-tab-profile", ctx.t("agents.profileTab")),
      ("agent-tab-permissions", ctx.t("agents.permissionsTab")),
      (
        "agent-tab-used",
        ctx.t_args(
          "agents.usedTab",
          [(
            "count",
            props
              .entry
              .as_ref()
              .map_or(0, |e| props.listed.usage.count(e.id))
              .to_string(),
          )],
        ),
      ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (id, text))| {
      let tab = self.tab.clone();
      PageTab::new(id, text, self.tab.get() == index, move || tab.set(index))
    })
    .collect::<Vec<_>>()
  }
  fn buttons(
    &self,
    ctx: &Ctx,
    props: &Props,
    dirty: bool,
    work: &actions::Work,
    profile: Option<&orchester_core::resource::AgentProfile>,
  ) -> Element {
    let state = props
      .entry
      .as_ref()
      .map_or(VersionState::Draft { published: None }, |e| e.state);
    let next = state.published().map_or(1, |v| v.get().saturating_add(1));
    let publish = ActionButton::new(
      "agent-publish",
      if dirty {
        ButtonKind::Secondary
      } else {
        ButtonKind::Primary
      },
      ctx.t_args("agents.publish", [("version", next.to_string())]),
    )
    .icon(Icon::Upload);
    let connection_runs = self
      .fields
      .connection(&props.listed)
      .is_some_and(|c| c.provider().runs_agents());
    let can_publish = connection_runs && !dirty && state.has_draft() && props.entry.is_some();
    let publish = if !connection_runs {
      publish.unavailable(ctx.t("agents.notEnforced"))
    } else if self.busy.get()
      || props.reading
      || props.load_error.is_some()
      || !can_publish
      || self.pending.get().0.is_some()
    {
      publish.disabled()
    } else if matches!(state, VersionState::StaleDraft { .. }) {
      let pending = self.pending.clone();
      let request = Request {
        existing: props.entry.clone(),
        profile: props
          .entry
          .as_ref()
          .expect("publish requires a saved profile")
          .profile
          .clone(),
        action: Action::PublishStale,
        allowed: Vec::new(),
      };
      publish.on_click(move || pending.set(Pending(Some(request.clone()))))
    } else {
      self.bind(publish, Action::Publish, props, work, profile)
    };
    let mut row = Row::new().spacing(SpacingSize::Sm).child(publish);
    if dirty {
      let save = ActionButton::new("agent-save", ButtonKind::Primary, ctx.t("agents.save")).icon(Icon::Save);
      let save = if self.busy.get() || props.reading || props.load_error.is_some() || self.pending.get().0.is_some() {
        save.disabled()
      } else {
        self.bind(save, Action::Save, props, work, profile)
      };
      row = row.child(save);
    }
    row.into()
  }
  fn bind(
    &self,
    button: ActionButton,
    action: Action,
    props: &Props,
    work: &actions::Work,
    profile: Option<&orchester_core::resource::AgentProfile>,
  ) -> ActionButton {
    let (work, busy, tried) = (work.clone(), self.busy.clone(), self.tried.clone());
    let request = profile.map(|profile| Request {
      existing: props.entry.clone(),
      profile: profile.clone(),
      action,
      allowed: Vec::new(),
    });
    button.on_click(move || {
      tried.set(true);
      if busy.get() {
        return;
      }
      if let Some(request) = &request {
        busy.set(true);
        work.run(request.clone());
      }
    })
  }
  fn pipeline_link_blocked(&self, props: &Props, dirty: bool, changed_outside: bool) -> Option<&'static str> {
    if self.pending.get().0.is_some() || self.busy.get() {
      Some("agents.pipelineWait")
    } else if dirty {
      Some("agents.pipelineSaveFirst")
    } else if props.reading || props.load_error.is_some() || changed_outside {
      Some("agents.pipelineRefreshFirst")
    } else {
      None
    }
  }
  fn body(&self, ctx: &mut Ctx, props: &Props, pipeline_blocked: Option<&str>) -> Element {
    let body = match self.tab.get() {
      1 => {
        let form = ctx.mount_keyed::<super::permissions::PermissionsForm>(
          "agent-permissions",
          super::permissions::Props {
            fields: self.fields.clone(),
            listed: props.listed.clone(),
            project: props.project.clone(),
          },
        );
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Section)
          .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(form))
          .child(super::policy_panel::enforcement(ctx, &self.fields, &props.listed))
          .into()
      }
      2 => super::uses::render(ctx, props, pipeline_blocked),
      _ => Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Section)
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .child(self.fields.render(ctx, &props.listed)),
        )
        .child(super::panels::draft(ctx, props, &self.fields))
        .into(),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .left(page_spacing())
          .right(page_spacing())
          .top(ExtraSpacing::Tabs.role())
          .bottom(SpacingSize::Lg),
      )
      .child(body)
      .into()
  }
}
fn agents_crumb(ctx: &mut Ctx, props: &Props) -> Crumb {
  let view = props.view.clone();
  let navigation = crate::navigation::Navigation::of(ctx);
  let project = props.project.open.project.id();
  Crumb::link(ctx.t("agents.title"), move || {
    if matches!(
      navigation.route_now(),
      crate::routes::Route::Project(_, crate::routes::ProjectPage::Agent(_))
    ) {
      navigation.go(crate::routes::Route::Project(
        project,
        crate::routes::ProjectPage::Agents,
      ));
    } else {
      view.set(View::List);
    }
  })
}

fn save_status(status: &str, state: VersionState, dirty: bool) -> Row {
  let stale = matches!(state, VersionState::StaleDraft { .. });
  Row::new()
    .id("agent-save-status")
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(
      if dirty {
        Icon::CircleDot
      } else if stale {
        Icon::GitBranch
      } else {
        match state {
          VersionState::Published(_) => Icon::CircleCheck,
          VersionState::Draft { published: None } => Icon::CircleDashed,
          _ => Icon::CircleDot,
        }
      },
      IconSize::Base,
      if stale {
        PaletteColor::Warning
      } else if matches!(state, VersionState::Published(_)) && !dirty {
        PaletteColor::Success
      } else {
        PaletteColor::TextMuted
      },
    ))
    .child(words::note(status).color(if stale {
      PaletteColor::Warning
    } else {
      PaletteColor::TextSecondary
    }))
}

fn read_failure(ctx: &Ctx, props: &Props) -> Option<Element> {
  let error = props.load_error.as_ref()?;
  let retry = props.retry.clone();
  Some(crate::ui::state::load_error(
    ctx,
    "agents-load-error",
    (&ctx.t("agents.loadFailed"), &message_text(ctx, error)),
    move || retry.update(|n| *n += 1),
  ))
}
