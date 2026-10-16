//! The first link page's parts: what it does in a line, what goes up or comes down (`Panel · What is uploaded` in
//! `KSnOW`), and *Discovery file* with whether to write `orchester.toml` (`Section · Discovery file`).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::project::TaskKey;
use orchester_workspace::sharing::project::{discovery::DiscoveryState, first_link::LinkKind};

use super::{super::confirm, DISCOVERY, Props, Upload};
use crate::{
  project::ShownProject,
  screens::{
    project_settings::page::{self, CONTROL_WIDTH},
    settings::orchester::{PersonOnly, shown_address},
  },
  ui::{
    form_section::{Label, Place, section, side_row},
    toggle::{Toggle, ToggleKind},
  },
};

/// What linking does, under the title.
pub(super) fn lead(ctx: &Ctx, props: &Props) -> String {
  let plan = &props.upload.plan;
  let names = [
    ("local", props.project.open.project.name().as_str().to_owned()),
    ("project", plan.project.name.clone()),
    ("server", props.upload.entry.stored.name.clone()),
  ];
  match &plan.kind {
    LinkKind::Upload { tasks, .. } if tasks.is_empty() => ctx.t_args("sharing.upload.leadEmpty", names),
    LinkKind::Upload { tasks, .. } => {
      let [local, project, server] = names;
      ctx.t_args(
        "sharing.upload.lead",
        [local, project, server, ("count", tasks.len().to_string())],
      )
    }
    LinkKind::Download { tasks } | LinkKind::Review { server: tasks, .. } => {
      let [local, project, server] = names;
      ctx.t_args(
        "sharing.upload.leadDownload",
        [local, project, server, ("count", tasks.to_string())],
      )
    }
  }
  .to_string()
}

/// *What is uploaded*, or *What comes down* for a checkout without tasks.
pub(super) fn what_goes(ctx: &Ctx, project: &ShownProject, upload: &Upload) -> Element {
  let plan = &upload.plan;
  let server_key = plan.project.key.as_str();
  let key = |id: &str, name: &str| (id.to_owned(), ctx.t(name).to_string());
  let mut pairs = vec![row(
    key("sharing-upload-project", "sharing.upload.project"),
    &ctx.t_args(
      "sharing.upload.projectValue",
      [
        ("project", plan.project.name.as_str()),
        ("server", upload.entry.stored.name.as_str()),
        ("key", server_key),
        ("address", shown_address(&plan.project.address)),
      ],
    ),
  )];
  let local_key = project.open.project.key().as_str();
  let (title, rest) = match &plan.kind {
    LinkKind::Upload { tasks, next, claimed } => (
      "sharing.upload.whatGoesUp",
      upload_rows(ctx, (tasks, next, claimed), (local_key, server_key)),
    ),
    LinkKind::Download { tasks } | LinkKind::Review { server: tasks, .. } => (
      "sharing.upload.whatComesDown",
      vec![
        row(
          key("sharing-upload-tasks", "sharing.upload.serverTasks"),
          &ctx.t_args(
            "sharing.upload.serverTasksValue",
            [
              ("count", tasks.to_string()),
              ("server", upload.entry.stored.name.clone()),
            ],
          ),
        ),
        row(
          key("sharing-upload-yours", "sharing.upload.yourTasks"),
          &ctx.t("sharing.upload.noneHere"),
        ),
      ],
    ),
  };
  pairs.extend(rest);
  confirm::panel("sharing-upload-what", &ctx.t(title), pairs)
}

fn row((id, key): (String, String), value: &str) -> lurq::components::Row {
  confirm::pair(&id, (&key, confirm::PANEL_KEY), value)
}

/// The rows of an upload that keeps the numbers: the tasks, a new key when the project's differs, where new tasks
/// continue, and the claims.
fn upload_rows(
  ctx: &Ctx,
  (tasks, next, claimed): (&[TaskKey], &TaskKey, &[TaskKey]),
  (local_key, server_key): (&str, &str),
) -> Vec<lurq::components::Row> {
  let key = |id: &str, name: &str| (id.to_owned(), ctx.t(name).to_string());
  let shown = |task: &TaskKey| format!("{server_key}-{}", task.number());
  let uploaded = match (tasks.first(), tasks.last()) {
    (Some(first), Some(last)) if tasks.len() > 1 => ctx.t_args(
      "sharing.upload.tasksValue",
      [
        ("count", tasks.len().to_string()),
        ("first", shown(first)),
        ("last", shown(last)),
      ],
    ),
    (Some(only), _) => ctx.t_args("sharing.upload.taskValue", [("key", shown(only))]),
    _ => ctx.t("sharing.upload.noTasks"),
  };
  let mut rows = vec![row(key("sharing-upload-tasks", "sharing.upload.tasks"), &uploaded)];
  if local_key != server_key {
    rows.push(row(
      key("sharing-upload-key", "sharing.upload.key"),
      &ctx.t_args(
        "sharing.upload.keyValue",
        [("local", local_key.to_owned()), ("key", server_key.to_owned())],
      ),
    ));
  }
  rows.push(row(
    key("sharing-upload-next", "sharing.upload.next"),
    &ctx.t_args("sharing.upload.nextValue", [("key", shown(next))]),
  ));
  let claims = if claimed.is_empty() {
    ctx.t("sharing.upload.noClaims")
  } else {
    let separator = ctx.t("sharing.upload.separator");
    let keys: Vec<String> = claimed.iter().map(shown).collect();
    ctx.t_args("sharing.upload.claimsValue", [("keys", keys.join(&separator))])
  };
  rows.push(row(key("sharing-upload-claimed", "sharing.upload.claimed"), &claims));
  rows
}

/// *Discovery file* with *Write orchester.toml into the repository*: a switch while the repository has none, which only
/// a person turns (the file puts an address into the repository that colleagues trust), else what the file there says,
/// which Orchester leaves as it is.
pub(super) fn discovery(ctx: &mut Ctx, upload: &Upload, (write, person): (&Signal<bool>, &PersonOnly)) -> Element {
  let address = upload.plan.project.address.clone();
  let label = Label::new("sharing-upload-discovery", ctx.t("sharing.upload.discovery.write"))
    .description(ctx.t_args("sharing.upload.discovery.description", [("address", address.as_str())]));
  let control: Element = match upload.plan.discovery {
    DiscoveryState::Absent => {
      let on = write.get();
      let (set, allows) = (write.clone(), person.clone());
      let words = ctx.t(if on {
        "sharing.upload.discovery.on"
      } else {
        "sharing.upload.discovery.off"
      });
      Column::new()
        .width(Dimension::Px(CONTROL_WIDTH))
        .child(
          // Room after *On*, so the focus ring, drawn inside the control's edge, keeps clear of it.
          Toggle::new(DISCOVERY, ToggleKind::Switch, on, words)
            .ring_room()
            .on_change(move |on| {
              if allows.allows(DISCOVERY) {
                set.set(on);
              }
            }),
        )
        .with_children(person.reason(ctx, DISCOVERY))
        .into()
    }
    DiscoveryState::Names => page::note(
      "sharing-upload-discovery-names",
      &ctx.t("sharing.upload.discovery.names"),
    ),
    DiscoveryState::Other => page::note(
      "sharing-upload-discovery-other",
      &ctx.t("sharing.upload.discovery.other"),
    ),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(section(
      "sharing-upload-discovery-section",
      &ctx.t("sharing.upload.discovery.section"),
      None,
      vec![side_row(label, control, Place::Last, true)],
    ))
    .into()
}
