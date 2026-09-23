# Orchester — workspace, agents, connections and access

Date: 2026-09-23. Status: proposed design for review, not an approved implementation contract.

## Brief

Personal agent orchestration first, with a future path to teams. The user requested a convenient, restrained application without bright colors, then added first-class instructions inside Orchester, a discoverable variable catalog, and secrets for staging deployment and remote database access. The user subsequently established that tools own credential bindings; secret settings show read-only links to tools; resources can be imported from another project. The final scope clarification makes the project the owner of all operational resources: tasks/runs, pipelines, agent profiles, instructions, tools, secrets and connections. There is no workspace instruction layer or shared mutable workspace resource library.

Navigation selects a project first. Every functional section uses that project context. All projects is a project-switching entry point; it does not make operational resources global. Imported resources are independent local copies with dependency remapping.

The design is an editable desktop application reference at 1440 × 960. English UI copy, grayscale surfaces, Inter for interface text and Geist Mono for identifiers and prompts. No runtime, framework, provider or storage implementation has been selected by this work.

All project names, runs, PRs, credentials, reports, usage events, token estimates and budgets are illustrative fixtures. No repository connection, deployment, database query, approval or secret storage actually occurred.

## Editable source

File: `H:/projects/orchester/data/orchester.pen`, edited exclusively through Pencil MCP.

The root contains eight numbered sections in one horizontal band, ordered left to right with a 320px gutter. Reading order within a flow is left to right, then the next row. Main-flow labels use section.step numbers; alternative states use letters. The layer tree follows the same order.

| Section | Node |
| --- | --- |
| 00 / Foundations | `cuxUu` |
| 01 / Getting started | `K7IL6` |
| 02 / Tasks & runs | `g7ncNr` |
| 03 / Pipelines | `Y6XGt` |
| 04 / Agents & connections | `wNy9i` |
| 05 / Instructions & context | `MRxol` |
| 06 / Tools & imports | `V8h6d` |
| 07 / Secrets & access | `b3N1Ix` |

Canonical screens and states, in canvas order:

| Screen | Node | Preview |
| --- | --- | --- |
| 01.01 · First launch | `ZBoBm` | [Preview](previews/ZBoBm.png) |
| 01.02 · Authorize GitHub | `jnpRq` | [Preview](previews/jnpRq.png) |
| 01.03 · Project & repository | `W6uXiL` | [Preview](previews/W6uXiL.png) |
| 01.04 · Runner & agent models | `w6ODp4` | [Preview](previews/w6ODp4.png) |
| 01.05 · Ready → Create first task | `x1YpfN` | [Preview](previews/x1YpfN.png) |
| 01.A · Setup blocked / Restore provider access | `Q7htyU` | [Preview](previews/Q7htyU.png) |
| 02.01 · Project overview | `Twtqc` | [Preview](previews/Twtqc.png) |
| 02.02 · New task / Brief, context & limits | `l2YA4` | [Preview](previews/l2YA4.png) |
| 02.03 · Research running | `RD4pc` | [Preview](previews/RD4pc.png) |
| 02.04 · Review & approve plan | `x3lVC` | [Preview](previews/x3lVC.png) |
| 02.05 · Working task / Conversation | `jgqPe` | [Preview](previews/jgqPe.png) |
| 02.06 · Ready for review | `vpgrJ` | [Preview](previews/vpgrJ.png) |
| 02.A · Paused / Resume from checkpoint | `Oqmog` | [Preview](previews/Oqmog.png) |
| 02.B · Retry limit / Review recovery | `J3JZaT` | [Preview](previews/J3JZaT.png) |
| 03.01 · Pipeline / Feature delivery | `Q21HOA` | [Preview](previews/Q21HOA.png) |
| 04.01 · Project connections | `a4Lqb` | [Preview](previews/a4Lqb.png) |
| 04.02 · Add agent provider | `h6CmJ` | [Preview](previews/h6CmJ.png) |
| 04.03 · Agent profiles / Assign connection & model | `WPmtS` | [Preview](previews/WPmtS.png) |
| 04.A · Expired MCP access / Reconnect | `RZUKo` | [Preview](previews/RZUKo.png) |
| 05.01 · Project rules & effective prompt | `yRKeF` | [Preview](previews/yRKeF.png) |
| 05.02 · Resolved prompt / Inspect sources | `fZa3J` | [Preview](previews/fZa3J.png) |
| 05.03 · Variables / Search, inspect & insert | `EfTiT` | [Preview](previews/EfTiT.png) |
| 05.A · Variable unavailable at this event | `m2WjL` | [Preview](previews/m2WjL.png) |
| 06.01 · Project tool library | `ZFwWl` | [Preview](previews/ZFwWl.png) |
| 06.02 · Tool configuration & credentials | `YEWFo` | [Preview](previews/YEWFo.png) |
| 06.03 · Tool access rules | `H97stw` | [Preview](previews/H97stw.png) |
| 06.04 · Import / Select project resources | `W4zGKx` | [Preview](previews/W4zGKx.png) |
| 06.05 · Import / Resolve dependencies | `QqnDd` | [Preview](previews/QqnDd.png) |
| 06.06 · Import / Ready to create local drafts | `hF664` | [Preview](previews/hF664.png) |
| 07.01 · Project secrets | `R99lt` | [Preview](previews/R99lt.png) |
| 07.02 · Add project credential | `tpF7R` | [Preview](previews/tpF7R.png) |
| 07.03 · Runtime approval / One deployment | `nuuZL` | [Preview](previews/nuuZL.png) |
| 07.A · Duplicate secret name | `m9Dhp` | [Preview](previews/m9Dhp.png) |

