//! A task of a project that joined a server project, on its page. One kept *Not shared* (`R4elt`) says so over its
//! Summary, read-only, with *Share task* in the header, which asks in place ([`question`], `TfVtK`); beside it, *On
//! this computer only*: its key, when it was left out, the key it would become, the server's similar task and the
//! server. One that went up with another key (`VuLYC`) shows *was KON-12* after its title and *On Kontur team* beside
//! its Summary: its key now and before joining, when it was shared, who created it and changed it last, and why texts
//! still say the old key.

pub mod question;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{project::TaskKey, time::Timestamp};
use orchester_workspace::{
  background,
  sharing::{
    Servers,
    project::{ProjectServer, share_task::KeptPreview, state::ProjectSharing},
  },
};

use super::TaskContext;
use crate::{
  screens::settings::orchester::{Said, day_of, moment, sharing_said, shown_address},
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    detail_row::{DetailValue, detail_row},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

/// What the page reads of the project's link for a joined or *Not shared* task.
#[derive(Clone, Debug, PartialEq)]
pub struct Linked {
  pub sharing: ProjectSharing,
  /// For a *Not shared* task: when it was left out and the server's similar task.
  pub kept: Option<KeptPreview>,
}

impl lurq::app::component::DevtoolsInspectable for Linked {}

/// The key a *Not shared* task would get now, as its server says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextKey(pub Option<TaskKey>);

impl lurq::app::component::DevtoolsInspectable for NextKey {}

/// Whether the task's page says anything of its project's link: kept *Not shared*, or uploaded with another key.
pub fn shown(task: &TaskContext) -> bool {
  task.detail.not_shared() || former_key(task).is_some()
}

/// The key the task had before its project joined, while it is the server's.
pub fn former_key(task: &TaskContext) -> Option<&str> {
  task
    .detail
    .shared
    .as_ref()
    .filter(|_| !task.detail.not_shared())
    .and_then(|info| info.former_key.as_deref())
}

/// The project's link as read for `task`; `None` until read, or for a task the link says nothing of.
pub fn linked(ctx: &mut Ctx, task: &TaskContext) -> Option<Linked> {
  if !shown(task) {
    return None;
  }
  let (servers, root, id, kept) = (
    Services::of(ctx).servers,
    task.project.open.repo_root.clone(),
    task.detail.task.id(),
    task.detail.not_shared(),
  );
  ctx
    .future((root, id, kept, servers), |(root, id, kept, servers)| async move {
      background::run(move || -> Result<Linked, Said> {
        let sharing = servers
          .project_sharing(&root)
          .map_err(|error| sharing_said(&error))?
          .ok_or_else(|| Said::new("sharing.linked.notLinked"))?;
        let kept = kept
          .then(|| orchester_workspace::sharing::project::share_task::kept_preview(&root, id))
          .transpose()
          .map_err(|_| Said::new("cloud.errors.local"))?;
        Ok(Linked { sharing, kept })
      })
      .await
      .map_err(|_| Said::new("cloud.errors.local"))?
    })
    .state()
    .get()
    .data
}

/// The key *Share task* would give the *Not shared* task now; `None` until its server answered, and when it can't.
pub fn next_key(ctx: &mut Ctx, task: &TaskContext) -> Option<TaskKey> {
  if !task.detail.not_shared() {
    return None;
  }
  let (servers, root) = (Services::of(ctx).servers, task.project.open.repo_root.clone());
  let read = ctx
    .future(
      (root, servers, task.project.feed.revision.get()),
      |(root, servers, _)| async move { Ok::<_, ()>(NextKey(ask_next(servers, root).await)) },
    )
    .state()
    .get();
  read.data.and_then(|next| next.0)
}

async fn ask_next(servers: Servers, root: std::path::PathBuf) -> Option<TaskKey> {
  let server = match background::run(move || ProjectServer::open(&servers, &root)).await {
    Ok(Ok(Some(server))) => server,
    Ok(Ok(None)) | Err(_) => return None,
    Ok(Err(error)) => {
      tracing::info!(%error, "the next key of a shared project can't be read");
      return None;
    }
  };
  match server.next_key().await {
    Ok(key) => Some(key),
    Err(error) => {
      tracing::info!(%error, "the next key of a shared project can't be read");
      None
    }
  }
}

/// *Not shared · read-only* over the Summary of a task kept *Not shared* (`Notice` in `R4elt`).
pub fn notice(ctx: &Ctx, task: &TaskContext, linked: Option<&Linked>) -> Element {
  let key = task.detail.task.key().to_string();
  let names = names(task, linked);
  Notice::new("task-not-shared", NoticeTone::Info, ctx.t("task.notShared.title"))
    .icon(Icon::Archive)
    .body(ctx.t_args(
      "task.notShared.body",
      [
        ("task", key.clone()),
        ("project", names.project.clone()),
        ("key", names.key.clone()),
        ("day", names.day(ctx)),
        ("server", names.server),
      ],
    ))
    .part(
      Row::new()
        .id("task-not-shared-locked")
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t_args("task.notShared.locked", [("task", key)])).variant(ExtraTypography::Note.role())),
    )
    .into()
}

/// The names the words of a joined task use: the project, the server project's key, the server, and when it joined.
struct Names {
  project: String,
  key: String,
  server: String,
  joined: Option<Timestamp>,
}

impl Names {
  fn day(&self, ctx: &Ctx) -> String {
    self.joined.map(|at| day_of(ctx, at)).unwrap_or_default()
  }
}

