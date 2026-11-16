//! *While you were away* on the Overview (`W01 · Overview · parked for you and decided without you` and `Parts ·
//! While you were away` on the board `Orchestrator autonomy · frames to approve`;
//! `docs/plan/orchestrator-autonomy/OA-23-…`), in the digest's pattern (`super::digest`): two frames of their own.
//!
//! *N parked for you*: a task the Orchestrator parked, oldest first, two lines each: the task and what is needed,
//! how long ago, **Give it back** and the link that opens the task; under them why it could not go on and what to do,
//! in its own words. Giving a task back ends the parking, and the conversation that leads it takes it up again.
//!
//! *N decided without you*: the decisions the person has not looked at, newest first, grouped by module, two lines
//! each: what was done and when, **Seen** and **Overrule…**; under them the reason and the conversation that decided.
//! **Mark all seen** at the head. *Overrule…* opens that conversation with the decision quoted in the composer: the
//! words are the person's own, and nothing is undone before the Orchestrator's next turn does it.
//!
//! The page mounts it twice: what was parked over what waits for the person, what was decided under it.
//!
//! All four actions are a person's: synthetic input is refused outside a QA copy (`crate::mcp::qa_person`). With
//! nothing parked and nothing decided nothing shows.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId, TaskId},
  orchestrator::parking::Needs,
  project::TaskKey,
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    away::{Away, OpenDecision, ParkedTask, away_in, mark_seen_in, overrule_in},
    parking::give_back_in,
  },
};

use super::{
  digest::frame,
  section::{age, link, mono_time, muted},
};
use crate::{
  mcp::qa_person::QaPerson,
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route, TaskView},
  screens::{orchestrator::draft, settings::orchester::moment},
  theme::{ExtraSpacing, ExtraTypography, IconSize, OverviewText},
  ui::{
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
  },
};

/// A frame's head (`Head` in W01), as the digest's.
const HEAD_HEIGHT: f32 = 36.0;
/// A module's line over its decisions (`Group · Webhooks v2`).
const GROUP_HEIGHT: f32 = 28.0;

#[derive(Clone, PartialEq)]
pub struct AwayProps {
  pub project: ShownProject,
  pub part: Part,
}

/// Which of the two frames a mount shows: what was parked stands over what waits, what was decided under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
  Parked,
  Decided,
}

impl DevtoolsInspectable for AwayProps {}

/// What was read for the person who comes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read(Arc<Away>);

impl DevtoolsInspectable for Read {}

/// What the person does about what they read.
#[derive(Clone, Debug)]
enum Act {
  GiveBack(TaskId),
  Seen(Vec<i64>),
  Overrule(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Done;

impl DevtoolsInspectable for Done {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed(Message);

impl DevtoolsInspectable for Failed {}

type ActAction = FutureAction<(PathBuf, Act), Done, Failed>;

pub struct AwayCards {
  act: ActAction,
}

impl Component for AwayCards {
  type Props = AwayProps;

  fn create(ctx: &mut Ctx) -> Self {
    let act: ActAction = ctx.future_action(move |(root, act): (PathBuf, Act)| async move {
      background::run(move || {
        let now = Timestamp::now();
        match act {
          Act::GiveBack(task) => give_back_in(&root, task).map(|_| ()),
          Act::Seen(numbers) => mark_seen_in(&root, &numbers, now),
          Act::Overrule(number) => overrule_in(&root, number, now),
        }
      })
      .await
      .map_err(Failed)?
      .map(|()| Done)
      .map_err(Failed)
    });
    let feed = ctx.props::<AwayProps>().project.feed.clone();
    ctx.watch(&act.state(), move |state| {
      if state.is_fulfilled() {
        feed.saved();
      }
    });
    Self { act }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let AwayProps { project, part } = ctx.props::<AwayProps>().clone();
    let root = project.open.repo_root.clone();
    // Read again after every save of the project's tasks: a parking that ended, a decision just made.
    let saves = project.feed.saves.get();
    let read = ctx
      .future((root, saves), move |(root, _): (PathBuf, u64)| async move {
        background::run(move || away_in(&root).map(|away| Read(Arc::new(away)))).await?
      })
      .state()
      .get();
    let column = Column::new()
      .id(match part {
        Part::Parked => "overview-away",
        Part::Decided => "overview-away-decided",
      })
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl);
    let Some(Read(away)) = read.data else {
      let unread = read
        .error
        .as_ref()
        .map(|error| field_error("overview-away-unread", &message_text(ctx, error)));
      return column.with_children(unread);
    };
    let state = self.act.state().get();
    let failed = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|Failed(message)| field_error("overview-away-failed", &message_text(ctx, message)));
    let view = View {
      project: project.open.project.id(),
      root: project.open.repo_root,
      act: self.act.clone(),
      person: QaPerson::of(ctx),
      now: Timestamp::now(),
    };
    let parked = (part == Part::Parked && !away.parked.is_empty()).then(|| view.parked(ctx, &away.parked));
    let decided = (part == Part::Decided && !away.decisions.is_empty()).then(|| view.decided(ctx, &away.decisions));
    column
      .describe("parked", away.parked.len().to_string())
      .describe("decisions", away.decisions.len().to_string())
      .with_children(parked)
      .with_children(decided)
      .with_children(failed)
  }
}

/// What the rows act with.
struct View {
  project: ProjectId,
  root: PathBuf,
  act: ActAction,
  person: QaPerson,
  now: Timestamp,
}

impl View {
  /// A person's own press runs `act`; synthetic input is refused, `action` worded after "Only a person can".
  fn press(&self, action: &'static str, act: Act) -> Press {
    let (run, root, person) = (self.act.clone(), self.root.clone(), self.person.clone());
    Press::run(move || {
      if !person.refuses(action) {
        run.run((root.clone(), act.clone()));
      }
    })
  }

