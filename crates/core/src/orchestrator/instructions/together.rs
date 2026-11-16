//! Several events that wait in one conversation are taken in one turn
//! (`docs/plan/orchestrator-autonomy/OA-07-…`): a conversation that leads a module of twenty tasks is not an hour
//! behind its own events. The turn the oldest of them starts takes the next ones with it, and is told all of them.

/// How many events one turn takes at most; the rest wait for the next turn.
pub const TOGETHER_MOST: usize = 5;

/// The prompt of a turn that takes several events, each told as `prompts` tell it alone, oldest first. One event is
/// told as it is.
pub fn together_prompt(prompts: &[String]) -> String {
  let [only] = prompts else {
    let count = prompts.len();
    let told: Vec<String> = prompts
      .iter()
      .enumerate()
      .map(|(index, prompt)| {
        format!(
          "Event {} of {count}:
{prompt}",
          index + 1
        )
      })
      .collect();
    return format!(
      "[Orchester] {count} events wait in this conversation, and this one turn takes them all, oldest first. Settle        them in that order, each as its own message below asks, with its own tools and limits: what holds for the        event that started a turn holds for each of them. An event you do not reach in this turn stays open.

{}",
      told.join("

")
    );
  };
  only.clone()
}