All 33 screen IDs are preserved. Screens remain 1440 × 960, at x 48 / 1568 / 3088 with 80px horizontal gaps, 88px row gaps and labels 32px above each frame. Section subgroups have a 96px gap.

- Getting started: first launch → GitHub authorization → repository → runner/agents → ready. Setup blocking and compact empty-project feedback are below the main flow.
- Tasks & runs: overview → new task → research → plan approval → working conversation → ready for review, together in `D4pfd`. Pause and retry are alternatives in `O4vgKt`; runner/repository/loading recovery, status variants and interaction notes are in `y8TKW8`.
- Pipelines: the visual editor is separate from task execution.
- Agents & connections: connections → add provider → assign connection/model in the agent profile; expired-access recovery is below.
- Instructions: rules → composed prompt → variable catalog; unavailable-variable state is below.
- Tools & imports: library → configuration → access policy in the first group. Select → resolve dependencies → ready to import is a separate complete row in `TSGMX`. Success/failure panels remain in `tjNmM`.
- Secrets & access: library → add credential → runtime approval; duplicate-name validation is below.

Section previews: [Getting started](previews/organization/K7IL6.png), [Tasks & runs](previews/organization/g7ncNr.png), [Pipelines](previews/organization/Y6XGt.png), [Agents & connections](previews/organization/wNy9i.png), [Instructions](previews/organization/MRxol.png), [Tools & imports](previews/organization/V8h6d.png), [Secrets & access](previews/organization/b3N1Ix.png).

## Components and tokens

30 reusable components are defined. Most are in Foundations; the composed secret inspector, effective instruction preview and inline notice have canonical definitions within their owning screens. The shared brand is canonical inside the sidebar. Screens use linked instances; content, icon, state and supported size are intended overrides.

| Family | Node |
| --- | --- |
| Button | `u1XBP` |
| Navigation item | `TSJpn` |
| Status label | `DeizF` |
| Pipeline step | `ajikA` |
| Task row | `zemhy` |
| Select/input field | `ZGwlv` |
| Sidebar | `zo9Py` |
| Tab | `SOW6S` |
| Activity event | `mBkeJ` |
| Evidence check | `l3hyb` |
| Project metric | `dZ2Zt` |
| Run progress step | `i9rVFu` |
| Pipeline summary step | `XVkaB` |
| Key/value property | `tfGRt` |
| Recovery panel | `iN90G` |
| Instruction rule item | `jNTVN` |
| Instruction source row | `SV0BN` |
| Variable row | `ZJ6KN` |
| Secret row | `vxAMD` |
| Secret details inspector | `CQxCb` |
| Tool library row | `sFtAz` |
| Preview execution context | `TnjFo` |
| Effective instruction preview | `RGk5C` |
| Inline notice | `q5LzI` |
| Import selectable resource | `cuocA` |
| Import dependency mapping | `HfNoA` |
| Conversation message | `J5MIT` |
| Task conversation panel | `kjsmd` |
| Agent model assignment row | `kOI6w` |
| Orchester brand | `a0S44m` |