  /// *N parked for you*.
  fn parked(&self, ctx: &mut Ctx, parked: &[ParkedTask]) -> Element {
    let title = if parked.len() == 1 {
      ctx.t("overview.away.parked.title.one")
    } else {
      ctx.t_args(
        "overview.away.parked.title.other",
        [("count", parked.len().to_string())],
      )
    };
    let rows: Vec<Element> = parked.iter().map(|task| self.parked_row(ctx, task)).collect();
    frame("overview-parked")
      .child(head(
        "overview-parked",
        (Icon::CirclePause, PaletteColor::Warning),
        (&title, &ctx.t("overview.away.parked.order")),
        None,
      ))
      .with_children(rows)
      .into()
  }

  fn parked_row(&self, ctx: &mut Ctx, parked: &ParkedTask) -> Element {
    let id = format!("overview-parked-{}", parked.key);
    let what = ctx.t_args(
      "overview.away.parked.what",
      [
        ("task", parked.key.as_str()),
        ("needs", &*needs(ctx, parked.parking.needs)),
      ],
    );
    let open = parked.key.parse::<TaskKey>().ok().map(|key| {
      let route = Route::Project(self.project, ProjectPage::Task(key, TaskView::default()));
      link(
        &format!("{id}-open"),
        &ctx.t("overview.away.parked.open"),
        Press::Open(Navigation::of(ctx).to(route)),
      )
    });
    let give_back = link(
      &format!("{id}-give-back"),
      &ctx.t("overview.away.parked.giveBack"),
      self.press(
        "give a parked task back to the Orchestrator",
        Act::GiveBack(parked.parking.task),
      ),
    );
    let line = line(&id, (Icon::CirclePause, PaletteColor::Warning), &what, &parked.title)
      .child(mono_time(&format!("{id}-age"), &age(ctx, parked.parking.at, self.now)))
      .child(give_back)
      .with_children(open);
    let words = ctx.t_args(
      "overview.away.parked.words",
      [
        ("why", parked.parking.why.as_str()),
        ("toDo", parked.parking.to_do.as_str()),
      ],
    );
    two_lines(&id, line, under(&format!("{id}-words"), &words, None))
      .describe("needs", parked.parking.needs.name())
      .into()
  }

  /// *N decided without you*, by module in the order their newest decisions come.
  fn decided(&self, ctx: &mut Ctx, decisions: &[OpenDecision]) -> Element {
    let title = if decisions.len() == 1 {
      ctx.t("overview.away.decided.title.one")
    } else {
      ctx.t_args(
        "overview.away.decided.title.other",
        [("count", decisions.len().to_string())],
      )
    };
    let all = link(
      "overview-decided-all-seen",
      &ctx.t("overview.away.decided.allSeen"),
      self.press(
        "mark the Orchestrator's decisions as seen",
        Act::Seen(decisions.iter().map(|open| open.decision.number).collect()),
      ),
    );
    let mut modules: Vec<&Option<String>> = Vec::new();
    for open in decisions {
      if !modules.contains(&&open.module) {
        modules.push(&open.module);
      }
    }
    let mut rows: Vec<Element> = Vec::new();
    for module in modules {
      let members: Vec<&OpenDecision> = decisions.iter().filter(|open| &open.module == module).collect();
      rows.push(group(ctx, module.as_deref(), members.len()));
      rows.extend(members.into_iter().map(|open| self.decision_row(ctx, open)));
    }
    frame("overview-decided")
      .child(head(
        "overview-decided",
        (Icon::Waypoints, PaletteColor::TextSecondary),
        (&title, &ctx.t("overview.away.decided.order")),
        Some(all),
      ))
      .with_children(rows)
      .into()
  }

