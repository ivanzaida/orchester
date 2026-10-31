//! *Clone it here* on *Link a folder* (06 `go2aM`, 07 `phoYc`, Row 3b; ORC-584): the person picks where the checkout
//! goes and presses *Clone repository…*; Git clones in a console window they see and can sign in in
//! ([`orchester_workspace::sharing::project::clone`]). A clone that Git finished and that checks as the project's
//! checkout fills the page's folder field, whose check then shows it matches; nothing is linked until the person
//! presses the page's button. Opening a console is a person's: synthetic input is refused, and the panel says so.

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_api::SharedProject;
use orchester_exec::{clone_console::ConsoleWords, programs::Programs};
use orchester_workspace::{
  background,
  sharing::project::clone::{
    CloneEnd, CloneKey, CloneRefusal, CloneRequest, Clones, Destination, check_destination, plain_destination,
  },
};
use parking_lot::Mutex;

use super::{
  Chosen,
  clone_view::{self, Choose},
  typed_path,
};
use crate::{
  routes::JoinTarget,
  screens::settings::orchester::PersonOnly,
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
};

/// The panel for `project`, which `target` names; a clone fills `folder`, the page's folder field.
pub(super) fn clone_option(
  ctx: &mut Ctx,
  target: &JoinTarget,
  project: &SharedProject,
  folder: &Signal<String>,
) -> Element {
  let key = format!("join-team-link-clone-{}-{}", target.server, target.slug());
  ctx.mount_keyed::<CloneOption>(
    &key,
    Props {
      target: target.clone(),
      project: project.clone(),
      folder: folder.clone(),
    },
  )
}

/// The clones the context provides, a test's or a QA copy's stand-in, else the session's.
fn clones_of(ctx: &mut Ctx) -> Clones {
  ctx.use_context::<Clones>().unwrap_or_else(|| Services::of(ctx).clones)
}

#[derive(Clone)]
struct Props {
  target: JoinTarget,
  project: SharedProject,
  folder: Signal<String>,
}

/// The page's field is the same signal throughout; the project is compared by what the panel shows of it.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    let shown = |project: &SharedProject| (project.id, project.name.clone(), project.remote_hint.clone());
    self.target == other.target && shown(&self.project) == shown(&other.project)
  }
}

impl DevtoolsInspectable for Props {}

/// What the check found for the text in *Clone into*; nothing for an empty field.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
  text: String,
  destination: Option<Destination>,
}

impl DevtoolsInspectable for Place {}

/// What a press, or the page opened again, asks of the project's clone.
#[derive(Clone)]
enum Ask {
  Start(Box<CloneRequest>),
  /// The clone that runs or last ended, as the page was opened again.
  Follow(CloneKey),
}

struct CloneOption {
  into: Signal<String>,
  choose: Choose,
  clone: FutureAction<Ask, CloneEnd, CloneRefusal>,
  clones: Clones,
  person: PersonOnly,
  /// The page's folder field, as the last render's props named it: a clone that ends cloned fills it.
  folder: Arc<Mutex<Option<Signal<String>>>>,
  /// Whether the clone the page had before it was left is followed already.
  followed: Arc<AtomicBool>,
}