fn names(task: &TaskContext, linked: Option<&Linked>) -> Names {
  let project = task.project.open.project.name().as_str().to_owned();
  match linked {
    Some(linked) => Names {
      project,
      key: linked.sharing.link.key.as_str().to_owned(),
      server: linked
        .sharing
        .server
        .as_ref()
        .map(|(name, _)| name.clone())
        .unwrap_or_default(),
      joined: Some(linked.sharing.link.joined_at),
    },
    None => Names {
      project,
      key: task.project.open.project.key().as_str().to_owned(),
      server: String::new(),
      joined: None,
    },
  }
}

/// The panel's first section: *On this computer only* for a *Not shared* task, *On Kontur team* for one that went up
/// with another key.
pub fn section(ctx: &Ctx, task: &TaskContext, (linked, next): (Option<&Linked>, Option<&TaskKey>)) -> Option<Element> {
  let linked = linked?;
  let names = names(task, Some(linked));
  if task.detail.not_shared() {
    return Some(kept_section(ctx, task, (linked, &names), next));
  }
  let former = former_key(task)?;
  Some(joined_section(ctx, task, &names, former))
}

fn kept_section(ctx: &Ctx, task: &TaskContext, (linked, names): (&Linked, &Names), next: Option<&TaskKey>) -> Element {
  let key = task.detail.task.key().to_string();
  let would = next.map_or_else(
    || ctx.t_args("task.notShared.wouldNext", [("key", names.key.as_str())]),
    |next| ctx.t_args("task.notShared.wouldValue", [("key", next.to_string())]),
  );
  let mut rows = vec![
    row(
      ctx,
      "kept-key",
      (
        "task.notShared.key",
        &ctx.t_args("task.notShared.keyValue", [("key", key)]),
      ),
      DetailValue::Mono,
    ),
    row(
      ctx,
      "kept-left-out",
      (
        "task.notShared.leftOut",
        &ctx.t_args(
          "task.notShared.leftOutValue",
          [
            ("day", names.day(ctx)),
            ("project", names.project.clone()),
            ("key", names.key.clone()),
          ],
        ),
      ),
      DetailValue::Text,
    ),
    row(ctx, "kept-would", ("task.notShared.would", &would), DetailValue::Text),
  ];
  if let Some(similar) = linked.kept.as_ref().and_then(|kept| kept.similar.as_ref()) {
    let label = ctx.t_args("task.notShared.similar", [("key", names.key.as_str())]);
    let value = ctx.t_args(
      "task.notShared.similarValue",
      [
        ("key", similar.key.as_str()),
        ("title", similar.title.as_str()),
        ("by", similar.created_by.as_str()),
      ],
    );
    rows.push(detail_row("task-sharing-kept-similar", &label, &value, DetailValue::Text).into());
  }
  rows.push(server_row(ctx, linked, names));
  block("task-sharing-kept", (&ctx.t("task.notShared.panel"), None), rows)
}

fn joined_section(ctx: &Ctx, task: &TaskContext, names: &Names, former: &str) -> Element {
  let info = task.detail.shared.as_ref();
  let created = ctx.t_args(
    "task.joined.createdValue",
    [
      ("by", info.and_then(|info| info.created_by.clone()).unwrap_or_default()),
      ("day", day_of(ctx, task.detail.task.created_at())),
      ("former", former.to_owned()),
    ],
  );
  let mut rows = vec![
    row(
      ctx,
      "joined-key",
      ("task.joined.key", &task.detail.task.key().to_string()),
      DetailValue::Mono,
    ),
    row(ctx, "joined-before", ("task.joined.before", former), DetailValue::Mono),
    row(
      ctx,
      "joined-shared",
      (
        "task.joined.shared",
        &ctx.t_args(
          "task.joined.sharedValue",
          [
            ("day", names.day(ctx)),
            ("project", names.project.clone()),
            ("key", names.key.clone()),
          ],
        ),
      ),
      DetailValue::Text,
    ),
    row(
      ctx,
      "joined-created",
      ("task.joined.created", &created),
      DetailValue::Text,
    ),
  ];
  if let Some((by, at)) = info.and_then(|info| Some((info.updated_by.clone()?, info.updated_at?))) {
    let value = ctx.t_args("task.joined.changedValue", [("by", by), ("time", moment(ctx, at))]);
    rows.push(row(
      ctx,
      "joined-changed",
      ("task.joined.changed", &value),
      DetailValue::Text,
    ));
  }
  rows.push(
    Row::new()
      .id("task-sharing-joined-note")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Xs))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Start)
      .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t_args("task.joined.note", [("former", former)]))
          .variant(ExtraTypography::Note.role())
          .flex(1.0),
      )
      .into(),
  );
  let title = ctx.t_args("task.joined.panel", [("server", names.server.as_str())]);
  block("task-sharing-joined", (&title, Some(Icon::Cloud)), rows)
}

fn server_row(ctx: &Ctx, linked: &Linked, names: &Names) -> Element {
  let address = linked
    .kept
    .as_ref()
    .and_then(|kept| kept.address.as_deref())
    .map(shown_address)
    .unwrap_or_default();
  let value = ctx.t_args(
    "task.notShared.serverValue",
    [("server", names.server.as_str()), ("address", address)],
  );
  row(ctx, "kept-server", ("task.notShared.server", &value), DetailValue::Text)
}

fn row(ctx: &Ctx, id: &str, (key, value): (&str, &str), kind: DetailValue) -> Element {
  detail_row(&format!("task-sharing-{id}"), &ctx.t(key), value, kind).into()
}

/// A section of the panel: its title, with an icon, over its rows, a line under it.
fn block(id: &str, (title, glyph): (&Arc<str>, Option<Icon>), rows: Vec<Element>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().bottom(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Tight.role())
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .padding(Padding::new().bottom(ExtraSpacing::Snug.role()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(glyph.map(|glyph| icon(glyph, IconSize::Base, PaletteColor::TextSecondary)))
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(TaskText::SectionTitle.role()),
        ),
    )
    .with_children(rows)
    .into()
}