  fn decision_row(&self, ctx: &mut Ctx, open: &OpenDecision) -> Element {
    let made = &open.decision.made;
    let number = open.decision.number;
    let id = format!("overview-decision-{number}");
    let (glyph, what) = did(ctx, &made.tool, made.task.as_deref());
    let seen = link(
      &format!("{id}-seen"),
      &ctx.t("overview.away.decided.seen"),
      self.press("mark a decision of the Orchestrator as seen", Act::Seen(vec![number])),
    );
    let overrule = link(
      &format!("{id}-overrule"),
      &ctx.t("overview.away.decided.overrule"),
      self.overrule(ctx, (number, made.chat), (&what, made.reason.as_str())),
    );
    let chat = Route::Project(
      self.project,
      ProjectPage::Orchestrator(OrchestratorChat::Chat(made.chat)),
    );
    let conversation = link(
      &format!("{id}-conversation"),
      &ctx.t("overview.away.decided.conversation"),
      Press::Open(Navigation::of(ctx).to(chat)),
    );
    let line = line(&id, (glyph, PaletteColor::TextMuted), &what, "")
      .child(mono_time(&format!("{id}-time"), &moment(ctx, made.at)))
      .child(seen)
      .child(overrule);
    two_lines(
      &id,
      line,
      under(&format!("{id}-reason"), made.reason.as_str(), Some(conversation)),
    )
    .describe("tool", made.tool.clone())
    .into()
  }

  /// *Overrule…*: the decision is quoted in its conversation's composer, marked overruled, and the conversation
  /// opens for the person's own words.
  fn overrule(&self, ctx: &mut Ctx, (number, chat): (i64, ChatId), (what, reason): (&str, &str)) -> Press {
    let quote = ctx.t_args("overview.away.decided.quote", [("what", what), ("reason", reason)]);
    let navigation = Navigation::of(ctx);
    let (run, root, person, project) = (self.act.clone(), self.root.clone(), self.person.clone(), self.project);
    Press::run(move || {
      if person.refuses("overrule a decision of the Orchestrator") {
        return;
      }
      draft::quote(project, chat, &quote);
      run.run((root.clone(), Act::Overrule(number)));
      navigation.go(Route::Project(
        project,
        ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)),
      ));
    })
  }
}

/// What the person is asked for, in the row's words.
fn needs(ctx: &Ctx, needs: Needs) -> Arc<str> {
  ctx.t(match needs {
    Needs::Decision => "overview.away.needs.decision",
    Needs::Approval => "overview.away.needs.approval",
    Needs::SignIn => "overview.away.needs.signIn",
    Needs::Tool => "overview.away.needs.tool",
    Needs::Ceiling => "overview.away.needs.ceiling",
    Needs::Other => "overview.away.needs.other",
  })
}

/// What a decision did, by the tool it was made with, and its glyph.
fn did(ctx: &Ctx, tool: &str, task: Option<&str>) -> (Icon, Arc<str>) {
  let (glyph, key) = match tool {
    "continue_run" => (Icon::RotateCw, "overview.away.did.continued"),
    "run_task" => (Icon::RotateCw, "overview.away.did.ran"),
    "answer_question" => (Icon::MessageCircleQuestion, "overview.away.did.answered"),
    "decide_plan" => (Icon::Map, "overview.away.did.plan"),
    "decide_approval" => (Icon::ShieldCheck, "overview.away.did.approval"),
    "park_task" => (Icon::CirclePause, "overview.away.did.parked"),
    "start_acceptance" => (Icon::ShieldCheck, "overview.away.did.acceptance"),
    "deliver_module" => (Icon::GitMerge, "overview.away.did.delivered"),
    _ => (Icon::Waypoints, "overview.away.did.other"),
  };
  let words = match task {
    Some(task) => ctx.t_args(key, [("task", task), ("tool", tool)]),
    None => ctx.t_args("overview.away.did.other", [("task", ""), ("tool", tool)]),
  };
  (glyph, words)
}

/// A frame's head: its glyph, `title` and `meta`, and what it ends in.
fn head(id: &str, (glyph, tone): (Icon, PaletteColor), (title, meta): (&str, &str), end: Option<Element>) -> Row {
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .child(icon(glyph, IconSize::Base, tone))
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(OverviewText::Head.role())
        .nowrap(),
    )
    .child(muted(meta))
    .child(Spacer::new().flex(1.0))
    .with_children(end)
}

/// A module's line over its decisions; a decision about no module's task stands under *No module*.
fn group(ctx: &Ctx, module: Option<&str>, count: usize) -> Element {
  let name = module.map_or_else(|| ctx.t("overview.away.decided.noModule"), Arc::from);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GROUP_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::Boxes, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&name)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(muted(&count.to_string()))
    .into()
}

/// A row's first line: its glyph, what it says and what follows it in the muted tone; the caller ends it.
fn line(id: &str, (glyph, tone): (Icon, PaletteColor), what: &str, aside: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, tone))
    .child(
      Text::new(what)
        .id(format!("{id}-what"))
        .variant(OverviewText::Line.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(aside)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// A row's second line, under its words and not its glyph: `words` wrapping, and a link at its end.
fn under(id: &str, words: &str, end: Option<Element>) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(Row::new().width(Dimension::Px(IconSize::Base.size())).flex_shrink(0.0))
    .child(
      Text::new(words)
        .id(id.to_owned())
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(end)
}

/// A row of two lines between the frame's hairlines.
fn two_lines(id: &str, first: Row, second: Row) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(first)
    .child(second)
}
