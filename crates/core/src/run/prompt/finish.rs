//! The *When you finish* part of a prompt: how the agent hands its result in and what each outcome means.

use super::{PromptInput, ResultDelivery, composition::push_block};

/// How an agent CLI hands in its result: its last message.
const RESULT_AS_MESSAGE: &str = "End with exactly one JSON object containing `outcome` and `summary`, for example \
                                 {\"outcome\":\"question\",\"summary\":\"Which option should I implement?\"}.";
/// How a native agent hands in its result: the `finish_step` tool (`docs/architecture/native-agent.md` §4.7,
/// §6.1).
const RESULT_AS_TOOL: &str = "End by calling the finish_step tool once with `outcome` and `summary`, for example \
                              {\"outcome\":\"question\",\"summary\":\"Which option should I implement?\"}; the call \
                              records your result and ends the step. When you need the person's answer to go on, \
                              call ask_question instead and wait for it.";
/// Every turn gets this contract after how to hand the result in, including resumed CLI sessions without an
/// output schema.
const RESULT_INSTRUCTIONS: &str = "Use `done` when your current step is complete and its required checks pass; \
                                   summarize its result for the steps that follow. For a review or verification, \
                                   completing the inspection alone is not success. A negative verdict or \
                                   unresolved blocking or major findings requires `failed`, even when you \
                                   finished writing the report. Explain the findings so the pipeline can route \
                                   corrections; do not repair them in a read-only review.\n\
                                   Check, review and verification verdicts must describe the current inputs. \
                                   Identify the revision or saved work each result and artifact covers. Earlier \
                                   failures are context, not a verdict on changed work: reproduce relevant \
                                   findings against the current candidate. Reuse a check only when its inputs \
                                   are unchanged, and state that limit. Preserve earlier evidence without \
                                   relabeling it as evidence for a new build. If fresh verification needs an \
                                   action from the person that lets you go on now, request it with `question`.\n\
                                   Use `unverified` when you checked what you could but some checks cannot be \
                                   done with what this step has (a state no fixture reaches, an app, tool or \
                                   fixture you cannot use): list each in `unverified` with what it is and why. \
                                   It goes to the person and sends nothing back to an earlier step. Mismatches \
                                   you did find still require `failed`.\n\
                                   Use `question` when progress needs the person's decision, permission, access, \
                                   or clarification. This includes a proposed command outside the sandbox, an \
                                   unavailable capability, or conflicting requirements that the person can \
                                   resolve by choosing an alternative or changing scope. Explain the blocker, \
                                   summarize saved progress, and ask a concrete question with the available \
                                   options. Orchester saves your work and waits for their answer, then resumes \
                                   the same session. Requesting permission does not grant it; use only the \
                                   permissions actually available to this session.\n\
                                   Use `failed` for checks that require corrections by an earlier step, or for \
                                   a failure you cannot resolve within this step and for which no decision or \
                                   action by the person would let you continue. Explain \
                                   what failed and what work was saved. This invokes the pipeline's failure \
                                   handling and can end the run and skip dependent steps: do not use it merely \
                                   because work is incomplete or needs permission. \
                                   If the person already answered the relevant question, apply their answer \
                                   within the effective permissions instead of asking again.\n\
                                   With `done`, `failed` or `unverified`, hand off to the agents after you in the \
                                   same object: \
                                   `completed` (what you completed), `decisions` (decisions later steps must \
                                   follow), `open_questions` and `next_agent_should_know`, each a list of short \
                                   strings, or null when you have nothing for it. Orchester keeps the handoff as \
                                   task memory; never put secret values in it.";
/// What a gated step's session is told on top: a person reviews its summary as the plan.
const GATED_RESULT_INSTRUCTIONS: &str = "A person reviews your summary before the steps after this one start: make \
                                         it the complete result of this step (for a plan, the whole plan), in \
                                         Markdown.";

/// How to hand the result in, what makes a result, and for a gated step that a person reviews it.
pub(super) fn push_result(text: &mut String, input: &PromptInput<'_>, delivery: ResultDelivery) {
  let how = match delivery {
    ResultDelivery::Message => RESULT_AS_MESSAGE,
    ResultDelivery::Tool => RESULT_AS_TOOL,
  };
  push_block(text, &format!("{how}\n{RESULT_INSTRUCTIONS}"));
  if input.step.approval_gate {
    push_block(text, GATED_RESULT_INSTRUCTIONS);
  }
}
