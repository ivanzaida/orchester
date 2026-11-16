//! *Leading to the end* (`Leading to the end` `mwOht` in `S01 · Project settings · Orchestrator · leading defaults`
//! `pK4S5`, board `Orchestrator autonomy · frames to approve`; `docs/plan/orchestrator-autonomy/OA-10-…`): the
//! ceilings a new subscription that leads its target to the end takes, under *Watching*. Four rows built as *A run
//! without progress for* is: *Runs of a task* (a box and *runs*), *Returns per run* (*returns*), *Raises of a limit*
//! (how often, *by*, how much, *%*) and *Budget* (*tokens*, empty for none, which the box says as *None*), each with
//! its bounds in its hint. They save on Enter or as the person leaves a box, like the rest of the tab; a value out of
//! bounds says why under its row and saves nothing, and leaving its box puts the saved value back. A subscription
//! that leads already keeps its own ceilings, which `/watching` changes for that one.
//!
//! They bound what the Orchestrator does without the person, so only a person sets them: while this tab shows, the
//! app's MCP server refuses window input (the switch above protects it, `mcp::input_gate`), and synthetic input that
//! reaches a box all the same saves nothing. A budget is typed as every budget is, an absolute whole number of
//! tokens, so its box is wider than the frame's 88.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::orchestrator::Ceilings;
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::typed_ceilings::{TypedCeilings, Wrong},
};
use parking_lot::Mutex;

use super::super::{
  Status, TabProps,
  kit::{Group, SettingRow, on_enter},
};
use crate::{
  queries::{self, Session, project},
  services::Services,
  theme::{ExtraSpacing, ProjectSettingsText},
  ui::{
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
    setting_rows::marked_words,
    skeleton,
  },
};

/// The group's element id; a row is `<ID>-<name>`, its box `<ID>-<name>-field` and its error `<ID>-<name>-error`.
const ID: &str = "settings-orchestrator-leading";
/// A count's box (`Number` `l6sGPO`), and the bone of its value while it is read.
const COUNT_WIDTH: f32 = 56.0;
const COUNT_BONE: f32 = 8.0;
/// The budget's box: the frame's (`K2Qo0`, 88) holds *None*; this one holds a budget as it is typed, *40,000,000*.
const BUDGET_WIDTH: f32 = 112.0;
const BUDGET_BONE: f32 = 32.0;

/// The ceilings as a signal's and a save's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Kept(Ceilings);

impl DevtoolsInspectable for Kept {}

type SaveAction = FutureAction<Kept, Kept, Message>;

/// One of the section's boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
  Runs,
  Returns,
  Raises,
  Percent,
  Budget,
}

impl Field {
  /// Whether `wrong` names this box.
  const fn is(self, wrong: Wrong) -> bool {
    matches!(
      (self, wrong),
      (Self::Runs, Wrong::Runs)
        | (Self::Returns, Wrong::Returns)
        | (Self::Raises, Wrong::Raises)
        | (Self::Percent, Wrong::PercentBelow | Wrong::PercentAbove)
        | (Self::Budget, Wrong::Budget)
    )
  }
}

#[derive(Clone)]
pub(super) struct Leading {
  runs: Signal<String>,
  returns: Signal<String>,
  raises: Signal<String>,
  percent: Signal<String>,
  budget: Signal<String>,
  /// What the boxes were filled from or last saved as: the base of the next save; `None` until first read.
  stored: Signal<Option<Kept>>,
  /// A save asked for while another ran.
  queued: Signal<bool>,
  save: SaveAction,
  /// What the query last read, so only a new read fills the boxes.
  seen: Arc<Mutex<Option<Ceilings>>>,
  /// The locale's group separator, which a budget's digits are grouped by.
  separator: Arc<str>,
}