Colors: canvas `#101010`, sidebar `#141414`, surface `#1A1A1A`, raised `#222222`, hover/selection `#2C2C2C`, border `#363636`, hairline `#2B2B2B`, text `#EEEEEE`, muted `#A3A3A3`, subtle `#8F8F8F`, primary action `#E6E6E6` with `#181818` text. Variables also hold fonts, spacing and 7/12px control/panel radii. Status always includes text and an icon.

## Core workflow

- New task collects a brief, acceptance criteria, context, pipeline and limits in its selected project. Project, agent-role and pipeline/step instructions are included automatically. Repository files are optional context, not the primary rules interface.
- Create & run submits once and displays pending feedback; retry must not create a duplicate run. Draft save and run creation have separate states.
- The sample pipeline is Research → Plan → Implement → Run checks and Review in parallel → Create pull request. Explorer researches, Planner proposes the plan, Builder implements and Reviewer independently reviews. Plan approval occurs before implementation; the user reviews the PR before merging. Failed verification returns to Implement, up to the configured retry limit.
- The task page binds evidence to an exact revision. New code invalidates stale evidence. Review actions expose changes; a report alone is not proof of implemented acceptance.
- Run snapshots retain effective pipeline, agent-profile, model, instruction and tool versions, explicit step overrides and context. Runner loss preserves checkpoints and exposes recovery; it must not manufacture progress or completion.
- Pipelines, profiles, instructions and tools share Draft saved → Publish vN → Published vN. Saving a draft does not publish it. The pipeline editor shows draft v4, published v3 and Run v3; tool setup shows draft v2 and published v1. Publishing affects new runs. Dragging, keyboard selection, undo and persistence remain implementation work.

The task creation fixture includes four acceptance criteria, including cancelled OAuth, and shows the six steps from published Feature delivery v3. Create & run leads to research and planning; it does not bypass plan approval. The plan screen exposes plan v1, scope and acceptance criteria, with Approve plan & continue and Request changes. Requested changes return to Planner; a revised plan requires a new decision.

Pause first settles at a safe checkpoint and stops new scheduling/tool calls; only then may the UI claim Paused. Resume continues the same run and version snapshot. Cancel stops new scheduling and preserves work and artifacts; it does not undo completed external operations, and unresolved in-flight outcomes remain visible. Retry-limit recovery offers one explicitly authorized additional attempt within the remaining budget or revision of the task first. The exhausted example is an alternative Run 129 / Attempt 3; the normal Run 128 shows intermediate checkpoint `4b92ad0` and final reviewed revision `8f3a2c1`.

## Task conversation

The task has a visible Conversation tab, an Open conversation entry on the review summary, and a conversation panel beside running/paused progress. The recipient defaults to Orchestrator; the picker can target a participating agent directly. Orchestrator routes messages, while direct messages retain their selected recipient. Messages belong to the task and record run, step, recipient and author.

Delivery states distinguish Queued, Delivered and Acknowledged. Acknowledgement does not prove the requested change was applied. Work-changing messages are delivered at the next checkpoint; Pause is the visible control for an urgent interruption. While paused, messages remain queued and are delivered before the next action on resume. Scope changes return to plan review rather than silently changing approved work.

Proposed interaction contract: failed or pending sends retain the draft; retries deduplicate the message; Enter adds a newline and Ctrl/Cmd+Enter sends. Escape closes the recipient picker and restores focus. Keyboard, delivery and persistence behavior remains to be implemented and tested.

## Agent profiles and connections

An agent profile defines a role, responsibilities, role instructions and a project connection/model. Connections establish project access to repositories, agent providers and MCP servers. Tools expose the actual callable actions. A provider is selected per profile; the project does not impose one provider on every agent.

The mixed-provider fixture assigns Explorer and Reviewer to Claude · Kontur / Opus, and Orchestrator, Planner and Builder to Codex · Kontur / Sol. The Builder inspector contains separate Connection and Model selectors; provider-supported settings such as reasoning/context appear conditionally. Pipeline steps select a published project profile and inherit its model by default. A step can explicitly override the model, and the inspector shows the effective connection and model.

