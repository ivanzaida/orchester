//! The Browser window's root: which project it shows (the latest request, [`BrowserLink::show`]), which of its tabs
//! the browser screencasts and at what size, the person's marks on the tabs, and closing. It sends `Show` when the tab
//! to show or the page area's size changes, and `StopShowing` when the window shows another project, is minimised or
//! closes (`docs/design/browser.md`, "Projects", "The page").

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    events::KeyboardEvent,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;
use orchester_ipc::browser::{BrowserCommand, BrowserMode, BrowserTabId, Viewport};
use parking_lot::Mutex;

use super::{
  commands::{Commands, Restarting},
  keys,
  link::{BrowserLink, Show},
  model::{self, Indicator},
  notice, page,
  state::{self, Lookup, MarksByProject, PersonHooks, Showing},
  title_bar, toolbar,
};
use crate::chrome;

/// How often the dots are judged again while one depends on the time.
const TICK: Duration = Duration::from_secs(1);

pub struct BrowserRoot {
  marks: Signal<MarksByProject>,
  failed: Signal<bool>,
  restarting: Signal<Restarting>,
  look: Signal<u64>,
  tick: Signal<u64>,
  interval: Interval,
  /// The page area, whose size the screencast takes.
  page: ElementRef,
  page_size: Signal<Option<(u32, u32)>>,
  wanted: Signal<Wanted>,
  /// The address line's text input, and Ctrl+L asking for its focus.
  address: ElementRef,
  focus_address: Signal<bool>,
  /// The page's hidden text input and a JavaScript prompt's field.
  page_text: ElementRef,
  dialog_answer: ElementRef,
  /// Whether the window showed *Empty* at the last render: the address line takes the focus when it starts to.
  was_empty: Mutex<bool>,
  /// The request last acted on.
  handled: Mutex<Option<u64>>,
  /// What the browser was last asked to show: screencast, or with the browser in a window of its own only named.
  sent: Mutex<Option<Sent>>,
  /// The tabs of the project shown as the window last saw them: a tab not among them just opened.
  known: Mutex<Option<(PathBuf, HashSet<BrowserTabId>)>>,
}

/// A tab asked for, by a request or the person, that is not shown yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Wanted(Option<BrowserTabId>);

impl DevtoolsInspectable for Wanted {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Sent {
  project: PathBuf,
  tab: Option<(BrowserTabId, Viewport)>,
}

impl Component for BrowserRoot {
  type Props = BrowserLink;

