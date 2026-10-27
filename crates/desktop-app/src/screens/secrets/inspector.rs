//! A secret's page (`06b` `famwr`): the crumb back to Secrets; its head with its kind's tile, its name, its
//! environment, how many use it and when its value was set, Replace value and the … menu (Copy name, Archive);
//! *Replace value* asked in place under the head (`form`); *Details* (project, environment and when it was last
//! rotated), *Used by* (each tool and MCP server, as links, with the variable a server is given it in), *Recent access*
//! (which Orchester does not record yet, and says so), and *Archive …?* asked in place under them (`Archive ·
//! question` `fw0eT`), brought into view. An archived secret says so, and offers neither.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::secret::Secret;
use orchester_workspace::{
  activity::operations,
  secrets::data::{ProjectSecrets, SecretUseKind},
  tasks::outside::Outside,
};
use parking_lot::Mutex;

use super::{form, state::View, words, work};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route, SecretsPanel},
  screens::project_settings::kit::{EmptyRow, Group, SettingRow, back, entity_head, question, value},
  services::Services,
  theme::{ExtraSpacing, IconSize, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    message::message_text,
    unavailable::reason_line,
  },
};

/// The … menu's width.
const MENU_WIDTH: f32 = 200.0;

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub listed: ProjectSecrets,
  pub secret: Secret,
  pub view: View,
  /// Replace value or Archive asked on the page, or neither.
  pub panel: SecretsPanel,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.listed == other.listed
      && self.secret == other.secret
      && self.panel == other.panel
  }
}
impl DevtoolsInspectable for Props {}
pub(super) struct Inspector {
  action: work::Action,
  /// The … menu and its button.
  more: (Signal<bool>, ElementRef),
  /// *Archive …?*, brought into view when it is asked, and what the page asked when it last rendered.
  question: (ElementRef, Mutex<SecretsPanel>),
}
impl Component for Inspector {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let action = work::action(ctx);
    let view = ctx.props::<Props>().view.clone();
    let changes = Services::of(ctx).changes;
    let project = ctx.props::<Props>().project.open.project.id();
    ctx.watch(&action.state(), move |state| {
      if state.is_fulfilled() {
        view.settled();
        if matches!(state.data, Some(work::Outcome::Saved(_))) {
          // Every screen that shows the project's secrets and connections reads them again, this one too.
          changes.record(project, Outside::Connections);
        } else {
          view.revision.update(|revision| *revision += 1);
        }
      }
    });
    Self {
      action,
      more: (ctx.signal(false), ctx.element_ref()),
      question: (ctx.element_ref(), Mutex::new(SecretsPanel::default())),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let crumb = back(
      "secret-crumb",
      (&ctx.t("project_settings.secrets"), None),
      props.view.list(),
    );
    let head = self.head(ctx, &props);
    let replacing = (props.panel == SecretsPanel::Replace && !props.secret.is_archived()).then(|| {
      ctx.mount_keyed::<form::SecretForm>(
        &format!("secret-replace-{}", props.secret.id()),
        form::Props {
          project: props.project.clone(),
          listed: props.listed.clone(),
          secret: Some(props.secret.clone()),
          view: props.view.clone(),
        },
      )
    });
    let asked = std::mem::replace(&mut *self.question.1.lock(), props.panel);
    let archive = (props.panel == SecretsPanel::Archive && !props.secret.is_archived()).then(|| {
      // Asked just now: its buttons are brought into view, wherever the page was scrolled.
      if asked != SecretsPanel::Archive {
        ctx.scroll_into_view(&self.question.0);
      }
      Column::new()
        .width(Dimension::Pct(100.0))
        .ref_element(self.question.0.clone())
        .child(self.archive(ctx, &props))
    });
    Column::new()
      .id("secret-inspector")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb)
      .child(head)
      .with_children(replacing)
      .child(details(ctx, &props.project, &props.secret))
      .child(uses(ctx, &props.project, &props.listed, &props.secret))
      .child(recent(ctx))
      .with_children(archive)
  }
}
impl Inspector {
  /// The head: the kind's tile, the name over what it is, Replace value and the … menu.
  fn head(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let secret = &props.secret;
    let counts = props.listed.use_counts(secret.id());
    let set = words::date(ctx, secret.rotated_at().unwrap_or(secret.created_at()));
    let meta = if secret.is_archived() {
      ctx.t_args(
        "secrets.metaArchived",
        [("environment", ctx.t(words::environment_key(secret.environment())))],
      )
    } else {
      ctx.t_args(
        "secrets.meta",
        [
          ("environment", ctx.t(words::environment_key(secret.environment()))),
          ("uses", words::uses(ctx, counts)),
          ("date", set),
        ],
      )
    };
    let title = Text::new(secret.name().as_str())
      .id("secret-inspector-name")
      .variant(ProjectSettingsText::PageTitle.role());
    let meta = Text::new(&meta)
      .id("secret-inspector-meta")
      .variant(ProjectSettingsText::Hint.role());
    let mut actions = Vec::new();
    if !secret.is_archived() {
      let (view, id) = (props.view.clone(), secret.id());
      let pending = self.action.state().get().is_pending();
      let replace =
        ActionButton::new("secret-replace", ButtonKind::Secondary, ctx.t("secrets.replace")).icon(Icon::RefreshCw);
      let replace = if pending || props.panel == SecretsPanel::Replace {
        replace.disabled()
      } else {
        replace.on_click(move || view.open(SecretsPanel::Replace, Some(id)))
      };
      actions.push(replace.into());
      actions.push(self.more(ctx, props, pending));
    }
    entity_head(
      ctx,
      "secret-inspector-header",
      (words::glyph(secret.kind()).into(), title.into()),
      meta.into(),
      actions,
    )
  }