impl Component for CloneOption {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let picker = PathPicker::of(ctx);
    let choose = ctx.future_action(move |(title, name): (Arc<str>, String)| {
      let picker = picker.clone();
      async move {
        Ok::<_, ()>(Chosen(
          picker
            .pick(PathKind::Folder, title)
            .await
            .map(|parent| parent.join(name)),
        ))
      }
    });
    let into = ctx.signal(String::new());
    let typed = into.clone();
    ctx.watch(&choose.state(), move |state| {
      if let (false, Some(Chosen(Some(path)))) = (state.is_pending(), &state.data) {
        typed.set(path.display().to_string());
      }
    });
    let clones = clones_of(ctx);
    let programs = Services::of(ctx).programs;
    let runner = clones.clone();
    let clone = ctx.future_action(move |ask: Ask| {
      let (clones, programs) = (runner.clone(), programs.clone());
      async move { asked(&clones, programs, ask).await }
    });
    let folder: Arc<Mutex<Option<Signal<String>>>> = Arc::default();
    let field = folder.clone();
    ctx.watch(&clone.state(), move |state| {
      if let (false, Some(CloneEnd::Cloned { folder })) = (state.is_pending(), &state.data)
        && let Some(field) = field.lock().as_ref()
      {
        field.set(folder.display().to_string());
      }
    });
    Self {
      into,
      choose,
      clone,
      clones,
      person: PersonOnly::new(ctx),
      folder,
      followed: Arc::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      target,
      project,
      folder,
    } = ctx.props::<Props>().clone();
    *self.folder.lock() = Some(folder);
    let key: CloneKey = (target.server, target.slug().to_owned());
    if !self.followed.swap(true, Ordering::Relaxed) && self.clones.follow(&key).is_some() {
      self.clone.run(Ask::Follow(key));
    }
    let state = self.clone.state().get();
    let ended = (!state.is_pending()).then(|| (state.data.clone(), state.error.clone()));
    let text = self.into.get();
    // Asked for in every render, in the same place: lurq keeps a component's futures by the order it asks for them.
    let destination = self.destination(ctx, &text, ended.clone());
    let name = repository_name(&project);
    let mut parts = clone_view::into_row(ctx, (&self.into, &self.choose), (&project, &name), destination.as_ref());
    let start = destination
      .as_ref()
      .filter(|destination| destination.allows() && !state.is_pending())
      .map(|_| self.starter(ctx, &target, &project));
    let button = clone_view::start_button(ctx, state.is_pending(), start);
    parts.push(clone_view::actions(button, self.person.reason(ctx, clone_view::START)));
    if state.is_pending() {
      parts.push(clone_view::waiting(ctx));
    }
    match ended {
      Some((Some(end), _)) => parts.extend(clone_view::ended(ctx, &project, &end)),
      Some((None, Some(refusal))) => parts.extend(clone_view::refused(ctx, &refusal)),
      _ => {}
    }
    clone_view::panel(ctx, &project, parts)
  }
}

impl CloneOption {
  /// What the check found for `text`; `None` until it is known, so a result for an earlier text is never shown for
  /// this one. Checked again when a clone ends: what it left changes the answer.
  fn destination(
    &self,
    ctx: &mut Ctx,
    text: &str,
    ended: Option<(Option<CloneEnd>, Option<CloneRefusal>)>,
  ) -> Option<Destination> {
    let deps = (text.to_owned(), ended);
    let place = ctx
      .future(deps, |(text, _)| async move { Ok::<_, ()>(checked(text).await) })
      .state()
      .get()
      .data;
    place
      .filter(|place| place.text == text)
      .and_then(|place| place.destination)
  }

  /// Opens the console for the folder in *Clone into*, a person's press only.
  fn starter(&self, ctx: &Ctx, target: &JoinTarget, project: &SharedProject) -> impl Fn() + Send + Sync + 'static {
    let words = ConsoleWords::new(
      &ctx.t_args(
        "startup.shared.link.clone.console.title",
        [("project", project.name.as_str())],
      ),
      &ctx.t("startup.shared.link.clone.console.failed"),
      &ctx.t("startup.shared.link.clone.console.close"),
    );
    let (run, person, into) = (self.clone.clone(), self.person.clone(), self.into.clone());
    let (server, project) = (target.server, project.clone());
    move || {
      if !person.allows(clone_view::START) {
        return;
      }
      run.run_if_idle(Ask::Start(Box::new(CloneRequest {
        server,
        project: project.clone(),
        destination: typed_path(&into.get_untracked()),
        words: words.clone(),
      })));
    }
  }
}

/// What `ask` comes to.
async fn asked(clones: &Clones, programs: Programs, ask: Ask) -> Result<CloneEnd, CloneRefusal> {
  match ask {
    Ask::Start(request) => clones.start(programs, *request).await,
    // The page asks to follow only a clone `follow` found, and a clone is never forgotten in a session.
    Ask::Follow(key) => clones.ended(&key).await.unwrap_or(Err(CloneRefusal::Interrupted)),
  }
}

/// What the folder `text` names is, as the place for a new checkout.
async fn checked(text: String) -> Place {
  if text.trim().is_empty() {
    return Place {
      text,
      destination: None,
    };
  }
  let path = plain_destination(&typed_path(&text));
  let destination = match background::run(move || check_destination(&path)).await {
    Ok(destination) => destination,
    // The work did not finish; `background::run` logged why.
    Err(_) => Destination::Unreadable,
  };
  Place {
    text,
    destination: Some(destination),
  }
}

/// The folder a checkout of `project` gets: its repository's last segment, as `git clone` names it.
fn repository_name(project: &SharedProject) -> String {
  project
    .remote_hint
    .as_ref()
    .and_then(|hint| hint.as_str().rsplit('/').next())
    .map_or_else(|| project.slug.clone(), str::to_owned)
}