  fn create(ctx: &mut Ctx) -> Self {
    let link = ctx.props::<BrowserLink>().clone();
    let window = ctx.window();
    window.on_close_requested(move |request| {
      let browser = link.services().runners.browser_window();
      if let Some(project) = browser.shown_project() {
        let services = link.services();
        let commands = Commands::new(
          services.runners.clone(),
          services.runtime.clone(),
          project,
          (Signal::new(false), Signal::new(Restarting::default())),
        );
        commands.send_quietly(BrowserCommand::StopShowing);
      }
      link.closed();
      request.proceed();
    });
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    Self {
      marks: ctx.signal(MarksByProject::default()),
      failed: ctx.signal(false),
      restarting: ctx.signal(Restarting::default()),
      look: ctx.signal(0),
      interval: ctx.create_interval(TICK, move || next.update(|tick| *tick += 1)),
      tick,
      page: ctx.element_ref(),
      page_size: ctx.signal(None),
      wanted: ctx.signal(Wanted::default()),
      address: ctx.element_ref(),
      focus_address: ctx.signal(false),
      page_text: ctx.element_ref(),
      dialog_answer: ctx.element_ref(),
      was_empty: Mutex::new(false),
      handled: Mutex::new(None),
      sent: Mutex::new(None),
      known: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let link = ctx.props::<BrowserLink>().clone();
    let services = link.services().clone();
    ctx.provide(services.clone());
    ctx.provide(link.clone());
    let browser = services.runners.browser_window();
    let revision = state::follow_changes(ctx, &browser);
    let asked = state::follow_asked(ctx, &link);
    let work = state::follow_work(ctx, &services.runners);
    let found = state::found(ctx, &services.browser, self.look.get());
    self.tick.get();
    let project = asked
      .as_ref()
      .map(|asked| asked.project.clone())
      .or_else(|| browser.projects().first().map(|(root, _)| root.clone()));
    let Some(project) = project else {
      return Element::new();
    };
    self.take_request(asked.as_ref(), &project);
    let view = browser.view(&project);
    let owners = state::owners(ctx, &project, view.as_ref(), work);
    let now = Instant::now();
    let marks = self.marks.get().of(&project);
    let strip = view
      .as_ref()
      .map(|view| model::strip(view, &owners, &marks, now))
      .unwrap_or_default();
    let name: Arc<str> = view
      .as_ref()
      .map_or_else(|| project_name(&project), |view| Arc::from(view.name()));
    ctx.window().set_title(
      ctx
        .t_args("browser.window.title", [("project", name.as_ref())])
        .to_string(),
    );
    let commands = Commands::new(
      services.runners.clone(),
      services.runtime.clone(),
      project.clone(),
      (self.failed.clone(), self.restarting.clone()),
    );
    let waits_left = view
      .as_ref()
      .and_then(|view| view.waiting(view.shown()?))
      .map(|waiting| model::seconds_left(waiting.until, Timestamp::now()));
    let showing = Showing {
      restarting: self.restarting.get().0.as_ref() == Some(&project),
      project: Arc::new(project.clone()),
      name,
      view,
      revision,
      strip,
      found,
      commands,
      marks,
      waits_left,
      person: self.hooks(&project),
      address: self.address.clone(),
      page_text: self.page_text.clone(),
      dialog_answer: self.dialog_answer.clone(),
    };
    self.screencast(ctx, &showing, now);
    self.keep_ticking(&showing, now);
    self.focus_address(ctx, &showing);
    let body = self.body(ctx, &showing);
    chrome::secondary(ctx, body, |ctx, bar, lights| {
      title_bar::content(ctx, bar, lights, &showing)
    })
  }
}

impl BrowserRoot {
  /// A new request for a tab of `project` makes that tab the one wanted.
  fn take_request(&self, asked: Option<&Show>, project: &Path) {
    let Some(asked) = asked.filter(|asked| asked.project == project) else {
      return;
    };
    let mut handled = self.handled.lock();
    if handled.is_some_and(|handled| asked.sequence <= handled) {
      return;
    }
    *handled = Some(asked.sequence);
    if let Some(tab) = &asked.tab {
      self.wanted.set(Wanted(Some(tab.clone())));
    }
  }

  /// What the window's parts tell about the person: their input on a tab, a tab they chose, *Look again*.
  fn hooks(&self, project: &Path) -> PersonHooks {
    let (marks, project) = (self.marks.clone(), project.to_path_buf());
    let wanted = self.wanted.clone();
    let look = self.look.clone();
    PersonHooks::new(
      move |tab| {
        let now = Instant::now();
        let fresh = marks
          .get_untracked()
          .of(&project)
          .last_input()
          .is_some_and(|at| now.saturating_duration_since(at) < TICK);
        if !fresh {
          marks.update(|marks| marks.0.entry(project.clone()).or_default().used(tab, now));
        }
      },
      move |tab| wanted.set(Wanted(Some(tab.clone()))),
      move || look.update(|look| *look += 1),
    )
  }

  /// Asks the browser to show the tab to show at the page area's size, or to stop: it screencasts that tab, or with the
  /// browser in a window of its own only names it, so the strip and the address line go on showing a tab (`05`
  /// `GjcAk`). The tab shown is the one the window last asked for while it is open: the runner reports it only once it
  /// switched.
  fn screencast(&self, ctx: &mut Ctx, showing: &Showing, now: Instant) {
    let window = ctx.window().info();
    let size = self.page_size(ctx);
    let Wanted(wanted) = self.wanted.get();
    let opened = self.opened(showing);
    let mut sent = self.sent.lock();
    let asked = sent
      .as_ref()
      .filter(|sent| sent.project == *showing.project)
      .and_then(|sent| sent.tab.as_ref().map(|(tab, _)| tab.clone()));
    let target = showing.view.as_ref().and_then(|view| {
      model::tab_to_show(
        &showing.strip,
        (wanted.as_ref(), asked.as_ref().or(view.shown()), view.last_shown()),
        (&opened, &showing.marks, now),
      )
    });
    let viewport = size.and_then(|(width, height)| {
      let scale = (window.scale_factor * 100.0).round() as u32;
      Viewport::new(
        width,
        height,
        scale.clamp(Viewport::MIN_SCALE_PERCENT, Viewport::MAX_SCALE_PERCENT),
      )
    });
    let next = Sent {
      project: showing.project.as_ref().clone(),
      tab: target.filter(|_| !window.is_minimized).zip(viewport),
    };
    if sent.as_ref() == Some(&next) {
      return;
    }
    if let Some(before) = sent.as_ref().filter(|before| before.project != next.project) {
      showing
        .commands
        .for_project(&before.project)
        .send_quietly(BrowserCommand::StopShowing);
    }
    match &next.tab {
      Some((tab, viewport)) => {
        showing.commands.send_quietly(BrowserCommand::Show {
          tab: tab.clone(),
          viewport: *viewport,
        });
        self
          .marks
          .update(|marks| marks.0.entry(next.project.clone()).or_default().seen(tab));
        if wanted.as_ref() == Some(tab) {
          self.wanted.set(Wanted::default());
        }
      }
      None
        if sent
          .as_ref()
          .is_some_and(|before| before.tab.is_some() && before.project == next.project) =>
      {
        showing.commands.send_quietly(BrowserCommand::StopShowing);
      }
      None => {}
    }
    *sent = Some(next);
  }

