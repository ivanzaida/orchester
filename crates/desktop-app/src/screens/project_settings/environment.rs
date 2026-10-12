//! Environment (O8o0l, ryH1J): save on leave, validation and person-only in-place confirmation.
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::CommandEnvironment;
use orchester_workspace::{
  background,
  environment::{self, load_in, save_checked_in},
  messages::{self, Message},
  tasks::outside::Outside,
  tools::form::{environment_text, parse_environment},
};

use super::{Status, TabProps, page};
use crate::{
  screens::connections::widen::{WidenConfirm, WidenProps},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field, field_error},
    form_section::section,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{load_error, loading},
  },
};

/// Area uSWRr/IBFli in O8o0l, at the 720px form width.
const ENVIRONMENT_HEIGHT: f32 = 132.0;

#[derive(Clone, Eq)]
struct Stored(CommandEnvironment, u64);
impl PartialEq for Stored {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}
impl DevtoolsInspectable for Stored {}
#[derive(Clone, PartialEq, Eq)]
struct Requested {
  base: CommandEnvironment,
  asked: CommandEnvironment,
  confirmed: bool,
}
#[derive(Clone, PartialEq, Eq)]
struct Asking(Option<Requested>);
impl DevtoolsInspectable for Asking {}
type Save = FutureAction<(PathBuf, Requested), Stored, Message>;