impl Component for Leading {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let root = ctx.props::<TabProps>().project.open.repo_root.clone();
    let save = save_action(ctx, root);
    let tab = Self {
      runs: ctx.signal(String::new()),
      returns: ctx.signal(String::new()),
      raises: ctx.signal(String::new()),
      percent: ctx.signal(String::new()),
      budget: ctx.signal(String::new()),
      stored: ctx.signal(None),
      queued: ctx.signal(false),
      save: save.clone(),
      seen: Arc::default(),
      separator: ctx.t("project_settings.groupSeparator"),
    };
    let after = tab.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if state.error.is_none()
        && let Some(saved) = state.data
      {
        after.stored.set(Some(saved));
      }
      if after.queued.get_untracked() {
        after.submit();
      }
    });
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let session = Session::of(ctx);
    let read = queries::state(&ctx.query(project::orchestrator_lead_defaults(
      session,
      props.project.open.repo_root.clone(),
    )));
    let state = self.save.state().get();
    if let Some(data) = read.data {
      self.follow(data, state.is_pending());
    }
    let stored = self.stored.get();
    let asked = self.typed(true).read(&self.separator);
    let wrong = match (&stored, &asked) {
      (Some(_), Err(wrong)) => wrong.clone(),
      _ => Vec::new(),
    };
    let edited = matches!((&stored, &asked), (Some(Kept(base)), Ok(asked)) if asked != base);
    props.status.set(match (state.is_pending(), &state.error) {
      (true, _) => Status::Saving,
      (false, Some(_)) => Status::Failed,
      (false, None) if !wrong.is_empty() => Status::Invalid,
      (false, None) if edited => Status::Unsaved,
      (false, None) => Status::Saved,
    });
    let rows = self.rows(ctx, stored.is_some(), &wrong);
    let place = marked_words(
      &format!("{ID}-where"),
      Icon::Flag,
      &ctx.t("project_settings.orchestrator.leading.where"),
    );
    let default = Ceilings::default();
    let foot = ctx.t_args(
      "project_settings.orchestrator.leading.foot",
      [
        ("runs", default.runs_of_task().to_string()),
        ("returns", default.returns_of_run().to_string()),
        ("raises", default.raises_of_run().to_string()),
        ("percent", default.raise_percent().to_string()),
      ],
    );
    let mut group = Group::new(ID, ctx.t("project_settings.orchestrator.leading.title"))
      .end(place)
      .rows(rows)
      .foot(&foot);
    let problems = [
      (
        read.error.as_ref(),
        "unreadable",
        "project_settings.orchestrator.unreadable",
      ),
      (
        state.error.as_ref(),
        "not-saved",
        "project_settings.orchestrator.notSaved",
      ),
    ];
    for (reason, part, key) in problems {
      if let Some(reason) = reason {
        let reason = message_text(ctx, reason);
        group = group.after(field_error(
          &format!("{ID}-{part}"),
          &ctx.t_args(key, [("reason", &*reason)]),
        ));
      }
    }
    // Enter saves, as the tab's other fields do.
    let tab = self.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .on_key_down(on_enter(move || tab.submit()))
      .child(Element::from(group))
  }
}