Connections (`a4Lqb`, section 04) lists GitHub, Codex · Kontur, Claude · Kontur and Project knowledge MCP. The selected connection shows provider, execution location, authentication source, status, last check, available models and read-only Used by links. Add connection (`h6CmJ`) configures an agent provider and validates access before saving. Expired MCP access (`RZUKo`) exposes Reconnect and preserves task work. Revocation blocks future calls immediately; ordinary profile publishing does not rotate or revoke credentials.

Sol, Luna and Opus are illustrative labels supplied during the design discussion. Provider adapters, public model identifiers, authentication options and supported settings were not verified by this design task. The implementation must obtain actual model catalogs and capabilities from each adapter and validate profile compatibility; the mockups do not establish working Codex or Claude integration.

## First project setup

First launch (`ZBoBm`) appears when the personal workspace has no projects or resumable setup. It has no selected project or project navigation, and no fabricated recent projects, activity or connected-runner status. Create your first project opens the first setup step. The name field initially starts empty; the following setup fixtures show the state after the user enters Kontur. The three-step preview explains repository access, mixed-provider agent assignment and the first task. The primary action creates a setup draft once, rather than starting agents or triggering provider authorization by itself. Existing incomplete setup should resume at its saved step; a configured returning workspace opens its projects rather than repeatedly showing Welcome.

Setup covers project/repository → runner and agents → readiness. GitHub authorization precedes repository selection; unavailable repository/continue controls stay disabled. The second step configures the runner and each role's connection/model, including mixed providers. The final review checks repository, runner, required profiles, instructions and the starter pipeline. Finish & create first task publishes the reviewed starter configuration and opens New task; it does not start a run automatically.

An expired required provider blocks completion with Reconnect or change-profile recovery. Save & leave retains incomplete setup so it can be resumed. Connection authorization is scoped to this project, including when importing configuration from another project.

## Instructions

Instructions are stored and versioned within a project in Orchester. Scope navigation covers project rules, agent roles, pipeline overrides and step overrides. A rule can target selected agent roles and an injection event. The mockup selects agent start/resume; the final event vocabulary remains to be specified. Importing an instruction from another project creates a new locally owned definition rather than a shared rule reference.

The preview displays proposed assembly order within the selected project: agent role → project rules → pipeline instructions → step instructions → task context. This order is an inspectable composition, not a promise that arbitrary contradictory prose can be resolved automatically. Inherited rules should retain source and version; disabling or replacing one must be explicit. Runtime permissions and budgets are separate enforced controls, not prompt text.

Publishing a draft creates a new version used by new runs. Existing runs retain their snapshot. Applying a new version to an active run needs a separate explicit action at a defined boundary; it is not designed as a silent update.

View full prompt opens the source-indexed read-only viewer. Variables are resolved for the selected task/agent/injection point. Copy full prompt copies composed instructions while excluding secret material. Unknown variables and missing required sources receive actionable validation. Closing with Escape restores focus to the invoking control and retains edits.

The Builder preview now accounts for seven individual sources: role, Implementation standards draft v3, Git conventions v1, Communication style v1, pipeline v3, step v2 and task context. Review checklist v2 is explicitly excluded because it targets Reviewer. The same execution context appears in the preview, full viewer and variable catalog: task ORC-128, historical Run 071 / Attempt 1, Builder / Implement, agent start before the first commit, with draft substitution identified. It is a historical preview context, separate from the current working Run 128. A task summary links to per-agent snapshots because applicable source counts differ by role.

## Variable catalog proposal

The Variables control is visible beside the prompt editor. The catalog searches names and descriptions, groups variables by source, and shows type, explanation, current value and availability. Selecting a variable does not modify the prompt; Insert writes its expression at the retained cursor and creates one undoable edit. Escape cancels insertion. Lists need a defined rendering format before implementation.

The count of 24 in the mockup refers to this proposed catalog, not a discovered runtime API:

