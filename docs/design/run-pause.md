# Pause: what each run state does (ORC-964)

Pause asks a running run to stop at a checkpoint. It never ends a running agent turn or a running command by itself; it
stops everything that would *start* next, ends what only waits for a person, and lets work that is running end on its
own. The run is recorded as paused once no agent or command of it is at work. Until then the run stays *Running* with
the pause requested, and the header's *Pausing…* names what it waits for (`PauseWait`,
`crates/workspace/src/tasks/pause_wait.rs`). Cancel stays available throughout.

Code: `Control::Pause` → `RunTask::control` (`crates/runner/src/execution/executor/halt.rs`) records the request,
tells every agent `Stop::AtApproval`, and `settle()` records the pause once `agents` is empty. A run no task executes
is handled by `coordinator::idle::pause`.

| State when Pause is pressed | What happens | The header says |
| --- | --- | --- |
| An agent waits for a tool approval | The agent is ended at once, its work saved; the approval is recorded as interrupted and asked again after Resume. If another command of the same agent still runs, the pause waits for that command to end first. | Pausing at the next checkpoint (for a moment) |
| An agent waits in its turn for the answer to a question with suggested answers | Same as an approval: the agent is ended at once (the bug of ORC-964 was Claude Code's own `ask_question` call counting as a running command, so the pause never settled). The question is recorded as interrupted: it cannot be answered while paused, and the agent asks it again after Resume if it still needs the answer. | Pausing at the next checkpoint (for a moment) |
| An agent waits after its turn (a question without options, a plan decision) | Nothing works: the run pauses at once. The question or plan stays open; a plan is decided once the run is resumed, a message queues for the agent. | – (Paused) |
| An agent works (thinking, editing, a long or hung command) | The agent finishes its invocation; no new step starts. A hung command is ended with Cancel. | Pausing once *agent* finishes *step* |
| A command tool step runs | The program runs to its end or to its tool's timeout; nothing after it starts. | Pausing once *step*'s command ends · *tool* |
| Several steps work beside each other | The pause waits for all of them. | Pausing once *steps* end |
| A step fails and its route starts an attempt again | No retry starts while a pause is requested; the run pauses at once and the attempt starts on Resume. | – (Paused) |
| The runner merges the moved base into a step, or an agent resolves it | The runner's own merge ends within the run task before it reads the pause; an agent's merge is an agent at work (above). | as the agent's step |
| The run waits for an agent slot, a workspace or the code index | Nothing starts while a pause is requested, so no wait is left: the run pauses at once. | – (Paused) |
| An agent's browser tool waits for a person | The wait is ended and the call refused, then the agent is at work as above. | as the agent's step |
| An agent starts a QA copy or services | A tool call of the agent: the pause waits for the agent. Known gap: a `start_qa_app` call that waits for a free QA slot of the computer waits for another run's QA app to end first. | Pausing once *agent* finishes *step* |
| The task waits on another task (`run_waits_on`) | The run is not started (*Pending*), so there is nothing to pause: Pause is not offered; Cancel stops the wait. | – |
| The runner restarts or is not connected | The controls are locked with *runner is not connected*; a run recorded as running without a runner is interrupted and continues with Continue. | Runner not connected |
| Already paused | Pause is not offered; a request answers *done* and changes nothing. | Paused |
| Stopped at a limit, interrupted, or waiting for a person's decision (*Needs input*) | Pause is not offered and the runner refuses it (`RunNotRunning`): nothing works for the run. A run that waits for a person while its task ended is paused at once by `coordinator::idle::pause`. | their own state |

Pausing is never left up with nothing to end it: each *Running* state above either pauses at once or names the work
that ends it, and every command ends (a tool's timeout, an agent's end, Cancel).