impl Leading {
  /// The four rows; until the ceilings are read (`read`) their boxes are bones. A box named in `wrong` says why under
  /// its row.
  fn rows(&self, ctx: &mut Ctx, read: bool, wrong: &[Wrong]) -> Vec<Element> {
    let state = (read, wrong);
    let range = |(least, most): (u8, u8)| vec![("min", least.to_string()), ("max", most.to_string())];
    let whole =
      |ctx: &Ctx, bounds: (u8, u8)| ctx.t_args("project_settings.orchestrator.leading.invalid", range(bounds));
    let runs = (Ceilings::MIN_RUNS, Ceilings::MAX_RUNS);
    let returns = (0, Ceilings::MAX_RETURNS);
    let raises = (0, Ceilings::MAX_RAISES);
    let share = (TypedCeilings::MIN_RAISE_PERCENT, Ceilings::MAX_RAISE_PERCENT);

    let controls = vec![
      self.number(
        ctx,
        (Field::Runs, "project_settings.orchestrator.leading.runs.label"),
        state,
      ),
      unit(ctx, ("runs-unit", "project_settings.orchestrator.leading.runs.unit")),
    ];
    let why = wrong.contains(&Wrong::Runs).then(|| whole(ctx, runs));
    let runs_row = self.row(ctx, ("runs", range(runs)), controls, why);

    let controls = vec![
      self.number(
        ctx,
        (Field::Returns, "project_settings.orchestrator.leading.returns.label"),
        state,
      ),
      unit(
        ctx,
        ("returns-unit", "project_settings.orchestrator.leading.returns.unit"),
      ),
    ];
    let why = wrong.contains(&Wrong::Returns).then(|| whole(ctx, returns));
    let returns_row = self.row(ctx, ("returns", range(returns)), controls, why);

    let controls = vec![
      self.number(
        ctx,
        (Field::Raises, "project_settings.orchestrator.leading.raises.times"),
        state,
      ),
      unit(ctx, ("raises-by", "project_settings.orchestrator.leading.raises.by")),
      self.number(
        ctx,
        (Field::Percent, "project_settings.orchestrator.leading.raises.share"),
        state,
      ),
      unit(
        ctx,
        ("raises-percent", "project_settings.orchestrator.leading.raises.percent"),
      ),
    ];
    // The first of the two boxes that is out of bounds says why.
    let why = if wrong.contains(&Wrong::Raises) {
      Some(whole(ctx, raises))
    } else {
      wrong.iter().any(|wrong| Field::Percent.is(*wrong)).then(|| {
        ctx.t_args(
          "project_settings.orchestrator.leading.raises.invalidShare",
          range(share),
        )
      })
    };
    let mut bounds = range(raises);
    bounds.push(("percent", share.1.to_string()));
    let raises_row = self.row(ctx, ("raises", bounds), controls, why);

    let controls = vec![
      self.number(
        ctx,
        (Field::Budget, "project_settings.orchestrator.leading.budget.label"),
        state,
      ),
      unit(
        ctx,
        ("budget-unit", "project_settings.orchestrator.leading.budget.unit"),
      ),
    ];
    let why = wrong
      .contains(&Wrong::Budget)
      .then(|| ctx.t("project_settings.orchestrator.leading.budget.invalid"));
    let budget_row = self.row(ctx, ("budget", Vec::new()), controls, why);
    vec![runs_row, returns_row, raises_row, budget_row]
  }