| Group | Variable | Type | Availability |
| --- | --- | --- | --- |
| Task | `task.id` | string | After task creation |
| Task | `task.title` | string | Task selected |
| Task | `task.description` | string | Task selected |
| Task | `task.acceptance_criteria` | list of strings | Task selected; may be empty |
| Task | `task.attachments` | list of safe attachment references | Task selected; may be empty |
| Task | `task.created_at` | timestamp | After task creation |
| Project | `project.id` | string | Project selected |
| Project | `project.name` | string | Project selected |
| Project | `project.repository` | sanitized repository reference | Repository connected |
| Project | `project.default_branch` | string | Repository metadata loaded |
| Agent | `agent.id` | string | Agent assigned |
| Agent | `agent.name` | string | Agent assigned |
| Agent | `agent.role` | string | Agent assigned |
| Agent | `agent.model` | string | Model selected |
| Pipeline | `pipeline.id` | string | Pipeline selected |
| Pipeline | `pipeline.name` | string | Pipeline selected |
| Pipeline | `pipeline.version` | integer | Published version selected |
| Run | `run.id` | string | Run created |
| Run | `run.attempt` | integer | Attempt created |
| Run | `run.budget` | decimal in project currency | Run limits configured |
| Run | `run.branch` | string | Working branch created |
| Run | `run.revision` | string | Commit available |
| Step | `step.id` | string | Step selected |
| Step | `step.name` | string | Step selected |

The sample syntax is `{{task.title}}`. Variables are evaluated as data, not executable expressions. Secret values are excluded from this catalog. Missing values must not silently become empty text: explain the unavailable phase and offer an appropriate injection point or an explicitly configured fallback.

The unavailable-variable frame demonstrates `run.revision` before the first commit. Required policy blocks injection; the user can choose a later injection event or explicitly configure a fallback. The UI must not fabricate a revision or silently insert an empty string.

## Project tools and imports

Tools, secrets, instructions, agent profiles, pipelines and connections are all project-owned. The navigation entry opens an explicit project context; the breadcrumb and project selector show the current owner. A pipeline selects tools and profiles available in its project. Connections represent provider/server authentication within the project; Tools represent concrete callable actions. Their exact backend relationship remains a separate engineering contract.

The tool library offers creation of HTTP, command and database tools and discovery/import from an MCP connection. Setup owns action configuration, input/output mapping, credentials, allowed environment, access rules and version. The sample HTTP deployment tool selects `STAGING_DEPLOY_TOKEN` from Kontur and maps it to bearer authentication. The example deployment endpoint is fictional. Configuration validation must not execute a deployment. A future test action must clearly expose its actual side effects and apply the normal access policy.

A tool may only bind secrets owned by its project. Tool settings offer New project secret, returning the newly created reference to the binding selector. Secret settings have no tool picker or editable tool policy: Used by is a derived read-only list linking to the consuming tool. The secret creation screen collects name, type, value and environment in its current project.

Import resources is available in project navigation and from resource libraries. It covers pipelines, agent profiles, instructions, tools and secrets. The proposed default is an independent copy with a new local identity and source provenance. Dependencies must be enumerated: pipeline steps reference local agent/tool/instruction versions, and tools reference local connections and secrets. Tool credential dependencies are mapped to an existing compatible target-project secret, imported as a new secret, or supplied with a new value. Standalone secret import is also supported. Connections require target-project authorization before use; importing configuration must not silently confer access. No credential value is displayed in the preview or placed on the clipboard.

The import fixture uses Select resources → Resolve dependencies for Preview release v3 from Personal website into Kontur. Seven mapping rows show the pipeline and six dependencies: reuse local Builder published v2 and Implementation standards v2; copy Build preview; rename the copied Deploy preview to Website preview because Kontur already contains a draft Deploy preview; import PREVIEW_HOST_API_KEY; and authorize Preview host configuration in Kontur. The library shows that existing draft among five tools, with two existing secret bindings.

The result is four new resource copies, two reused local resources and one separately authorized connection. The blocked and ready states differ in destination authorization; source access is never copied. The target policy is explicit: Build preview → Build, Website preview → Deploy, Preview environment, each deployment requires approval, all other callers denied. New configuration stays draft until reviewed and published; import does not start a run. Source/version and destination mappings remain inspectable.

Source edits do not propagate. Permission to read/import source resources and create target resources must be checked independently. Name collisions require an explicit rename or reuse selection, never a silent overwrite. Missing credentials leave the tool unready. Import failure preserves mappings and the original project and allows an idempotent retry without duplicate resources. Success opens the imported pipeline draft for review.

## Secrets and access

The proposed design stores credentials separately from prompt rules and task context. A secret has an owner project, environment, name, type and value. The tool owns credential references, operation constraints and step access rules. Values are masked in management UI; replacing a value rotates it. Used by is a read-only reverse reference to local tools. No actual secrets exist in this design file or its previews.