#[derive(Clone)]
pub(super) struct Environment {
  save: Save,
  text: Signal<String>,
  stored: Signal<Option<Stored>>,
  asking: Signal<Asking>,
  revision: Signal<u64>,
  reloading: Signal<bool>,
}
impl Component for Environment {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      save: action(ctx),
      text: ctx.signal(String::new()),
      stored: ctx.signal(None),
      asking: ctx.signal(Asking(None)),
      revision: ctx.signal(0),
      reloading: ctx.signal(false),
    };
    let save = tab.save.clone();
    let (after, status, feed) = (
      tab.clone(),
      ctx.props::<TabProps>().status.clone(),
      ctx.props::<TabProps>().project.feed.clone(),
    );
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(Status::Saving);
        return;
      }
      if state.error.is_some() {
        status.set(Status::Failed);
      } else if let Some(stored) = &state.data {
        after.stored.set(Some(stored.clone()));
        after.asking.set(Asking(None));
        status.set(Status::Saved);
        after.revision.update(|r| *r += 1);
        feed.saved();
      }
    });
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let save = self.save.clone();
    let state = self.read(ctx, &props);
    self.finish_reload(&state, &save);
    if self.stored.get_untracked().is_none() && state.is_pending() {
      return page::scroll(self.pending(ctx, &state));
    }
    let Some(read) = state.data.clone().or_else(|| self.stored.get()) else {
      return page::scroll(self.pending(ctx, &state));
    };
    if self.stored.get_untracked().is_none() {
      self.fill(read.clone());
    }
    let changed = self
      .stored
      .get()
      .is_some_and(|s| parse_environment(&self.text.get()).as_ref() != Ok(&s.0));
    let asking = self.asking.get().0;
    let outside =
      !state.is_pending() && self.stored.get().is_some_and(|s| s != read) && !save.state().get().is_pending();
    if outside && !changed && asking.is_none() {
      self.fill(read);
    }
    if !changed
      && asking.is_none()
      && !self.reloading.get()
      && !save.state().get().is_pending()
      && save.state().get().error.is_some()
    {
      save.state().set(FutureState::idle());
    }
    if self.reloading.get() {
      props.status.set(Status::Saving);
    } else if state.error.is_some() {
      props.status.set(Status::Failed);
    } else if !save.state().get().is_pending() && save.state().get().error.is_none() {
      props.status.set(if parse_environment(&self.text.get()).is_err() {
        Status::Invalid
      } else if changed {
        Status::Unsaved
      } else {
        Status::Saved
      });
    }
    let fields = self.fields(ctx, &props, &save, asking.is_some());
    let mut content = Column::new()
      .spacing(crate::theme::ExtraSpacing::Roomy.role())
      .child(
        section(
          "settings-environment-section",
          &ctx.t("project_settings.environmentHeading"),
          None,
          vec![
            page::explanation(
              "settings-environment-description",
              &ctx.t("project_settings.environmentDetail"),
            ),
            fields,
          ],
        )
        .spacing(crate::theme::ExtraSpacing::Inset.role()),
      )
      .with_children(
        save
          .state()
          .get()
          .error
          .map(|e| field_error("settings-environment-save-error", &message_text(ctx, &e))),
      )
      .with_children(match &state.error {
        Some(error) => Some(self.read_error(ctx, &save, error)),
        None => (self.reloading.get() || (outside && changed)).then(|| self.outside(ctx, &save)),
      })
      .child(page::review_notice(ctx, true));
    if let Some(request) = asking {
      content = content.child(self.question(ctx, &props, &save, request));
    }
    page::scroll(content)
  }
}
impl Environment {
  fn finish_reload(&self, state: &FutureState<Stored, Message>, save: &Save) {
    if !self.reloading.get() || state.is_pending() {
      return;
    }
    self.reloading.set(false);
    if state.error.is_none()
      && let Some(read) = &state.data
    {
      self.fill(read.clone());
      self.asking.set(Asking(None));
      save.state().set(FutureState::idle());
    }
  }
  fn read(&self, ctx: &mut Ctx, props: &TabProps) -> FutureState<Stored, Message> {
    let revision = self.revision.get() + props.project.feed.outside.get().of(Outside::ProjectSettings);
    let mut state = ctx
      .future(
        (props.project.open.repo_root.clone(), revision),
        |(root, revision)| async move {
          background::run(move || load_in(&root).map(|environment| Stored(environment, revision)))
            .await?
            .map_err(|e| messages::store_error(&e))
        },
      )
      .state()
      .get();
    if state.data.as_ref().is_some_and(|read| read.1 != revision) {
      state.data = None;
    }
    state
  }
  fn pending(&self, ctx: &Ctx, state: &FutureState<Stored, Message>) -> Element {
    match &state.error {
      Some(e) => {
        let revision = self.revision.clone();
        load_error(
          ctx,
          "settings-environment-load-error",
          (&ctx.t("project_settings.loadFailed"), &message_text(ctx, e)),
          move || revision.update(|r| *r += 1),
        )
      }
      None => loading("settings-loading", &ctx.t("project_settings.reading")).into(),
    }
  }
  fn fields(&self, ctx: &mut Ctx, props: &TabProps, save: &Save, asking: bool) -> Element {
    let commit = self.commit(props, save);
    let mut input = FieldInput::new(
      "settings-environment-variables",
      ctx.t("project_settings.variables"),
      self.text.clone(),
    )
    .text_area()
    .area_height(ENVIRONMENT_HEIGHT)
    .mono()
    .text_style(crate::theme::ExtraTypography::MonoArea.role())
    .on_leave(move || commit());
    if asking || self.reloading.get() || save.state().get().is_pending() {
      input = input.locked();
    }
    if let Err(error) = parse_environment(&self.text.get()) {
      input = input.error(message_text(ctx, &error));
    }
    let variables = input.render(ctx);
    let count = parse_environment(&self.text.get())
      .map(|v| v.iter().count())
      .unwrap_or(0);
    Column::new()
      .spacing(crate::theme::ExtraSpacing::Snug.role())
      .child(field(
        "settings-environment",
        Some(&ctx.t("project_settings.variables")),
        variables,
        None,
      ))
      .child(page::icon_note(
        "settings-variable-count",
        Icon::List,
        &ctx.t_args("project_settings.variableCount", [("count", count.to_string())]),
      ))
      .child(helper(ctx, props))
      .into()
  }
  fn fill(&self, stored: Stored) {
    self.text.set(environment_text(&stored.0));
    self.stored.set(Some(stored));
  }
  fn commit(&self, props: &TabProps, save: &Save) -> Arc<dyn Fn() + Send + Sync> {
    let (tab, save, root) = (self.clone(), save.clone(), props.project.open.repo_root.clone());
    Arc::new(move || {
      if tab.reloading.get_untracked()
        || save.state().get_untracked().is_pending()
        || tab.asking.get_untracked().0.is_some()
      {
        return;
      }
      let (Some(base), Ok(asked)) = (tab.stored.get_untracked(), parse_environment(&tab.text.get_untracked())) else {
        return;
      };
      if base.0 == asked {
        return;
      }
      let request = Requested {
        base: base.0,
        asked,
        confirmed: false,
      };
      if environment::widenings(&request.base, &request.asked).is_empty() {
        save.run((root.clone(), request));
      } else {
        tab.asking.set(Asking(Some(request)));
      }
    })
  }
  fn question(&self, ctx: &mut Ctx, props: &TabProps, save: &Save, mut request: Requested) -> Element {
    let reasons = environment::widenings(&request.base, &request.asked)
      .iter()
      .map(|m| message_text(ctx, m))
      .collect::<Vec<_>>();
    request.confirmed = true;
    let (save, root, asking, reloading) = (
      save.clone(),
      props.project.open.repo_root.clone(),
      self.asking.clone(),
      self.reloading.clone(),
    );
    ctx.mount_keyed::<WidenConfirm>(
      "settings-environment-confirm",
      WidenProps {
        id: "settings-environment-question",
        card: false,
        automation: crate::mcp::input_gate::Guarded::PermissionWidening,
        title: ctx.t("project_settings.confirmEnvironment"),
        reasons: reasons.into(),
        keep: ctx.t("project_settings.keep"),
        confirm: ctx.t("project_settings.confirm"),
        on_keep: Arc::new(move || asking.set(Asking(None))),
        on_confirm: Arc::new(move || {
          if !reloading.get_untracked() && !save.state().get_untracked().is_pending() {
            save.run((root.clone(), request.clone()));
          }
        }),
      },
    )
  }
  fn read_error(&self, ctx: &Ctx, save: &Save, error: &Message) -> Element {
    Notice::new(
      "settings-environment-read-error",
      NoticeTone::Danger,
      ctx.t("project_settings.loadFailed"),
    )
    .body(message_text(ctx, error))
    .action(self.reload_button(ctx, save, "project_settings.retry"))
    .into()
  }
  fn outside(&self, ctx: &Ctx, save: &Save) -> Element {
    let title = if self.reloading.get() {
      "project_settings.reading"
    } else {
      "project_settings.outsideTitle"
    };
    Notice::new("settings-environment-outside", NoticeTone::Warning, ctx.t(title))
      .body(ctx.t("project_settings.outsideDetail"))
      .action(self.reload_button(ctx, save, "project_settings.reload"))
      .into()
  }
  fn reload_button(&self, ctx: &Ctx, save: &Save, label: &str) -> ActionButton {
    let (tab, save) = (self.clone(), save.clone());
    let button = ActionButton::new("settings-environment-reload", ButtonKind::Secondary, ctx.t(label));
    if self.reloading.get() {
      return button.loading();
    }
    if save.state().get().is_pending() {
      return button.disabled();
    }
    button.on_click(move || {
      if tab.reloading.get_untracked() || save.state().get_untracked().is_pending() {
        return;
      }
      tab.reloading.set(true);
      tab.revision.update(|revision| *revision += 1);
    })
  }
}
fn helper(ctx: &mut Ctx, props: &TabProps) -> Element {
  let nav = crate::navigation::Navigation::of(ctx);
  let project = props.project.open.project.id();
  Row::new()
    .id("settings-environment-helper")
    .width(Dimension::Pct(100.0))
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(crate::ui::icon::icon(
      Icon::Eye,
      crate::theme::IconSize::Sm,
      lurq::app::theme::PaletteColor::TextMuted,
    ))
    .child(
      Text::new(&ctx.t("project_settings.plainText"))
        .id("settings-environment-plaintext")
        .variant(crate::theme::ExtraTypography::Note.role())
        .flex(1.0),
    )
    .child(
      ActionButton::new(
        "settings-open-secrets",
        ButtonKind::Ghost,
        ctx.t("project_settings.openSecrets"),
      )
      .icon(Icon::KeyRound)
      .compact()
      .in_form()
      .on_click(move || {
        nav.go(crate::routes::Route::Project(
          project,
          crate::routes::ProjectPage::Secrets,
        ))
      }),
    )
    .into()
}

fn action(ctx: &mut Ctx) -> Save {
  let changes = crate::services::Services::of(ctx).changes;
  ctx.future_action(move |(root, request): (PathBuf, Requested)| {
    let changes = changes.clone();
    async move {
      background::run(move || {
        save_checked_in(&root, &request.base, &request.asked, request.confirmed)?;
        let db = orchester_store::ProjectDb::open(&root).map_err(|e| messages::store_error(&e))?;
        changes.record(db.project().id(), Outside::ProjectSettings);
        Ok(Stored(request.asked, u64::MAX))
      })
      .await?
    }
  })
}