  /// The row `name`: its label, its hint with `bounds` in it, its `controls` at its end, and `why` its value is wrong
  /// under it.
  fn row(
    &self,
    ctx: &mut Ctx,
    (name, bounds): (&str, Vec<(&'static str, String)>),
    controls: Vec<Element>,
    why: Option<Arc<str>>,
  ) -> Element {
    let key = |part: &str| format!("project_settings.orchestrator.leading.{name}.{part}");
    let id = format!("{ID}-{name}");
    let control = Row::new()
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .with_children(controls);
    let problem = why.map(|why| Element::from(field_error(&format!("{id}-error"), &why)));
    SettingRow::new(id, ctx.t(&key("label")))
      .hint(ctx.t_args(&key("hint"), bounds))
      .control(control)
      .under_all(problem)
      .render(ctx)
  }

  /// The box `which`, named for tools by the words at the key `name`; a bone until the ceilings are `read`, and in
  /// the `danger` stroke while `wrong` names it.
  fn number(&self, ctx: &mut Ctx, (which, name): (Field, &str), (read, wrong): (bool, &[Wrong])) -> Element {
    let (part, width, bone) = match which {
      Field::Runs => ("runs", COUNT_WIDTH, COUNT_BONE),
      Field::Returns => ("returns", COUNT_WIDTH, COUNT_BONE),
      Field::Raises => ("raises", COUNT_WIDTH, COUNT_BONE),
      Field::Percent => ("percent", COUNT_WIDTH, COUNT_BONE),
      Field::Budget => ("budget", BUDGET_WIDTH, BUDGET_BONE),
    };
    if !read {
      return skeleton::field(ctx, width, bone).into();
    }
    let label = ctx.t(name);
    let tab = self.clone();
    let field = FieldInput::new(format!("{ID}-{part}-field"), label, self.signal(which).clone())
      .width(Dimension::Px(width))
      .text_style(ProjectSettingsText::MonoValue.role())
      .end_aligned()
      .on_leave(move || tab.leave(which));
    let field = if which == Field::Budget {
      field
        .placeholder(ctx.t("project_settings.orchestrator.leading.budget.none"))
        .grouped(self.separator.clone())
    } else {
      field
    };
    let field = if wrong.iter().any(|wrong| which.is(*wrong)) {
      field.invalid()
    } else {
      field
    };
    field.render(ctx)
  }

  const fn signal(&self, which: Field) -> &Signal<String> {
    match which {
      Field::Runs => &self.runs,
      Field::Returns => &self.returns,
      Field::Raises => &self.raises,
      Field::Percent => &self.percent,
      Field::Budget => &self.budget,
    }
  }

  /// What the boxes hold, read tracked when `tracked`.
  fn typed(&self, tracked: bool) -> TypedCeilings {
    let read = |field: &Signal<String>| if tracked { field.get() } else { field.get_untracked() };
    TypedCeilings {
      runs: read(&self.runs),
      returns: read(&self.returns),
      raises: read(&self.raises),
      percent: read(&self.percent),
      budget: read(&self.budget),
    }
  }

  /// Fills the boxes from what the query read, once it read something new: first, and after a change from elsewhere
  /// or a save, unless a save runs or the boxes hold an edit not saved yet.
  fn follow(&self, data: Ceilings, saving: bool) {
    {
      let mut seen = self.seen.lock();
      if *seen == Some(data) {
        return;
      }
      *seen = Some(data);
    }
    let unedited = self
      .stored
      .get_untracked()
      .is_none_or(|Kept(base)| self.typed(false).read(&self.separator) == Ok(base));
    if !saving && unedited {
      self.fill(data);
    }
  }

  fn fill(&self, data: Ceilings) {
    let typed = TypedCeilings::of(data, &self.separator);
    self.runs.set(typed.runs);
    self.returns.set(typed.returns);
    self.raises.set(typed.raises);
    self.percent.set(typed.percent);
    self.budget.set(typed.budget);
    self.stored.set(Some(Kept(data)));
  }

  /// Leaving the box `which`: a value out of bounds goes back to the saved one, and nothing wrong is saved.
  fn leave(&self, which: Field) {
    let wrong = self.typed(false).read(&self.separator).err().unwrap_or_default();
    if wrong.iter().any(|wrong| which.is(*wrong))
      && let Some(Kept(base)) = self.stored.get_untracked()
    {
      let kept = TypedCeilings::of(base, &self.separator);
      let value = match which {
        Field::Runs => kept.runs,
        Field::Returns => kept.returns,
        Field::Raises => kept.raises,
        Field::Percent => kept.percent,
        Field::Budget => kept.budget,
      };
      self.signal(which).set(value);
    }
    self.submit();
  }

  /// Saves what the boxes ask for when every one holds a ceiling and they differ from what is stored; a save asked
  /// for while another runs follows it. Only a person's input saves.
  fn submit(&self) {
    if is_synthetic_input() {
      tracing::warn!(
        setting = "Leading to the end",
        "synthetic input reached a person's setting; not taken"
      );
      return;
    }
    if self.save.state().get_untracked().is_pending() {
      self.queued.set(true);
      return;
    }
    self.queued.set(false);
    let (Some(Kept(base)), Ok(asked)) = (self.stored.get_untracked(), self.typed(false).read(&self.separator)) else {
      return;
    };
    if asked != base {
      self.save.run(Kept(asked));
    }
  }
}

/// *runs*, *returns*, *by*, *%* or *tokens* beside a box: the words at `key`, named `<ID>-<part>`.
fn unit(ctx: &Ctx, (part, key): (&str, &str)) -> Element {
  Text::new(&ctx.t(key))
    .id(format!("{ID}-{part}"))
    .variant(ProjectSettingsText::Value.role())
    .nowrap()
    .into()
}

/// Saves the ceilings a new leading subscription of the project at `root` takes; the workspace counts the change,
/// which reads the query again.
fn save_action(ctx: &mut Ctx, root: PathBuf) -> SaveAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |Kept(ceilings): Kept| {
    let (workspace, root) = (workspace.clone(), root.clone());
    async move {
      background::run(move || workspace.set_orchestrator_lead_defaults(&root, &ceilings))
        .await?
        .map(|()| Kept(ceilings))
        .inspect_err(|reason| tracing::error!(?reason, "the ceilings of leading to the end were not saved"))
    }
  })
}
