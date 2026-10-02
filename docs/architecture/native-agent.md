# Orchester — native agent

Date: 2026-10-01. Status: **decided, not started**. The user's answers are recorded in
[Decisions (2026-10-01)](#decisions-2026-10-01) and folded into the sections below; no question is left open. Items
marked *to verify* are facts to check against the provider's current documentation before code relies on them, not
decisions.

Sources:
- Anthropic Messages API: the `claude-api` skill bundled with Claude Code (model table cached 2026-09-25).
- OpenAI Responses API: the developers.openai.com reference pages, fetched 2026-10-01.
- DeepSeek: the pages saved in `.tmp/deepseek/` on 2026-10-01, and the findings of the research done alongside this
  proposal.

Today every Orchester agent is a CLI child process: Codex CLI or Claude Code CLI. A **native agent** is Orchester
running the coding-agent loop itself. It sends requests to a model API, runs the tools the model asks for under
Orchester's own permission rules, and records everything as the same `AgentEvent` transcript the CLIs produce. Users
add it as a connection of kind *Anthropic API (native)* or *OpenAI API (native)*, optionally with a custom base URL
(DeepSeek and other compatible endpoints) and a custom model list.

## Decisions (2026-10-01)

The user answered the proposal's questions on 2026-10-01:

1. **Allowed commands ask about the rest, everywhere.** *Allowed commands* means: listed and known-safe read-only
   commands run, and every other command asks a person. This holds for the CLIs (branch `claude/ask-outside`
   implements it) and for the native agent. There is no separate `AllowedThenAsk` option (§5.2).
2. **A denial does not fail the step.** The native agent gets the denial as the command's result and continues. The
   CLIs, where Deny still fails the step, will be aligned later (§5.2).
3. **Shell for `shell: true`:** on Windows `pwsh` when it is found, otherwise Windows PowerShell 5.1; on macOS zsh
   (§4.6).
4. **Anthropic provider fallback is off by default.** A refusal is shown as a problem (§3.1, §7).
5. **A Chat Completions client comes later**, after P3, for self-hosted servers (§3.2, §9).
6. **The runner may talk to MCP servers itself** for native connections, in P4. The design rule in
   `docs/design/orchester-workspace.md` (MCP servers) says so (§4.9).
7. **No custom base URL or key for CLI connections** for now (§7).
8. **No cost estimates.** At most later, as information in the Usage dashboard, never in limits (§10).
9. **No `task_budget`** for now; revisit after P1 (§3.1).
10. **One acknowledgement per project** is enough for presets with a data-location notice, such as DeepSeek (§7).

## 1. Goals and non-goals

### Day-one parity with the CLI agents

These must work in the first shipped phase, the same way they work for Codex and Claude Code:

- **Running a step.** A pipeline agent step runs on a native connection with the profile's model and effort. It uses
  the same prompt composition: role, project rules, pipeline, step, code index, skills, task memory, earlier results,
  messages.
- **Step results.** The step ends with a validated `StepResult` (`done` / `failed` / `question`) whose handoff lists
  become task memory. A `question` makes the step wait for the person. The answer resumes the same conversation.
- **Tools.**
  - Read, list/glob and search the worktree and the code index.
  - Exact-string edit, multi-edit, and write file.
  - Run commands: allowed list plus known-safe read-only commands run without asking. Every other command asks a
    person through the existing command approvals; a denial goes back to the agent, which continues.
- **Permissions.** Repository access, writable folders, commands and network map onto `EnforcedPermissions` with
  honest mechanisms. Options Orchester cannot enforce are locked, as they are for the CLIs.
- **Run controls.** Usage accounting, run, step and daily token budgets, active-time limits, pause, cancel, resume
  and continue after an interruption all work as they do today. Native usage is reported per model response, so
  budget stops are finer-grained than with the CLIs.
- **Activity and events.** The Activity tab works unchanged: messages, reasoning summaries, commands, file changes,
  denied calls, usage, runner problems. `first_edit` keeps working because native edits emit `FileChanged`.
- **Platforms.** Windows and macOS from the first phase. No platform-only HTTP or TLS dependency.
- **Secrets.** The API key lives in the OS keychain as a project secret. It is read only by the runner. It never
  reaches logs, events, prompts, tool output, command environments, errors or `Debug` output.

### Non-goals (deliberately left out)

- **Server-side tools.** Anthropic web search, web fetch and code execution, and OpenAI hosted tools run outside
  Orchester's permission model. They are never declared in a request.
- **Other input and agent forms.** No vision or image input, computer use, subagents, or batch processing. No fast
  mode.
- **Other sign-in and cloud clients.** No sign-in with a Claude or ChatGPT subscription; native connections use API
  keys only. No Bedrock, Vertex or Foundry clients.
- **Other wire formats and caps.** No OpenAI Chat Completions wire format before P3 is done (it follows as P3b, see
  the [OpenAI recommendation](#32-openai-responses)). No money-based budgets or cost estimates: limits stay in tokens,
  as today.
- **Later phases.** MCP passthrough and memory tools come in P4; OS-level command sandboxing comes in P5.

## 2. Crate layout

```
orchester-core      native/   provider-neutral domain: conversation items, tool catalog and typed inputs,
                              path and command policy, edits, output limits, the loop state machine,
                              the ModelClient trait, capability rows. No HTTP, no tokio, no file IO.
orchester-agent     (new)     model clients over HTTP + SSE (Anthropic Messages, OpenAI Responses),
                              endpoint quirks, retries, model listing and key check, file/search tool executors.
orchester-runner    step/native/  drives one native invocation: loop, approvals, commands, persistence, events.
orchester-store     native conversation tables and the NativeTranscriptStore implementation.
orchester-desktop   connection forms, inspector, enforcement panel rows (no HTTP).
```

**Why a new crate.** It is a real boundary. The HTTP client, TLS, SSE and provider wire formats must stay out of core
(AGENTS.md), and out of the desktop. The future server needs them without the desktop. The runner and later the server
depend on `orchester-agent`; the desktop does not. The desktop asks the runner to test a connection, so API keys are
read in one process only.

**Why the loop lives in two places.** Deciding what to do next is a sans-IO state machine in core. Given a model
response it returns the tool calls to plan; given a tool outcome it decides what comes next. This is where permissions,
staleness checks, result validation and stop modes live, and it is tested in core with no network. Doing the work is
thin async glue in the runner, which already owns the database, vault, approvals, notifier and process containment.

### 2.1 Core types (`crates/core/src/native/`)

```rust
// model.rs — the provider boundary (AGENTS.md: providers are reached through core traits)
pub trait ModelClient: Send + Sync {
    /// One model response. `events` receives text, reasoning and tool-call progress as it streams.
    fn respond(&self, request: &ModelRequest, events: &mut (dyn FnMut(StreamEvent) + Send))
        -> impl Future<Output = Result<ModelResponse, ModelError>> + Send;
    fn models(&self) -> impl Future<Output = Result<Vec<RemoteModel>, ModelError>> + Send;
}

pub struct ModelRequest { pub model: ModelId, pub effort: Option<Effort>, pub max_output: u32,
    pub system: SystemPrompt, pub tools: ToolSet, pub items: Vec<Item>, pub cache_key: ConversationId }
pub struct ModelResponse { pub items: Vec<Item>, pub stop: StopReason, pub usage: TokenUsage,
    pub served_by: ModelId /* differs from request.model after a provider fallback */ }
pub enum StopReason { EndTurn, ToolUse, MaxOutput, ContextExceeded, Refusal { category: Option<String> }, Filtered }
pub enum ModelError { KeyRejected, Billing, Forbidden, ModelNotFound(ModelId), TooLarge, RateLimited { retry_after: Option<Duration> },
    Overloaded, Server(u16), Transport(String), Protocol(String), InvalidRequest(String) }

// item.rs — append-only conversation; provider payloads kept verbatim
pub enum Item {
    User { text: String, origin: UserOrigin },          // prompt parts, CONTINUE_PROMPT, answers, permission notes
    Assistant { text: String },
    Reasoning { summary: Option<String>, opaque: ProviderPayload }, // thinking block / encrypted reasoning, never shown raw
    ToolCall { id: ToolCallId, name: ToolName, input: serde_json::Value },
    ToolResult { id: ToolCallId, content: String, is_error: bool },
    Compaction { summary: String },                       // starts a new segment (§6.3)
}
pub struct ProviderPayload { pub wire: WireApi, pub json: Box<serde_json::value::RawValue> }

// tools.rs — the native tool catalog (§4); inputs are typed structs with deny_unknown_fields
pub enum ToolName { ReadFile, ListFiles, Search, EditFile, WriteFile, RunCommand, FinishStep, AskQuestion }
pub enum ToolInput { ReadFile(ReadFile), ListFiles(ListFiles), Search(Search), EditFile(EditFile),
    WriteFile(WriteFile), RunCommand(RunCommand), FinishStep(StepResult), AskQuestion(AskQuestion) }
pub fn parse_call(name: &str, input: &serde_json::Value) -> Result<ToolInput, ToolInputError>;

// scope.rs — FileScope: pure checks over paths the executor has already resolved (§5.1)
// command_policy.rs — CommandVerdict { Run, Ask(ApprovalAsk), Refuse(RefusalReason) } (§5.2)
// edit.rs — apply_edits(text, &[Edit]) -> Result<String, EditError>
// limits.rs — OutputLimits, truncate_lines/head_tail
// session.rs — the loop state machine
pub struct NativeLoop { /* conversation, policy, read stamps, pending calls, stop mode */ }
pub enum LoopInput { Responded(ModelResponse), ModelFailed(ModelError), ToolDone(ToolCallId, ToolOutcome),
    Decided(ToolCallId, bool), Stop(StopMode) }
pub enum LoopAction { CallModel(ModelRequest), Run(Vec<PlannedTool>), Ask(ApprovalAsk), Persist(Vec<Item>),
    Emit(Vec<AgentEvent>), End(LoopEnd) }
pub enum LoopEnd { Result(StepResult), Question(String), Stopped(StopMode), NoResult, Failed(LoopFailure) }
pub trait NativeTranscriptStore { type Error; /* append_items, items(conversation), conversation(id) */ }
```

`AgentProvider` becomes `{ CodexCli, ClaudeCodeCli, AnthropicApi, OpenAiApi }`, stored as `anthropic_api` /
`openai_api`. It gets `pub const fn runtime(self) -> AgentRuntime` with `AgentRuntime { Cli { program: &'static str },
Native(WireApi) }`, and `WireApi { AnthropicMessages, OpenAiResponses }`.

`Connection.executable: PathBuf` becomes `endpoint: AgentEndpoint`, with `AgentEndpoint { Program(PathBuf), Api(ApiEndpoint) }`.
Absolute-path validation stays on `Program`. `ApiEndpoint` is described in §7. The fan-out the closed enums cause is
listed in §9 (P1 size).

### 2.2 Where it plugs into the runner

`RunTask::start` keeps its order: take an agent slot, workspace, `start_invocation`, prompt, permissions,
`spawn`. `step/command.rs` (572 lines) is split first, into `step/start/{cli.rs, shared.rs}` plus a new
`step/start/native.rs`. That returns `AgentStart::Native(NativeStart)` instead of a `Command`.

The worker stops being tied to a child process:

```rust
enum Driver {
    Process { process: Supervised, output: AgentStream, app_server: Option<AppServerSession> }, // today's fields
    Native(NativeHandle),
}
struct NativeHandle { updates: mpsc::Receiver<NativeUpdate>, control: mpsc::Sender<NativeControl>, task: JoinHandle<()> }
enum NativeUpdate { Activity(AgentEvent), Approval(NativeAsk), Ended(LoopEnd) }
enum NativeControl { Stop(Stop), Decision { call: ToolCallId, approve: bool } }
```

`worker.rs` (465 lines) is split into `worker/{mod.rs, process.rs, native.rs}` before this lands. The `select!` loop
reads `Driver::Native` updates where it reads process events today. Everything downstream is shared: `observe`
(redaction, `InvocationTracker`, `FirstEditTracker`, budget checks), `flush` (`append_activity`, notifier), approvals
and `finish`.

The native task runs inside the runner's tokio runtime. It holds the model client, the `NativeLoop`, the
`FileTools` executor and a `CommandHost`; the `CommandHost` starts commands with today's `Supervised`, so the Windows
job object and the macOS process group apply to every command.

| Concern | How it works for a native invocation |
|---|---|
| Events | The loop emits ordinary `AgentEvent`s. `SessionStarted{session: conversation id}`. `TurnStarted`/`TurnCompleted{usage}` around **each model response**. `Message`, `Reasoning` (summaries only). `CommandStarted`/`CommandFinished` for every tool call (`read_file src/a.rs:1-200`, `run_command cargo test`). `FileChanged` after edits and writes. `PermissionDenied` for refusals. `RunnerProblem` for retries given up, compaction and recoveries. No new `AgentEvent` variant in P1. |
| Usage | `TurnCompleted{usage}` per response. `WorkerEvent::Usage` → `run.record_usage` as today. Anthropic: `input = input_tokens + cache_creation_input_tokens + cache_read_input_tokens`, `cached_input = cache_read_input_tokens`, so cache writes count, as `docs/design` § Token budgets requires. OpenAI: `input = input_tokens` (cached included), `cached_input = input_tokens_details.cached_tokens`, `output = output_tokens` (reasoning included). |
| Budgets | `Stop::AfterTurn` is checked before every model call: the current response finishes, its tool calls are **not** run, and the loop ends `Stopped`. The 5 s `TURN_GRACE` timer is not needed for native. |
| Time limit | `Stop::Now` drops the in-flight HTTP future and terminates a running command through `Supervised`. Active time excludes approval waits (`InvocationTiming`), as today. |
| Pause | `Stop::AtApproval` while waiting ends without granting. `Stop::AfterCommand` lets the running command finish and records its result. The runner then commits the checkpoint as for the CLIs (`step/end.rs` unchanged). |
| Cancel | `Stop::Now(Requested)`. |
| Result | On `LoopEnd::Result`, the runner emits the validated result JSON as the last `Message{text}`. `StepOutput::from_final_message` and `end.rs` stay unchanged. |
| Outcome | `Result`/`Question` → `Completed`; `Stopped` → `Terminated`; `NoResult` → `NoResult`; `Failed(KeyRejected)` → `NotSignedIn{message}`; other failures → `TurnFailed{message}`. This is a small `InvocationOutcome::from(&LoopEnd)` instead of `tracker.outcome(exit_code)`. |
| Resume | `Invocation.session` holds the conversation id. A new invocation with `InjectionEvent::Resume` loads the stored items and appends. Calls left unanswered by a stop get the result *"Not run: the step was stopped before this call ran. Call it again if it is still needed."* Then the resume prompt (`CONTINUE_PROMPT`, revise note or messages) follows as a new user item. |
| Question | `ask_question` ends the invocation with `LoopEnd::Question`, and the step waits (`WaitReason::Message`). The person's answer becomes the `ToolResult` of that pending call when the conversation resumes, so the model sees it as the tool's answer. |
| Crash or interrupt | Items are persisted before the action they describe completes (§6.2), so continue-from-checkpoint works like a resume. |

## 3. Model clients (`orchester-agent`)

```rust
pub enum NativeClient { Anthropic(AnthropicClient), Responses(ResponsesClient) } // both impl core::native::ModelClient
pub struct Endpoint { base_url: BaseUrl, key: Arc<SecretValue>, quirks: WireQuirks, user_tag: ProjectTag }
pub async fn check(endpoint: &Endpoint) -> ApiCheck; // GET models; free, no tokens spent
```

### 3.0 HTTP stack

- **`reqwest`** (async, on the existing tokio runtime), with `default-features = false` and rustls (the `ring`
  provider) using the operating system's trust store. Corporate TLS-inspecting proxies work on Windows and macOS
  without OpenSSL. Feature names depend on the version chosen *(to verify)*.
- It honours `HTTPS_PROXY`/`NO_PROXY`, which the runner already passes through.
- One new workspace dependency, with the reason recorded in `Cargo.toml`: a TLS-capable HTTP/1.1 and HTTP/2 client
  is not "a few lines of code". `hyper` is already in the lock file through lurq, so the transitive cost is small.
- **No SSE crate.** A ~100-line parser in `agent/src/sse.rs` handles `event:`, multi-line `data:`, and `:` comment
  lines. Comments are DeepSeek keep-alives and must be ignored.
- **No JSON-schema crate.** Tool inputs are validated by deserializing into typed structs (`deny_unknown_fields`).
- **The key is never exposed.** It is a `HeaderValue` marked sensitive, built per request from `Arc<SecretValue>`.
  Transport errors pass through `redact_secrets` with the key as a `SecretVariable` before they become `ModelError`.
- **Timeouts.** Connect timeout 30 s. No total timeout on a stream. Instead an idle timeout: 120 s without a byte
  (keep-alive comments count as bytes). Then the attempt fails as `Transport` and is retried.

### 3.1 Anthropic Messages

Rust has no official Anthropic SDK, so the skill's guidance for raw HTTP applies.

**Request.**
- `POST {base}/v1/messages`, default base `https://api.anthropic.com`.
- Headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`, and `anthropic-beta` only
  for the betas listed below.
- Always `stream: true`. `max_tokens` defaults to 64000, capped by the model's `max_tokens` from the Models API.
  Thinking counts toward it.

**Models and thinking.**
- Default model `claude-opus-5-5`. The catalog comes from `GET /v1/models`, using `id`, `display_name`,
  `max_input_tokens`, `max_tokens` and `capabilities`. A fallback list uses the skill's table:
  `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-fable-5-1`, `claude-haiku-4-5`. Those are the exact ids, with no
  date suffixes.
- Thinking is configured per model from `capabilities`:
  - **Adaptive** where `capabilities.thinking.types.adaptive.supported`, by sending `thinking: {type: "adaptive",
    display: "summarized"}`. Summaries become `Reasoning` events. Opus 5.5 and Fable 5.1 cannot disable thinking:
    `{type: "disabled"}` and `budget_tokens` return 400. `display: "updates"` (beta
    `thinking-display-updates-2026-08-18`) is a later option for live progress lines.
  - **Haiku 4.5** has no adaptive thinking and no effort. It runs without thinking, with the effort control hidden.
- **Effort** goes in `output_config.effort` (`low`, `medium`, `high`, `xhigh`, `max`, per `capabilities.effort`).
  Always send it explicitly. The default differs: Opus 5.5 defaults to `medium`, Sonnet 5.5 to `high`. Effort is
  pinned per conversation, because changing the top-level value invalidates the message cache.

**Tools and tool choice.**
- Client tools use `name`, `description`, `input_schema` and `strict: true`. The schemas in §4 already meet strict's
  rules: `additionalProperties: false`, every field required, optional fields nullable.
- On the official endpoint, each tool also sends `eager_input_streaming: true`, so large `write_file`/`edit_file`
  inputs stream instead of stalling. The client then owns validation:
  - parse the accumulated JSON strictly, then deserialize into the typed input;
  - a failure becomes `ToolResult{is_error: true, content: {"INVALID_JSON": "<raw>"}}`, built with serde, never by
    string concatenation;
  - check `stop_reason` before running any tool. `max_tokens` re-asks the same turn with room once, and `refusal`
    runs no tool.
  - Custom base URLs leave `eager_input_streaming` off (§3.3).
- `tool_choice` stays `auto`. Opus 5.5, Sonnet 5.5 and Fable 5.1 reject `any`/`tool` with 400. `finish_step` is
  therefore required by the prompt plus a check (§4.7), never forced.
- Parallel tool use stays on. Every `tool_result` of a response goes back in **one** user message, as the skill
  requires.

**Streaming.**
- Events: `message_start` (id and input usage), `content_block_start`, `content_block_delta` (`text_delta`,
  `thinking_delta`, `input_json_delta`), `content_block_stop`, `message_delta` (`stop_reason`, output usage),
  `message_stop`, `ping`, and an `error` event mid-stream.
- Every content block is accumulated exactly as streamed, including thinking signatures. Delta event names beyond
  these are *to verify*.
- A response is appended to the conversation only after `message_stop`. A broken stream discards the partial
  response and the request is retried. No tool has run at that point, so a retry is safe.

**Preserved thinking.** Thinking blocks are passed back unchanged, and the history is strictly append-only:
- the frozen `system` and `tools` never change within a conversation;
- injected instructions are appended, never edited in;
- a result is never deleted.

Opus 5.5 and Fable 5.1 bind each block to the exact prefix. For accounts created on or after 2026-08-31, an edited
history is a 400. The client sends `thinking.block_binding.prefix_mismatch_behavior: "error"` with beta
`thinking-binding-controls-2026-08-01` on the official endpoint, so a harness bug fails loudly in tests. In production,
such a 400 triggers the documented one-time recovery: strip every `thinking` block from the history (text and tool
calls stay), retry once, and record a `RunnerProblem`. The model must stay the same within a conversation. A changed
profile model starts a new conversation segment with a compaction (§6.3).

**Prompt caching.** The cache is a prefix match in the order tools → system → messages.
- Tools are serialized deterministically, sorted by name, and never vary within a conversation.
- `system` holds the frozen Start prompt (§6.1). It never includes timestamps or ids, and carries one explicit
  `cache_control: {type: "ephemeral"}` breakpoint.
- A top-level `cache_control: {type: "ephemeral"}` (automatic caching) covers the growing tail. This is the skill's
  "robust combination for agent loops".
- Each response appends one tool-use run and one tool-result run, well within the 20-position lookback.
- The TTL is 5 minutes: turns start well under 5 minutes apart. An approval wait longer than that costs one cold
  read. P1 measures whether the 1-hour TTL pays off; that is an engineering check, not a product decision.
- Orchester instructions injected mid-run are appended as `{"role": "system"}` messages on models that support them
  (Opus 5/5.5, Fable 5.1, Sonnet 5.5). On others they go as a user text item. They are never edited into earlier
  turns.
- A standing test asserts `cache_read_input_tokens > 0` on the second request against the fake server's echo of
  cache fields, plus a live check (§8).

**Refusals.**
- `stop_reason: "refusal"` (HTTP 200, `stop_details.category`) is checked before content. The turn's tools are not
  run, and the loop ends `Failed(Refused{category})`. The step fails and the refusal is shown as a problem with its
  category (`RunnerProblem::ModelRefused{category}`, new).
- **Provider fallback is off by default** (decision 4). The skill recommends `fallbacks: "default"` with beta
  `server-side-fallback-2026-07-01` for Opus 5.5, Sonnet 5.5 and Fable 5.1; the official-endpoint connection offers it
  as *Retry declined requests on another model*, unchecked. When a person turns it on, `served_by` records the model
  that answered and the Activity shows the switch; the fallback model runs without the earlier thinking blocks, which
  the API drops.

**Errors and retries.** Every status maps to a typed `ModelError`:
- **400** `InvalidRequest`, except the preserved-thinking recovery above. **401** `KeyRejected`. **402** `Billing`.
  **403** `Forbidden`. **404** `ModelNotFound`. **413** `TooLarge`, which triggers compaction once and then fails.
- **429**, **500**, **529** and transport errors are retried with exponential backoff and jitter, honouring
  `retry-after`: 2 s, 4 s, 8 s, 16 s, 32 s, at most 5 attempts.
- Only active-time-limit room is spent while waiting. Each wait is a `RunnerProblem` entry, so a silent stall is
  impossible.
- Context exhaustion (`model_context_window_exceeded`, or a 400 naming the context) leads to compaction (§6.3).

**Not in P1.**
- `output_config.task_budget` (beta `task-budgets-2026-03-13`, at least 20k) is not sent (decision 9). It is revisited
  after P1, with P1's usage data.
- Server-side compaction (beta `compact-2026-01-12`) on the official endpoint is a later option. P1 uses the
  provider-neutral client-side compaction.

### 3.2 OpenAI Responses

The Responses details below come from the developers.openai.com reference fetched 2026-10-01.

**Request.**
- `POST {base}/responses`, default base `https://api.openai.com/v1`.
- `Authorization: Bearer <key>`. `stream: true`. `max_output_tokens` is always set explicitly.

**Stateless use.**
- `store: false` and `include: ["reasoning.encrypted_content"]`.
- `previous_response_id` is never used. The full item list is sent each turn: reasoning items (with
  `encrypted_content`), `function_call` (`call_id`, `name`, `arguments` as a JSON string) and `function_call_output`
  (`call_id`, `output`).
- Stateless means one code path for OpenAI and DeepSeek, which does not keep responses. Resume and compaction stay
  under Orchester's control, and no conversation is stored at the provider.

**Instructions, tools and reasoning.**
- `instructions` carries the frozen system prompt. Mid-run Orchester notes become `developer` messages *(to verify
  that `developer` items are accepted mid-list)*.
- Tools: `{type: "function", name, description, parameters, strict: true}`. Strict requires
  `additionalProperties: false` and every field in `required`, with optional fields typed as `[T, "null"]`. The §4
  schemas are written that way.
- `tool_choice: "auto"`, `parallel_tool_calls: true`.
- `reasoning: {effort, summary: "auto"}`. The accepted effort and summary values per model are *to verify*. The model
  list (`GET /models`) carries no effort or context metadata, so these come from a small built-in table or from
  manual entry.

**Caching.** Caching is automatic for repeated prefixes. `prompt_cache_key` is set to the conversation id. The same
append-only discipline as §3.1 keeps hits high.

**Streaming.** Confirmed event names:
- `response.created`, `response.output_item.added`, `response.output_item.done`;
- `response.function_call_arguments.delta`, `response.function_call_arguments.done`;
- `response.completed`, `response.incomplete`, `response.failed`.

Text and reasoning-summary delta names (`response.output_text.delta`, `response.reasoning_summary_text.delta`) are
*to verify*. The response is appended only after `response.completed` or `response.incomplete`.

**Stop and errors.**
- `status: "incomplete"` with `incomplete_details.reason`: `max_output_tokens` maps to `MaxOutput` and
  `content_filter` maps to `Filtered`.
- HTTP errors map as in §3.1. The `retry-after` header and rate-limit header names are *to verify*.

**Usage.** `input_tokens` (cached included), `input_tokens_details.cached_tokens`, `output_tokens` (reasoning
included in `output_tokens_details.reasoning_tokens`). This fits `TokenUsage` exactly.

**Recommendation: Responses, not Chat Completions.**
- Responses carries reasoning items across a tool loop, which OpenAI reasoning models need.
- It is the wire format Codex itself uses, and DeepSeek serves it statelessly.
- A Chat Completions client would be a third wire format, with its own reasoning, usage and tool-call shapes.

Chat Completions matters for self-hosted servers that do not offer Responses. It is added after P3 as
`WireApi::OpenAiChat` (phase P3b, decision 5), behind the same `ModelClient` trait and fake-server tests.

### 3.3 Compatible endpoints and provider quirks

A native connection has a **preset**: `Official`, `DeepSeek` or `Custom`. The preset fills `WireQuirks`. The client
never branches on host names.

```rust
pub struct WireQuirks {
    pub betas: bool,                  // anthropic-beta headers (fallbacks, block binding, display updates)
    pub eager_input_streaming: bool,  // proxies and compatible servers may reject the field
    pub cache_control: bool,
    pub provider_fallbacks: bool,
    pub models_have_metadata: bool,   // context window / efforts from GET models
    pub explicit_max_output: u32,     // always sent; some defaults are small
    pub error_text_in_results: bool,  // prefix failed tool output with "Error:" because is_error may be ignored
    pub user_tag: Option<UserTagField>,
    pub effort_values: EffortValues,
}
```

`Custom` is conservative: no betas, no eager streaming, no fallbacks, model metadata entered by hand, explicit max
output, error text in results. Every preset puts the error text in results anyway; it costs nothing.

**DeepSeek** (preset `DeepSeek`; from `.tmp/deepseek/`, fetched 2026-10-01):

**Models and endpoints.**
- Models: `deepseek-flash` and `deepseek-v4-pro`. Both have a 1M context and up to 384K output, with thinking on by
  default. `deepseek-chat` and `deepseek-reasoner` were retired on 2026-07-24.
- One key works on three wire formats:
  - OpenAI Chat;
  - OpenAI **Responses** at `https://api.deepseek.com`;
  - Anthropic Messages at `https://api.deepseek.com/anthropic`.
- **The preset uses Responses**, stateless. DeepSeek's Responses has no `previous_response_id` and runs as
  `store=false`. It supports function tools and `json_schema`, ignores hosted tools, and treats `developer` as `user`.
- The Anthropic route is the weakest:
  - it silently maps unknown model names to `deepseek-flash`, and `claude-opus*` names to `deepseek-v4-pro`;
  - it ignores `is_error`, `cache_control` and `anthropic-beta`;
  - it has no schema output.

  It is reachable only through `Custom`, with a warning.

**Wire rules.**
- **Reasoning must be resent on every turn of a tool loop.** A missing reasoning item returns 400. The stateless
  design already resends it, and the stored items keep it for resume.
- No forced `tool_choice` in thinking mode (400). Orchester never forces anyway.
- **Effort mapping:** `medium` runs as `high`, and `xhigh` runs as `high`. The model picker shows the effort levels
  that actually differ (`low`, `high`, `max`).
- Always send `max_output_tokens`: the defaults are small.

**Usage.** Input includes cached tokens. Responses reports `input_tokens_details.cached_tokens`; Chat reports
`prompt_cache_hit_tokens`. Caching is automatic, on disk and prefix-based, so append-only histories hit.

**Limits and errors.**
- Limits are on concurrency, so a 429 means back off.
- 402 means the balance is empty. It is terminal, with a clear status.
- 401 is a bad key. 400/422 are request bugs.
- 500/503, and `insufficient_system_resource` or `aborted` finish reasons, are retried.
- Keep-alive SSE comments arrive while a request is queued. The server drops a request after 10 minutes in the
  queue; the idle timeout and retry cover that.

**Connection setup.**
- Send a per-project, non-identifying user tag. The field name per wire format is *to verify*.
- Validate model ids against `GET /models`.
- `GET /models` and `GET /user/balance` are free, so *Test connection* uses them and shows the balance.

**Data location.** DeepSeek's privacy policy stores data in the PRC. The inspector says so, and the first use in a
project needs a recorded acknowledgement (§7). The full notes are in `.tmp/deepseek/`.

**Other Anthropic- or OpenAI-compatible servers** use `Custom`. *Test connection* lists models where `GET models`
works. Otherwise the model list is manual and the status says *Models not listed by the endpoint*.

## 4. Tools

All tool schemas are strict-compatible: `additionalProperties: false`, every property in `required`, optional
properties nullable. Paths are relative to the worktree root. Absolute paths are accepted only inside the readable
scope (§5.1). Every result is plain text with a trailing *truncated* note when cut (§6.4). A tool failure is returned
as an error result so the model can recover. It never ends the loop, except the cases named below.

### 4.1 `read_file`
`{path: string, offset: integer|null, limit: integer|null}`
- `offset` is the 1-based start line and `limit` is the number of lines. The defaults are line 1 and 2000 lines.
- Output is numbered lines; each line is cut at 2000 characters.
- Binary files (a NUL byte in the first 8 KiB) return `binary file, N bytes`.
- Readable scope only. The read records a **read stamp**: path, length, modified time and a content hash.

### 4.2 `list_files`
`{pattern: string|null, path: string|null}`
- `pattern` is a glob relative to `path`, which defaults to the worktree root.
- Respects `.gitignore` by delegating to `rg --files --glob`, the ripgrep Orchester already ships. Without ripgrep it
  falls back to a plain directory walk with the same exclusions as the code index.
- Sorted output, at most 1000 entries.

### 4.3 `search`
`{pattern: string, path: string|null, glob: string|null, case_insensitive: boolean|null, context: integer|null}`
- Runs `rg` with an argument vector Orchester builds: `--json`, `--max-count`, `--max-columns 500`, and never `--pre`
  or `--search-zip`.
- `path` may point into the code index folder, which is how the model searches `symbols.tsv` and outlines. The prompt
  names those files (§6.1).
- At most 200 matches or 64 KiB. Without ripgrep: `RunnerProblem::RipgrepNotFound` and a slower built-in literal
  search.

### 4.4 `edit_file`
`{path: string, edits: [{old: string, new: string, replace_all: boolean|null}]}`
- **One element is a plain edit; several make an atomic multi-edit.** Edits are applied in order to the in-memory
  text, and nothing is written if any edit fails.
- `old` must occur exactly once unless `replace_all`. Zero or several matches return an error naming the count.
- **Staleness check.** The file must have a read stamp from `read_file` or the agent's own last write, still matching
  the file on disk. Otherwise the error says *the file changed since you read it; read it again*. Commands the agent
  ran can change files, so the stamp is re-checked at edit time.
- **Atomic write.** The new text is written to a temporary file in the same folder, flushed, then renamed over the
  target. Line endings are preserved, along with the BOM when present. A rename replaces the directory entry, so an
  edit never writes *through* a hard link or symbolic link that leaves the scope (§5.1).
- Emits `FileChanged{Modified}`.

### 4.5 `write_file`
`{path: string, content: string}`
- Creates a file, along with missing parent folders inside the writable scope, or replaces one.
- Replacing an existing file needs a valid read stamp, as for edits.
- Same atomic write. Emits `FileChanged{Added|Modified}`.

### 4.6 `run_command`
`{command: string, shell: boolean|null, workdir: string|null, timeout_seconds: integer|null}`
- **Direct (default).** `command` is tokenized by core: POSIX-style quoting on macOS, Windows `CommandLineToArgvW`
  rules on Windows.
  - Shell operators are rejected with a hint to use `shell: true`: `;`, `&`, `|`, `<`, `>`, `$(`, backticks and
    redirections.
  - The program is resolved on the agent `PATH` (ripgrep's folder first) through `exec::launch`, then run without a
    shell. This is what makes classification sound.
- **Shell.** `shell: true` runs the string in the platform shell:
  - Windows: `pwsh -NoProfile -NonInteractive -Command` when `pwsh` is found on the agent `PATH`, otherwise Windows
    PowerShell 5.1, `powershell.exe -NoProfile -NonInteractive -Command`;
  - macOS: `/bin/zsh -c` (decision 3).

  The shell is resolved once per invocation and named in the system prompt.

  A shell command is never known-safe and never matches the allowed list. It needs a person (§5.2).
- **Working directory and timeout.** `workdir` must be inside the worktree. `timeout_seconds` defaults to 120, with a
  maximum of 1800, bounded by the remaining active time.
- **Process and environment.** Started with `Supervised` (job object / process group). Stdin is closed. The
  environment is `agent_environment(...)` plus the project's plain variables. It contains no secrets and no API key,
  and the `ANTHROPIC_*`/`OPENAI_*` variables are withheld as today.
- **Output** goes to the model as head plus tail, 32 KiB in total, with `omitted_bytes`. The full output, capped at
  1 MiB, goes to Activity in `CommandFinished`.
- Exit code and duration are included in the result. A timeout terminates the tree and says so.

### 4.7 `finish_step`
- Input schema is exactly `STEP_RESULT_SCHEMA`, which is already strict-compatible.
- Validated by deserializing into `StepResult` (strict on `outcome`/`summary`, lenient lists as today).
- **Ending.** A valid call ends the loop with `LoopEnd::Result`. The tool result is never sent back, so the model is
  not asked again. An invalid call gets an error result naming the field, and the loop continues.
- **Missing call.** If the model ends its turn (`end_turn`) without calling `finish_step`, Orchester appends one
  reminder: *"Call finish_step with your result."* After a second miss it tries `StepResult::at_end` on the final
  text, then ends `NoResult`.
- **Gated steps.** `GATED_RESULT_INSTRUCTIONS` apply unchanged.
- **Portability.** This is how Claude Code's `StructuredOutput` works, and it is portable to every provider. No
  provider-specific structured-output feature is needed.

### 4.8 `ask_question`
`{question: string, options: [string]|null}`
- Ends the invocation with `LoopEnd::Question`. The step shows the question as it does for an `outcome: question`
  result.
- The answer resumes the conversation as this call's tool result (§2.2).
- Use it when the agent needs an answer to continue. `finish_step` with `question` remains for "I stopped and here is
  why".

### 4.9 Later tools
- **P4, MCP passthrough.** Tools from the profile's project MCP servers, named `mcp__<server>__<tool>` with the
  server's schema.
  - The runner acts as the MCP client: stdio or streamable HTTP, secrets from the vault as today. `rmcp` is already in
    the lock file through lurq.
  - Decided (decision 6): for native connections the runner talks to the server itself. The design rule in
    `docs/design/orchester-workspace.md` (MCP servers) now says so; CLI agents keep starting their servers themselves.
  - Calls follow `interactive_tool_approvals`.
- **P4, memory tools.** `memory_search {query}` reads active records (FTS5, as recall does). `memory_propose {kind,
  text}` creates a *proposed* record and is screened against keychain secrets like handoffs.

### 4.10 Concurrency

Read-only calls (`read_file`, `list_files`, `search`) in one response run concurrently. Writes, edits, commands and
questions run one at a time in the order given, after the reads. A refused or denied call does not stop the calls
before it, and its result explains the refusal.

## 5. Permissions and sandboxing

The native agent has no CLI sandbox. Every guarantee is either enforced by Orchester in the tool executors, or locked
as unavailable. A guarantee is never assumed.

### 5.1 File tools: confinement by Orchester

**Scopes.** The readable scope is:
- the worktree;
- the code index folder (`.orchester/index/<revision>`, read-only);
- the step's skills folder (`.orchester/skills/<execution>`, read-only);
- writable folders.

The writable scope is the worktree, only when repository access is *Edit files in the task worktree*, plus the
granted writable folders (`WritableFolders::resolve` rules unchanged).

**Protected paths, never writable:**
- `.git` (file or folder) at any depth;
- `.orchester/` inside the worktree;
- the code index;
- the skills folder.

Writing `.git` could redirect the worktree's git directory or plant hooks that Orchester's checkpoint commit would
run.

**Resolving a path** (`agent::files::resolve`, platform details behind `cfg`):
1. Refuse NUL characters, `\\?\` and device paths (`\\.\`, `CON`, `NUL`, …), alternate data streams (`file:stream`)
   and UNC paths, unless they lie inside a granted writable folder.
2. Join a relative path to the worktree. Normalise `.` and `..` lexically, then **canonicalise the longest existing
   ancestor** with the OS: `std::fs::canonicalize`, then strip `\\?\` on Windows with our own few lines.
3. Compare the canonical path against the **canonicalised** scope roots, component by component. Comparison is
   case-insensitive on Windows. On macOS it follows the volume's case sensitivity *(to verify on a Mac;
   `/private/var` vs `/var` is handled because both sides are canonicalised)*.
4. Symlinks and junctions are followed by canonicalisation, so a link inside the worktree that points outside is
   refused for reads and writes.
5. Writes go through the atomic temp-and-rename (§4.4), which replaces the final link or hard link instead of
   writing through it. The parent folder is canonicalised, and must be in scope, at write time.

**Remaining risk.** A command the agent runs could swap a folder for a junction between the check and the write.
Commands and file writes never run at the same time (§4.10), which leaves only processes the agent started earlier
that still run in the background. Job objects and process groups end those with the command on Windows and macOS,
with the documented `setsid` gap on macOS. This is the same class of gap the design already lists.

### 5.2 Commands: agents ask

`CommandPolicy::classify(&ParsedCommand, &EnforcedPermissions) -> CommandVerdict`, pure, in core:

1. **Shell** (`shell: true`) → `Ask`.
2. **Allowed list.** `AllowedCommands::allows(CommandPrefix)`, the existing whole-word prefix match on the
   re-joined argv → `Run`.
3. **Known-safe read-only** → `Run`. This is a new core table, `permission/known_safe/`, modelled on Codex's
   `is_known_safe_command` *(to verify against the current Codex source)*. Its entries are argv rules, not prefixes.
   Every program and every git subcommand admits **only the options it lists**, by exact name; a short-option cluster
   only from listed letters. Any other option, an abbreviation of a listed one included (git's parse-options and
   `getopt_long` accept any unambiguous prefix, so `git grep --op=<cmd>` would be `--open-files-in-pager`), makes
   the command ask. The lists leave out every option that starts a program, writes a file, reads a file of paths, or
   follows links into folders the path check does not see:
   - `git` with `status`, `log`, `diff`, `show`, `rev-parse`, `ls-files`, `branch` (list only), `blame` or `grep`.
     Every git call gets `-c core.fsmonitor=false --no-pager --no-optional-locks` (an fsmonitor setting can run a
     program on `git status`; the last keeps `git status` from rewriting `.git/index`). `diff`, `log` and `show` also
     get `--no-ext-diff --no-textconv`, and `blame`, which runs a textconv program by default, `--no-textconv`. Not
     listed are, among others, `--help` (starts a help viewer: a browser on Windows, `man` on macOS), `-c`,
     `--output`, `--ext-diff`, `--textconv`, `--open-files-in-pager` (`-O`) and `--show-signature`; the `%G…`
     format placeholders, which start `gpg` like `--show-signature`, are refused in any argument. Git still keeps
     its own files: `git diff` refreshes the file stamps cached in `.git/index`, without changing what the index
     holds.
   - `rg`, without `--pre`, `--pre-glob`, `--search-zip` (`-z`), `--hostname-bin` or `--follow` (`-L`).
   - `ls` (without `-L`), `cat`, `head`, `tail` (without `-f`), `wc` (without `--files0-from`), `pwd`, `echo`,
     `which`, `file` (without `-f`/`--files-from`, `-m`, `-C`, `-z`, `-s`, `-p`), `stat`.
   - Windows `where` with bare name patterns and `/Q`, `/F`, `/T` only: `$VAR:pattern` and `folder:pattern` search
     folders the path check does not see, and `/R` searches a folder tree.
   - `find` with start points and listed tests and printing actions (`-name`, `-type`, `-mtime`, `-newer`, `-print`,
     …), without leading options; so without `-exec`, `-execdir`, `-ok`, `-delete`, `-fprint*`, `-fls`, `-follow`,
     `-L` or `-files0-from`.
   - `cargo` with `metadata` or `tree`, and `--version` of any listed program.

   Every argument that resolves to an existing path must lie in the **readable** scope; otherwise the command is not
   known-safe. This stops `cat ~/.ssh/id_rsa`. The value after `=` of a long option and a value attached to a short
   option, also inside a cluster (`rg -nf<file>`), are checked as well; a drive-relative argument (`C:file`) is
   refused, as the file tools refuse it.
4. **Otherwise** → `Ask`. Every command that steps 2–3 do not run asks a person (decision 1). `Refuse` remains only
   for calls that can never run: an unparsable command, or a `workdir` outside the worktree. A refusal emits
   `PermissionDenied` and returns an error result; the loop continues.

**Mapping the existing `CommandPermission`** (the same meaning the CLIs have on branch `claude/ask-outside`):

| Setting | Native behaviour |
|---|---|
| `Allowed(list)` | Listed commands run. Known-safe read-only commands run once the list has an entry (plan C4). Every other command asks a person; with an empty list every command asks. In a read-only step the approval says that an approved command can change files. |
| `AskBefore` | Every command asks a person, known-safe ones too, as Claude Code does. |
| `AnyInWorktree` | Unavailable until P5. |

`run_command` is always offered, because a command can always reach a person.

**Approvals** reuse the existing path end to end:
- `PendingToolApproval{command: true, action: <command, redacted>, target: <workdir>, server: "Orchester"}`;
- id `<invocation>:<tool call id>`;
- `claim_tool_approval`, the "Needs you" modal, and `ToolApprovalNotAwaited`;
- the wait is excluded from active time.

The runner adds `Awaiting::Native{call, reply: oneshot::Sender<bool>}` next to `Awaiting::Claude`. **Approve once**
runs that one command. **Deny** does not fail the step (decision 2): the call gets the error result *"Denied by a
person. Do not run this command again; continue without it or explain in finish_step why you cannot."*, Activity
records `PermissionDenied`, the approval row ends `denied`, and the loop continues. The CLIs, where Deny still fails
the step, will be aligned later.

### 5.3 Network, environment and secrets

- **Model traffic** is Orchester's own HTTPS to the connection's endpoint. It is not "agent network", and the
  inspector labels it separately (§7).
- **Network.** The native agent has no web tools. Commands it runs have whatever network the OS gives them, the same
  as Claude's commands. **NetworkOff** is therefore unavailable on both platforms in P1. It degrades to On with
  `network_stays_on`, as for Claude, and the run summary says *Commands the agent runs can reach the network*. P5
  can make it enforced on macOS.
- **Environment.** Commands get the cleaned `agent_environment` plus the project's plain variables. Project secrets
  are never put in a command's environment; today secrets reach only MCP servers.
- **Secrets.**
  - The API key is read from the vault at invocation start (`Secrets::fetch` path, archived secrets refused) and kept
    as `Arc<SecretValue>`.
  - It is added as a `SecretVariable` to the worker's redaction set, so any echo in an error or event is masked.
  - Conversation items and tool outputs are redacted with the same set before they are stored or emitted.
  - Opaque provider payloads (thinking signatures, encrypted reasoning) are stored but never shown and never copied.

### 5.4 Capability table rows

`support(provider, platform, option)` gains the native rows. `Mechanism` gains `Native*` variants; the enforcement
panel title becomes *Enforced by Orchester*.

| Option | Native, P1 (Windows and macOS) | After P5 |
|---|---|---|
| ReadOnly | Enforced(`NativeWriteToolsWithheld`): no `edit_file`/`write_file`; writes refused by scope | same |
| EditWorktree | Enforced(`NativeFileScope`) | same |
| writable folders | `NativeFileScopeFolders` | same |
| AskBeforeCommands | Enforced(`NativeCommandApprovals`): every command asks | same |
| AllowedCommands | Enforced(`NativeCommandList`): listed and known-safe run, others ask | same |
| AnyCommand | **Unavailable(CannotConfineCommands)** | macOS: Enforced(`NativeSeatbeltCommands`) |
| NetworkOff | **Unavailable(CannotBlockNetwork)** → stays On | macOS: Enforced(`NativeSeatbeltNetworkOff`) |
| NetworkOn | Enforced(`NativeNetworkUnrestricted`) | same |

The panel text must state the limit honestly. A listed command, like a listed Claude command, is chosen by Orchester
but not confined, so it can write anywhere the user can. The mechanism text reads *listed and known-safe commands run
unconfined · others ask you · file tools confined to the worktree*. `mcp_support` for native in P1 means servers are left out with a
new `McpServerProblem::NotSupportedByConnection`, which is visible and does not fail the step.

### 5.5 OS sandboxing for commands (realistic now vs later)

| Platform | Option | Assessment |
|---|---|---|
| macOS | `sandbox-exec` with a generated SBPL profile: deny `file-write*` except the worktree, writable folders and temp; deny `network*` when off | Proven: Codex runs its commands this way *(to verify the current profile)*. `sandbox-exec` is deprecated but present. Realistic in P5a. Toolchains need read access to their own folders, which a deny-write profile keeps. |
| Windows | Job object limits (already per command: kill on close; can add process-count and memory caps) | Realistic now. Contains the process tree only, not files or network. |
| Windows | AppContainer with `internetClient` withheld and the worktree ACL'd to the container SID | Real file and network isolation, but toolchains break without access to the user profile (`~/.cargo`, `~/.rustup`, npm caches), and changing worktree ACLs is invasive. Research item, P5b. |
| Windows | Restricted token / low integrity + firewall rule per sandbox user | Codex's elevated Windows sandbox uses dedicated sandbox users and firewall rules *(to verify)*. Needs elevation once. P5b candidate. |

Every Windows option needs FFI. It would go in a dedicated `#![allow(unsafe_code)]` module with `// SAFETY:`
comments (rule 9), like `ipc/src/pipe_owner.rs`.

**The UI stays honest:**
- a mechanism appears in the table only when it is implemented and probed on this machine;
- the capability check becomes a per-machine probe for native, for example by running a tiny sandboxed
  `write outside` test once;
- until then the options stay locked with their reason.

## 6. Context management

### 6.1 Prompt assembly

The prompt reuses `PromptInput::compose` unchanged and splits its parts into two places. Neither carries timestamps
or ids.

- **`system`, frozen for the conversation:**
  - a short native preamble: the tools and when to call `finish_step`/`ask_question`, the OS, the shell
    `run_command` uses, and paths relative to the worktree;
  - then `AgentRole`, `ProjectRules`, `Pipeline`, `Step`.
- **First user item:** code index (map plus the names of `symbols*.tsv`, read with `read_file`/`search` instead of
  the internal ripgrep skill), skills listing (paths readable with `read_file`), task memory, `Task`, earlier results,
  plan revision, messages.
- **"When you finish."** `RESULT_INSTRUCTIONS` is swapped for a `ResultDelivery::Tool` variant that names
  `finish_step`. This is one new enum parameter on `compose`.
- **Resume parts** (`CONTINUE_PROMPT`, revise note, messages, recovery) are appended as a new user item. Orchester
  notes are appended as described in §3.1.
- `invocation_prompts` records the sources as today.

### 6.2 Conversation persistence (store schema)

There is one conversation per step execution, shared across its invocations: resume, question/answer, permission
restart. A retry attempt or follow-up starts a new conversation, because it is a new execution.

```sql
CREATE TABLE native_conversations (
  id TEXT PRIMARY KEY,                 -- UUIDv7, also Invocation.session
  run TEXT NOT NULL, execution TEXT NOT NULL,
  connection TEXT NOT NULL, wire TEXT NOT NULL, model TEXT NOT NULL,
  system_blob TEXT NOT NULL,           -- blob hash of the frozen system prompt
  tools_blob TEXT NOT NULL,            -- blob hash of the serialized tool set
  created_at INTEGER NOT NULL
);
CREATE TABLE native_items (
  conversation TEXT NOT NULL REFERENCES native_conversations(id),
  seq INTEGER NOT NULL,                -- append-only, gapless
  invocation TEXT NOT NULL,
  segment INTEGER NOT NULL,            -- increments at each Compaction item
  kind TEXT NOT NULL,                  -- user|assistant|reasoning|tool_call|tool_result|compaction
  body TEXT NOT NULL,                  -- JSON of Item; provider payloads verbatim; large bodies as blob refs
  created_at INTEGER NOT NULL,
  PRIMARY KEY (conversation, seq)
);
-- triggers forbid UPDATE and DELETE on both tables (append-only, as run_events)
```

**Write order.**
1. A model response is stored before its tools run.
2. A tool call's result is stored before the next model request.
3. A command's start is recorded through `CommandStarted`.

On resume, any call without a result gets the "not run" result (§2.2).

**Request rebuild.** The request is built from `system_blob`, `tools_blob` and the items of the latest segment. The
stored bytes are byte-identical to what was sent, which preserves caches and thinking binding.

**Where it lives.**
- Schema code goes in a new `store/src/project_db/native.rs`; `schema.rs` is at 583 lines, so its migration goes in
  a new module.
- `storage.md` gets the tables and the missing `tool_approvals` entry.
- Checkpoints need no agent-state blob for native. The conversation id in the invocation is enough, which answers the
  storage open question "what each provider can actually resume from" for native.

### 6.3 Compaction

**Trigger.** Before each request, the previous response's total input (`input` from §2.2) plus the size of the
items appended since, estimated at bytes/4 as memory already does, is compared with the model's context window:
- the window comes from Models API `max_input_tokens`, a manual value, or 200K when unknown;
- the threshold is 80% of the window, keeping room for `max_output`.

**Procedure: simple compaction**, the shape the skill recommends as safe with preserved thinking.
1. A fork request with the **same** model, `system` and tools (cache-friendly) adds one user item. It asks for a
   summary that keeps:
   - task and acceptance criteria;
   - decisions made, files changed and why;
   - commands run and their outcomes;
   - current plan and next action;
   - open problems;
   - exact identifiers the agent will need.
2. A `Compaction{summary}` item starts a new segment.
3. The next request sends: the first user item (task context), then the summary, then a note listing files edited so
   far, so read stamps restart and the agent re-reads before editing. Nothing from earlier segments is replayed, and
   no thinking blocks are replayed.
4. Never compact in the middle of a tool round. Compaction happens only after all results of a response are
   appended.

**Recording and limits.**
- Every compaction emits `RunnerProblem::ContextCompacted{before, after}`, a new variant shown in Activity.
- At most 3 compactions per invocation. After that the loop ends `Failed(ContextExhausted)` instead of looping.
- Server-side compaction (Anthropic beta) and OpenAI-side compaction events are later options.

### 6.4 Tool output limits

| Tool | To the model | To Activity |
|---|---|---|
| `read_file` | ≤ 2000 lines, ≤ 256 KiB, lines ≤ 2000 chars | command line only (`read_file path:1-200`) |
| `list_files` | ≤ 1000 entries | count |
| `search` | ≤ 200 matches / 64 KiB | count |
| `run_command` | head + tail 32 KiB with omitted byte count | output ≤ 1 MiB, exit code |
| any result | hard cap 128 KiB after the tool's own limit | — |

Truncation notes are part of the result text so the model can narrow its request.

## 7. Connections UI and data

**Connection kinds** in *Add connection*: *Codex CLI*, *Claude Code CLI*, *Anthropic API (native)*, *OpenAI API
(native)*.

**Native fields** (`ApiEndpoint`):

| Field | Rules |
|---|---|
| Preset | Official / DeepSeek / Custom (§3.3); sets the base URL and quirks |
| Base URL | `https` only, except `http://localhost`/`127.0.0.1`/`[::1]`. No credentials, query or fragment. Normalised without a trailing slash. Shown in full everywhere. |
| API key | a project **secret** (kind API key), picked or created in place. The connection stores the `SecretId` only. The value is entered once in the secret field and never shown again. |
| Models | fetched from `GET models` (source *From the endpoint*, with time) plus manual entries (source *Added by hand*). Each model has an id, optional context window, optional max output, and efforts. |
| Default model / effort | used by profiles that do not choose; the effort list comes from the model |
| Provider fallback | official Anthropic only (§3.1); default **off** (decision 4) |
| Data notice acknowledged | timestamp and person, once per project; required before the first run for presets that flag it (DeepSeek; decision 10) |

**Statuses and test.** `ConnectionStatus` gains `KeyMissing`, `KeyRejected`, `BillingProblem` and
`EndpointUnreachable`. `CheckResult` gains `Api { catalog, balance: Option<String> }` and the failure variants.

*Test connection* is an IPC request to the runner (`Command::CheckConnection`, protocol bump). The runner calls
`orchester_agent::check`:
- `GET models` decides reachable and accepted (zero tokens);
- DeepSeek also calls `GET /user/balance`.

The desktop never reads the key.

**Honest labels.**
- The inspector shows: *Prompts, repository content and command output are sent to `api.deepseek.com`.*
- Presets with a data-location note add *Data is stored in the PRC, per DeepSeek's privacy policy.*
- The enforcement panel says *Enforced by Orchester*, with the §5.4 mechanisms. Execution reads *This computer
  (Orchester runs the agent)*, and Authentication reads *API key `<secret name>`*.
- The usage dashboard uses the reserved `chart-series-3` for native connections.

**Screens that need Pencil design** (none exist; a designer comes later):
1. *Add connection* with the four kinds, and the native form: preset, base URL, key secret picker or creation,
   models.
2. Native connection inspector (`JuVaA` variant): endpoint, data notice, key secret, models with source, default
   model and effort, test result with balance, statuses.
3. Manual model editor: add, edit and remove a model with context and efforts.
4. Data-location acknowledgement dialog, per project.
5. Enforcement panel *Enforced by Orchester*, with the locked rows and their reasons.
6. Activity rendering of native tool calls (`read_file`, `search`) if they should look different from shell commands;
   P1 reuses the command rows.
7. Run summary note for network stays on and unconfined listed commands. The existing Claude pattern can be reused.
8. Approval modal content for a `run_command` with `shell: true` (shows the full shell string and the shell).

**CLI connections get no custom base URL or key** for now (decision 7). Orchester keeps storing no credentials for
CLIs and passing none to them (`invocation/mod.rs` withholds `ANTHROPIC_*`). Pointing Claude Code at DeepSeek's
`/anthropic`, or Codex at a custom model provider, is not offered.

## 8. Testing strategy

**Layers.**
1. **Core, no IO** (`crates/core/tests/native_*.rs`):
   - `NativeLoop` scripted with `ModelResponse` values: loop order, parallel reads, sequential writes, finish/question
     ends, the reminder then `NoResult`, every `Stop` mode, resume with unanswered calls, compaction trigger and
     segment rebuild;
   - `parse_call` against every schema, plus round-trip examples asserting each schema and its struct agree;
   - `apply_edits`;
   - `CommandPolicy::classify` table tests (known-safe argv rules, path arguments, operators, shell);
   - `FileScope` decisions;
   - capability rows for native on both platforms.
2. **Agent crate against a fake model server** (`crates/agent/tests/`):
   - The server is a local tokio listener on `127.0.0.1:0`, scripted with turns:
     `Script::new().expect(|req| ...).reply_sse(events)`. Both wire formats are supported.
   - It **asserts append-only history.** Each request's `system`/`instructions`, tools and item prefix must be
     byte-identical to the previous request's, which catches cache and preserved-thinking regressions without a live
     model.
   - Cases: SSE parsing (comments, multi-line data, split chunks), eager tool input with invalid JSON, `max_tokens`
     mid-tool-input, refusal, `incomplete`, 401/402/404/413/429 with `retry-after`/500/529, a mid-stream drop then
     retry, the idle timeout, the preserved-thinking 400 then strip-and-retry, usage mapping, and models listing.
   - Key redaction: the key never appears in any `ModelError` `Debug`/`Display` or event, asserted by searching
     serialized output.
3. **File tools on real file systems:** temp worktrees with `..`, absolute paths, symlinks (Unix), junctions
   (Windows, created by `cmd /c mklink /J` in the test), hard links, `.git` writes, ADS and device names (Windows),
   CRLF/BOM preservation, the stale read stamp, and an atomic write leaving no partial file.
4. **Runner with the fake server** (`crates/runner/tests/native_*.rs`): full step to `StepOutput`; command approval
   approve and deny (claim, modal row, wait excluded from active time); budget `AfterTurn`, time limit `Now`, pause
   at approval and checkpoint, cancel, resume after a runner restart, question/answer round trip, usage recorded per
   response, activity entries and `FileChanged` driving `first_edit`.

All tests carry name filters (rule 14), for example
`cargo test -p orchester-agent --test anthropic_wire anthropic_`.

**Live-test policy.** Live tests spend the user's money and send repository content to a third party.
- They are `#[ignore]`d, and run only when a person asks, with a key the person supplies in the environment
  (`ORCHESTER_LIVE_ANTHROPIC_KEY`, `ORCHESTER_LIVE_OPENAI_KEY`, `ORCHESTER_LIVE_DEEPSEEK_KEY`). Keys are never
  written to files.
- They run against a disposable fixture repository, never this one. Each uses the cheapest suitable model, a 200K
  token run budget and a 5-minute time limit.
- Each phase's acceptance needs one live run per affected provider: read, search, edit, an approved command,
  `finish_step`, and a second request showing cache reads.
- Results are recorded in `docs/evidence/native-<provider>-<date>.md`, with token counts and no key material.

## 9. Phased plan

Sizes are rough: S ≤ 1 week, M 1–3 weeks, L 3–6 weeks of one agent's focused work, including tests and reviews.

**P1 · Anthropic native: read, search, edit, write, approved commands, finish.** Size **L**.

Prerequisite refactors:
- split `step/command.rs` and `worker.rs`;
- `AgentEndpoint` in `Connection`;
- the new `AgentProvider` variants across all match sites: core adapter/catalog/stream, store mapping, runner
  `command.rs`/`mcp.rs`/`codebase_index.rs`, and about 10 desktop sites.

Done when:
- [ ] A native Anthropic connection can be added with a key secret. *Test connection* lists models through the
  runner. Key rejected, missing and unreachable each show their own status.
- [ ] A pipeline step runs on Opus 5.5 with prompt parity and ends with a validated `finish_step`. `question` waits,
  and the answer resumes the same conversation.
- [ ] File tools are confined per §5.1, with the platform tests passing on Windows and cross-checked on macOS.
- [ ] Commands are classified per §5.2: listed and known-safe run, every other command asks. Approvals use the
  existing modal; a denial is returned to the agent, which continues, and the step does not fail.
- [ ] A refusal (`stop_reason: "refusal"`) is shown as a problem with its category; provider fallback is off unless
  the connection turns it on.
- [ ] The capability rows and *Enforced by Orchester* panel follow §5.4. AnyCommand and NetworkOff are locked with
  reasons.
- [ ] Budgets, time limit, pause, cancel and resume (including after a runner restart) pass the runner tests. Usage
  is recorded per response, with cache reads and writes per §2.2.
- [ ] The append-only assertion passes in the fake server. A live run shows `cache_read_input_tokens > 0` from the
  second request.
- [ ] Compaction per §6.3, tested with a small fake context window.
- [ ] `cargo check`/`clippy --target aarch64-apple-darwin` is clean. `tools/check.ps1` passes. UI parts pass design
  verification (rule 15) once Pencil designs exist.

**P2 · OpenAI Responses.** Size **M**.
- [ ] `ResponsesClient` is stateless (§3.2), with reasoning items persisted and resent.
- [ ] Models come from `GET /models` plus the built-in metadata table and manual entries. Effort and summary values
  are verified against the docs.
- [ ] Every fake-server case from §8 is ported to the Responses wire format. One live run is recorded.

**P3 · Compatible endpoints and DeepSeek.** Size **M**.
- [ ] Presets and `WireQuirks`, with a `Custom` base URL on both wire formats.
- [ ] DeepSeek preset on Responses: reasoning replay, effort mapping, explicit max output, user tag, 402 status,
  `/user/balance` in the test, and the data-location acknowledgement.
- [ ] A fake-server script per preset quirk. One live DeepSeek run is recorded.

**P3b · OpenAI Chat Completions for self-hosted servers.** Size **M**. After P3 (decision 5).
- [ ] `WireApi::OpenAiChat` behind `ModelClient`, with reasoning, usage and tool-call mapping and its quirks in
  `WireQuirks`.
- [ ] The §8 fake-server cases ported to the Chat wire format. One live run against a self-hosted server is recorded.

**P4 · MCP passthrough and memory tools.** Size **M–L**.
- [ ] The runner works as an MCP client for the profile's servers, with `interactive_tool_approvals` on calls.
  Secrets come from the vault, and redaction is tested.
- [ ] `memory_search` and `memory_propose`, with secret screening.
- [ ] Runner-side MCP for native connections follows the design rule as updated on 2026-10-01 (decision 6).

**P5 · Sandboxing.** Size **L**, research-heavy.
- [ ] P5a, macOS: a seatbelt profile for commands enables AnyCommand and NetworkOff, probed per machine. Toolchains
  (cargo, npm, git) are verified on a real Mac.
- [ ] P5b, Windows: an evaluation of AppContainer against restricted-token options, with a prototype on a real
  repository and toolchain. Ship only what passes. Otherwise the rows stay locked, with the finding recorded in
  evidence.

## 10. Risks

**Risks.**
- **No OS sandbox for commands in P1.** Safety rests on Orchester deciding which commands start, and on what a listed
  command does once started. That is the same posture as Claude Code today, but users may assume "Orchester runs it"
  means "Orchester contains it". The labels in §5.4 and §7 are the mitigation.
- **Shell parsing on Windows.** `CommandLineToArgvW` rules differ from what PowerShell or `cmd` would do. Direct
  execution avoids interpreting shells, but `.cmd`/`.bat` shims go through `cmd.exe` (`exec::launch` already refuses
  unsafe arguments for that path).
- **Fast-moving provider APIs.** Several Anthropic features used here are betas (block binding, fallbacks, display
  updates), and the Responses streaming details are partly *to verify*. The quirks struct confines the churn to
  `orchester-agent`.
- **Cost surprises.** API usage is billed per token, unlike CLI subscriptions. Run budgets apply, but the defaults
  (1.4M tokens per run) may cost real money on Opus-tier models. Orchester shows no cost estimates (decision 8); at
  most later, as information in the Usage dashboard, never in limits.
- **Data residency.** DeepSeek stores data in the PRC, and every provider receives repository content. Labels and the
  acknowledgement are the mitigation.
- **Fan-out of the closed enums.** The new variants touch many files in four crates. The prerequisite refactors keep
  every file under 600 lines, but the P1 diff is large.

All product questions were answered on 2026-10-01; see [Decisions (2026-10-01)](#decisions-2026-10-01).