  /// The tabs of the project shown that opened since the window last saw its tabs; none when it first sees them.
  fn opened(&self, showing: &Showing) -> Vec<BrowserTabId> {
    let now: HashSet<BrowserTabId> = showing.strip.all().map(|tab| tab.id.clone()).collect();
    let mut known = self.known.lock();
    let opened = match known.as_ref() {
      Some((project, before)) if project == showing.project.as_ref() => now.difference(before).cloned().collect(),
      _ => Vec::new(),
    };
    *known = Some((showing.project.as_ref().clone(), now));
    opened
  }

  /// The page area's size in logical pixels, once it was laid out; watched from now on.
  fn page_size(&self, _ctx: &mut Ctx) -> Option<(u32, u32)> {
    let size = self.page_size.clone();
    self.page.observe_rect(move |rect| {
      let measured = (rect.width.round() as u32, rect.height.round() as u32);
      if measured.0 > 0 && measured.1 > 0 && size.get_untracked() != Some(measured) {
        size.set(Some(measured));
      }
    });
    self.page_size.get()
  }

  /// Ticks while what is shown depends on the time: an agent acting or waiting and the seconds it still waits, or the
  /// person's recent input, which keeps the window from following an agent to its new tab.
  fn keep_ticking(&self, showing: &Showing, now: Instant) {
    let timed = showing
      .strip
      .all()
      .any(|tab| matches!(tab.indicator, Some(Indicator::Acting | Indicator::Waits)))
      || showing
        .marks
        .last_input()
        .is_some_and(|at| now.saturating_duration_since(at) < model::LEFT_ALONE_AFTER);
    if timed {
      self.interval.start();
    } else {
      self.interval.stop();
    }
  }

  /// Gives the address line the focus when Ctrl+L asks for it, and when the window starts to show *Empty*, whose
  /// address line has the focus (`06` `eCCuU`); the line itself lets go of that when a tab comes (`toolbar::Address`).
  fn focus_address(&self, ctx: &mut Ctx, showing: &Showing) {
    let empty = showing.found == Lookup::Found && showing.usable() && showing.strip.is_empty();
    let became_empty = {
      let mut was_empty = self.was_empty.lock();
      let became = empty && !*was_empty;
      *was_empty = empty;
      became
    };
    let asked = self.focus_address.get();
    if asked {
      self.focus_address.set(false);
    }
    if asked || became_empty {
      ctx.focus(&self.address);
    }
  }

  /// The toolbar over the page, in the window's panel (`Body` `JiHDn`, `Panel` `DsHim`), which takes the window's
  /// shortcuts before the page (`keys`).
  fn body(&self, ctx: &mut Ctx, showing: &Showing) -> Element {
    let failed = self.failed.get();
    let content = if showing.found == Lookup::Looking {
      Element::new()
    } else if showing.unavailable() {
      notice::not_found(ctx, showing)
    } else if showing.mode() == BrowserMode::Headed {
      notice::headed(ctx, showing)
    } else if showing.strip.is_empty() {
      notice::empty(ctx)
    } else {
      ctx.mount_keyed::<page::Page>("browser-page", showing.clone())
    };
    let page_area = Column::new()
      .id("browser-page-area")
      .ref_element(self.page.clone())
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(content);
    let focus = self.focus_address.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .on_key_down(keys::handler(showing.clone(), move || focus.set(true)))
      .on_key_up(|event: KeyboardEvent| keys::swallow_up(&event))
      .child(toolbar::panel(ctx, showing, failed, page_area.into()))
      .into()
  }
}

/// The name of the project at `root` until its runner reports its browser: its folder's.
fn project_name(root: &Path) -> Arc<str> {
  Arc::from(
    root
      .file_name()
      .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
  )
}
