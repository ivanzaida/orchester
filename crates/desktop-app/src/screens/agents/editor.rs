//! Profile editor gTrvs: header, persistent field values, tabs and independently scrolling body.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
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
  project::ShownProject,
  theme::{ExtraSpacing, agents::page_spacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    field::field_error,
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
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.listed == other.listed && self.entry == other.entry
  }
}
impl DevtoolsInspectable for Props {}
pub struct Editor {
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
    let work = actions::work(ctx);
    actions::follow(ctx, &work, editor.pending.clone(), editor.busy.clone());
    editor
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let mut props = ctx.props::<Props>().clone();
    let changed_outside = props.entry != self.original;
    props.entry = self.original.clone();
    let work = actions::work(ctx);
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
    if let Some(question) = actions::confirm(ctx, &self.pending, &self.busy, &work) {
      page = page.child(question);
    }
    let body = if self.busy.get() {
      crate::ui::state::loading("agent-saving", &ctx.t("agents.saving")).into()
    } else {
      self.body(ctx, &props)
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
    let view = props.view.clone();
    let crumbs = breadcrumb(
      ctx,
      "agent-crumbs",
      CrumbSeparator::Chevron,
      vec![
        Crumb::link(ctx.t("agents.title"), move || view.set(View::List)),
        Crumb::here(title.clone()),
      ],
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
          .top(SpacingSize::Lg),
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
              .spacing(ExtraSpacing::Snug.role())
              .child(Text::new(&title).variant(TypographyStyle::Heading))
              .child(words::note(&self.fields.responsibility.get())),
          )
          .child(words::note(&status).id("agent-save-status"))
          .child(actions),
      )
      .child(super::enforcement::summary(ctx, &self.fields, &props.listed))
      .child(words::note(&words::state_note(ctx, state, dirty)))
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
      ButtonKind::Primary,
      ctx.t_args("agents.publish", [("version", next.to_string())]),
    );
    let connection_runs = self
      .fields
      .connection(&props.listed)
      .is_some_and(|c| c.provider().runs_agents());
    let can_publish = connection_runs
      && !dirty
      && state.has_draft()
      && !matches!(state, VersionState::StaleDraft { .. })
      && props.entry.is_some();
    let publish = if !connection_runs {
      publish.unavailable(ctx.t("agents.notEnforced"))
    } else if self.busy.get() || !can_publish || self.pending.get().0.is_some() {
      publish.disabled()
    } else {
      self.bind(publish, Action::Publish, props, work, profile)
    };
    let mut row = Row::new().spacing(SpacingSize::Sm).child(publish);
    if dirty {
      let save = ActionButton::new("agent-save", ButtonKind::Primary, ctx.t("agents.save"));
      let save = if self.busy.get() || self.pending.get().0.is_some() {
        save.disabled()
      } else {
        self.bind(save, Action::Save, props, work, profile)
      };
      row = row.child(save);
    }
    if matches!(state, VersionState::StaleDraft { .. }) {
      let pending = self.pending.clone();
      let request = props.entry.as_ref().map(|entry| Request {
        existing: Some(entry.clone()),
        profile: entry.profile.clone(),
        action: Action::KeepDraft,
        allowed: Vec::new(),
      });
      let button = ActionButton::new("agent-keep-draft", ButtonKind::Secondary, ctx.t("agents.keepDraft"));
      row = row.child(if self.busy.get() || dirty || self.pending.get().0.is_some() {
        button.disabled()
      } else if let Some(request) = request {
        button.on_click(move || pending.set(Pending(Some(request.clone()))))
      } else {
        button.disabled()
      });
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
  fn body(&self, ctx: &mut Ctx, props: &Props) -> Element {
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
          .spacing(SpacingSize::Xl)
          .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(form))
          .child(super::panels::enforcement(ctx, &self.fields, &props.listed))
          .into()
      }
      2 => super::uses::render(ctx, props),
      _ => Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xl)
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
      .padding(Padding::symmetric(page_spacing(), SpacingSize::Lg))
      .child(body)
      .into()
  }
}