  /// The … menu: Copy name, and Archive, asked in place.
  fn more(&self, ctx: &mut Ctx, props: &Props, pending: bool) -> Element {
    let (open, anchor) = self.more.clone();
    let toggle = open.clone();
    let button = IconButton::new("secret-more", Icon::Ellipsis, ctx.t("secrets.more"))
      .element(anchor.clone())
      .on_click(move || toggle.update(|open| *open = !*open))
      .render(ctx);
    let (copier, name) = (
      crate::ui::code_box::Copier::of(ctx),
      props.secret.name().as_str().to_owned(),
    );
    let copy = MenuItem::new("secret-copy-name", ctx.t("secrets.copyName"), move || {
      if !copier.copy(&name) {
        tracing::warn!("the clipboard did not take the secret's name");
      }
    })
    .icon(Icon::Copy);
    let archive = if pending {
      MenuItem::unavailable(
        "secret-archive",
        ctx.t("secrets.archive"),
        ctx.t("secrets.archiving.running"),
      )
    } else {
      let (view, id) = (props.view.clone(), props.secret.id());
      MenuItem::new("secret-archive", ctx.t("secrets.archive"), move || {
        view.open(SecretsPanel::Archive, Some(id));
      })
    }
    .icon(Icon::Archive);
    let panel = menu_panel("secret-more-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .child(copy.render(ctx, &open))
      .child(archive.render(ctx, &open));
    Row::new().child(button).child(menu_at_end(anchor, open, panel)).into()
  }

  /// *Archive …?* in place: what it does to what uses the secret, Archive and Keep it.
  fn archive(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let state = self.action.state().get();
    let title = ctx.t_args("secrets.archiving.title", [("name", props.secret.name().as_str())]);
    let impact = words::archive_impact(ctx, props.listed.use_counts(props.secret.id()));
    let explanation = ctx.t_args(
      "secrets.archiving.explanation",
      [("project", props.project.open.project.name().as_str())],
    );
    let go = ActionButton::new(
      "secret-archive-confirm",
      ButtonKind::Danger,
      ctx.t(if state.is_pending() {
        "secrets.archiving.running"
      } else {
        "secrets.archive"
      }),
    )
    .icon(Icon::Archive);
    let go = if state.is_pending() {
      go.loading()
    } else {
      let (action, open, id, vault) = (
        self.action.clone(),
        props.project.open.clone(),
        props.secret.id(),
        Services::of(ctx).vault,
      );
      let name = props.secret.name().as_str().to_owned();
      go.on_click(move || {
        if action.state().get().is_pending() {
          return;
        }
        action.run(work::Start {
          repo: open.repo_root.clone(),
          vault: vault.clone(),
          write: work::Write::Archive(id),
          value: None,
          spec: operations::secret_archive(&open, &name),
        });
      })
    };
    let view = props.view.clone();
    let keep = ActionButton::new("secret-archive-cancel", ButtonKind::Ghost, ctx.t("secrets.keepIt"))
      .on_click(move || view.close_panel());
    let keep = if state.is_pending() { keep.disabled() } else { keep };
    let mut asked = question(
      "secret-archive-question",
      &title,
      &[impact, explanation],
      vec![go.into(), keep.into()],
    );
    if let Some(error) = &state.error {
      asked = asked.child(field_error("secret-archive-error", &message_text(ctx, error)));
    }
    asked.into()
  }
}

/// *Details* (`NLrZH`): the project, the environment (its kind in the (i)) and when its value was last rotated (how
/// it is kept in the (i)), as `06b` draws them; an archived secret also says its value is gone.
fn details(ctx: &mut Ctx, project: &ShownProject, secret: &Secret) -> Element {
  let rotated = match secret.rotated_at() {
    Some(at) => words::date(ctx, at),
    None => {
      let added = words::date(ctx, secret.created_at());
      ctx.t_args("secrets.neverRotated", [("date", added)])
    }
  };
  let mut rows = vec![
    detail(
      ctx,
      "secret-project",
      ("secrets.project", project.open.project.name().as_str()),
    ),
    SettingRow::new("secret-environment", ctx.t("secrets.environment"))
      .info(ctx.t_args("secrets.kindInfo", [("kind", ctx.t(words::kind_key(secret.kind())))]))
      .control(value(
        "secret-environment-value",
        &ctx.t(words::environment_key(secret.environment())),
      ))
      .render(ctx),
  ];
  if secret.is_archived() {
    rows.push(
      SettingRow::new("secret-value", ctx.t("secrets.value"))
        .hint(ctx.t("secrets.archivedHint"))
        .control(value("secret-archived", &ctx.t("secrets.archived")))
        .render(ctx),
    );
  }
  rows.push(
    SettingRow::new("secret-rotated", ctx.t("secrets.rotated"))
      .info(ctx.t("secrets.valueHint"))
      .control(value("secret-rotated-value", &rotated))
      .render(ctx),
  );
  Group::new("secret-details", ctx.t("secrets.details")).rows(rows).into()
}

fn detail(ctx: &mut Ctx, id: &str, (key, shown): (&str, &str)) -> Element {
  SettingRow::new(id.to_owned(), ctx.t(key))
    .control(value(&format!("{id}-value"), shown))
    .render(ctx)
}

/// *Used by* (`WtpzX`): each tool and MCP server that takes the secret, opening it; or that none does.
pub(super) fn uses(ctx: &mut Ctx, project: &ShownProject, listed: &ProjectSecrets, secret: &Secret) -> Element {
  let counts = listed.use_counts(secret.id());
  let key = match (counts.tools, counts.mcp_servers) {
    (0, 0) => "secrets.uses.empty",
    (0, _) => "secrets.uses.serverBindings",
    (_, 0) => "secrets.uses.toolBindings",
    _ => "secrets.uses.bindings",
  };
  let note = ctx.t_args(key, [("project", project.open.project.name().as_str())]);
  let navigation = Navigation::of(ctx);
  let rows: Vec<Element> = listed
    .used_by(secret.id())
    .map(|used| {
      let (glyph, what, page) = match used.kind {
        SecretUseKind::Tool(_) => (Icon::Wrench, ctx.t("secrets.uses.toolKind"), ProjectPage::Tools),
        SecretUseKind::McpServer(id) => (
          Icon::Server,
          // The variable its program is given it in (`MCP server · variable FIGMA_API_KEY` in `06b`).
          used.variable.as_deref().map_or_else(
            || ctx.t("secrets.uses.serverKind"),
            |variable| ctx.t_args("secrets.serverVariable", [("variable", variable)]),
          ),
          ProjectPage::Connections(ConnectionsView::of(id)),
        ),
      };
      SettingRow::new(format!("secret-use-{:?}", used.kind), used.name.clone())
        .item()
        .lead(icon(glyph, IconSize::Md, PaletteColor::TextMuted))
        .info(note.clone())
        .control(value(&format!("secret-use-{:?}-kind", used.kind), &what))
        .control(icon(Icon::ArrowUpRight, IconSize::Md, PaletteColor::TextMuted))
        .opens(navigation.to(Route::Project(project.open.project.id(), page)))
        .render(ctx)
    })
    .collect();
  let group = Group::new("secret-uses", ctx.t("secrets.usedBy"));
  let group = if rows.is_empty() {
    group.row(EmptyRow::new(
      "secret-use-none",
      Icon::Unlink,
      ctx.t("secrets.uses.noneTitle"),
      note,
    ))
  } else {
    group.rows(rows)
  };
  group.into()
}

/// *Recent access* (`O5DCl`): Orchester does not record which run read a secret yet, so the group says it is
/// unavailable, and why, instead of listing rows.
fn recent(ctx: &Ctx) -> Element {
  Group::new("secret-recent", ctx.t("secrets.recent"))
    .after(reason_line("secret-recent-unavailable", &ctx.t("secrets.recentHint")))
    .into()
}