Agents request capabilities through named tools. The tool receives the credential for the allowed operation/window; the model must not receive raw secret values through prompts, variables, copied instructions, logs or tool output. Implementation must keep credentials out of agent-readable process state and redact tool output; a mask in the UI is not a security boundary. Storage encryption, key management, runner isolation, provider adapters and rotation are open engineering decisions requiring separate review.

The sample database capability uses a database account with read-only privileges and a constrained query tool. The deployment tool binds a token and grants its `deploy.staging` capability to a specific step. Read-only guarantees require database/tool enforcement, not instructions asking the agent to avoid writes. Rotation and archival remain secret lifecycle actions; usage remains observable from the secret, while editing permissions happens in Tools.

The Access tab (`H97stw`) edits the allowed pipelines and steps, environment, approval policy, access window and denied behavior. Configured Used by links are distinct from the allowlist. The selected policy asks for each deployment; policy configuration may permit previously authorized scopes without repeated prompts. The shown deployment approval names project, environment, run, step, action, exact revision, tool and credential reference. It authorizes one deployment and expires when the step finishes. A different revision or operation requires a fresh policy decision. Denial leaves the step awaiting a changed plan; it does not imply a crash or erase work. All other callers are denied. Revoke access is a separate immediate action.

Add-secret's successful fixture uses the unique name STAGING_READ_API_KEY. The duplicate state shows STAGING_DEPLOY_TOKEN already exists in Kontur / Staging, disables Save and links to the existing secret. Replace value remains the distinct rotation action.

Audit records contain who/what requested access, capability, scope, run/step, decision, time and outcome without credential values. Revocation affects future calls immediately; completed external side effects are not reversed. Network failure must not display a successful save, deployment or query. Retries need idempotency and explicit outcome handling.

## Copy, layout and implementation acceptance

Proposed i18n namespaces: `nav.*`, `workspace.*`, `tasks.*`, `runs.status.*`, `runs.controls.*`, `planReview.*`, `conversation.*`, `pipelines.*`, `agents.*`, `connections.*`, `setup.*`, `resourceVersions.*`, `instructions.*`, `variables.*`, `tools.*`, `projectImport.*`, `secrets.*`, `access.*`, `recovery.*`. Examples: `instructions.publishVersion`, `instructions.previewEffective`, `variables.insert`, `variables.unavailableAtPhase`, `tools.credentials`, `tools.validateConfiguration`, `projectImport.resolveDependency`, `projectImport.independentCopies`, `secrets.usedBy`, `secrets.add`, `secrets.replaceValue`, `access.allowDeployment`, `access.deny`, `agents.inheritModel`, `connections.refreshModels`, `conversation.queuedAtCheckpoint`, `planReview.requestChanges`, `setup.resume`. Prompts, task descriptions, secret names and repository content are user-authored data.

Desktop is verified visually at 1440 × 960. At narrower widths collapse navigation first, then open inspectors on demand; stack creation-form columns. Dialogs need viewport-constrained scrolling and focus trapping. Search, pagination, keyboard navigation, Escape, clipboard, screen-reader labels, loading, validation and persistence still need runtime implementation and testing. Native/web runtime choice and mobile editing are not established by these screens.

Visual evidence: the original 13 screens and 20 additional screens were inspected through live Pencil nodes and screenshot/export review. The independent audit was repeated against the expanded 32-screen set; its remaining import/library fixture mismatch was corrected by adding the existing draft Deploy preview and updating the count. The subsequently requested first-launch screen received primary-agent layout and full-size visual review. Full-size export review caught and corrected a tool-list overlap and import-tab spacing that the structural check did not flag. Repeated controls use 30 linked component definitions. See the [audit and resolution record](../evidence/design-consistency-audit-2026-09-23.md).

The subsequent canvas-organization pass grouped the same 33 screens into eight numbered sections, aligned the layer tree and screen labels, and separated normal flows from alternate states. All 33 screens passed depth-9 layout checks after relocation; section exports were inspected for actual visibility and ordering. Reflow exposed excess tool-list height, which was corrected in the library and three import backgrounds. The product flows and screen IDs were preserved.

These are static design checks. Actual orchestration, provider authentication, chat delivery, persistence, responsive behavior, accessibility and credential isolation were not implemented or tested. No production code or real external service state was changed by this design work.
