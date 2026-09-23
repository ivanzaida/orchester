# Orchester — design consistency audit

Date: 2026-09-23. Scope: the 13 canonical desktop screens, recovery gallery, live Pencil nodes and [design handoff](../design/orchester-workspace.md).

The user requested an independent subagent audit. The subagent inspected all 13 previews and their current Pencil nodes; the primary agent cross-checked orchestration, versioning and the findings below. This was a read-only design review: neither the Pencil document nor its handoff was changed. Findings concern the proposed interface, not observed runtime defects. All displayed project data is illustrative.

Source: `H:/projects/orchester/data/orchester.pen`.

P1 means resolve before treating the affected flow as ready for implementation. P2 means a concrete consistency correction or additional state needed in the next design pass. Missing flows are identified separately from contradictions in existing screens.

## Findings

### 01 · P1 · Effective instructions do not account for every applicable rule

**Type:** visible inconsistency / incomplete explanation.

**Evidence:** [Instructions](../design/previews/yRKeF.png), frame `yRKeF`, lists `Git conventions` (`g4s4HO`) and `Communication style` (`xWkiV`) as `All agents`. For Builder, the effective-source row `hwc5i` lists only `Implementation standards · draft v3`. The [full prompt viewer](../design/previews/fZa3J.png), frame `fZa3J`, repeats that source (`hBmRg`), displays only its text (`iJdsS`) and reports `No missing sources` (`T8eQc`).

**Impact:** users cannot tell whether the other rules are included, grouped, excluded or accidentally omitted. This is a transparency defect in the design; the audit does not assert that a runtime drops instructions.

**Correction:** expand Project rules into every applicable rule with version and assembly order. Explain exclusions. Distinguish source groups from individual rules in counts.

### 02 · P1 · Save, publish and active versions have inconsistent meanings

**Type:** cross-screen ambiguity.

**Evidence:** [Instructions](../design/previews/yRKeF.png) has `Publish v3` and explicitly says existing runs keep v2 (`TyKBM`). The [pipeline editor](../design/previews/Q21HOA.png), frame `Q21HOA`, shows `v3` (`pLCWI`) and `All changes saved` (`Hsare`) without a corresponding draft/published distinction. [Tool setup](../design/previews/YEWFo.png), frame `YEWFo`, shows `Draft changes` (`qUklC`), `v2 draft` (`kutua`) and `Save tool` (`bp5XX`).

**Impact:** it is unclear whether saving makes changes available to new runs, saves a draft, or publishes a version. The handoff promises version snapshots for runs.

**Correction:** define one draft → published version lifecycle, or explicitly explain intentional differences between resource types. Display the published version separately from the editable draft and identify the version used when starting a run.

### 03 · P1 · General resource import only demonstrates tool-to-secret dependencies

**Type:** flow coverage gap.

**Evidence:** [Import](../design/previews/W4zGKx.png), frame `W4zGKx`, dialog `x2PZm`, advertises pipelines, agents, instructions, tools and secrets (`mHcJ7`). The illustrated dependency resolution covers a tool and credential binding (`rTyCo`). There is no illustrated pipeline dependency mapping, source-version choice, connection authorization or tool-name collision state.

**Impact:** importing a pipeline cannot yet be reviewed as a complete local copy with resolved references. The handoff promises broader behavior than the drawn flow covers.

**Correction:** use Select resources → Resolve dependencies. Show source/version, target resource and resolution status. Add a pipeline example with agent, instruction, tool and connection dependencies, including one unresolved dependency or name collision.

### 04 · P1 · Tool access rules have an entry point but no designed editing state

**Type:** flow coverage gap.

**Evidence:** [Tool setup](../design/previews/YEWFo.png), frame `YEWFo`, shows owner, environment, approval, access window and version (`k97jh` / `b0yLi7`). `Used by` (`zZmQ0`) reports configured usage. `Edit access rules` (`bW4hq`) and Access (`KPRiv`) have no corresponding state in the current screen set. Import refers to target-project access defaults (`vNLaq`) without displaying them.

**Impact:** configured usage does not establish permission. Users cannot yet inspect or edit which steps or agents may call a tool, a central part of the agreed tool-owned credential model.

**Correction:** design the Access state with allowed pipelines/steps, any role restrictions, environment, approval policy and denied behavior. Display the resulting policy during import. Keep secret Used by links read-only.

### 05 · P1 · The promised plan-approval gate has no actionable run state

**Type:** flow coverage gap.

**Evidence:** [New task](../design/previews/l2YA4.png), frame `l2YA4`, promises plan review before implementation (`uE7qG` / `Sdzbc`). The [pipeline](../design/previews/Q21HOA.png) Plan step (`NoQKb`) waits for approval. The canonical [task screen](../design/previews/vpgrJ.png) shows the later Ready for review state. No plan-awaiting-approval screen provides plan inspection, approval or requested changes. Recovery panels and status variants also do not establish the normal running, paused or retry-exhausted task controls.

**Impact:** the primary task flow contains a required human decision with no designed way to make it.

**Correction:** first add Awaiting plan approval with the plan, approve and request-changes actions. Then specify relevant running/recovery controls and their effects, including pause/resume, retry and cancel where supported. Do not present status labels alone as a complete flow.

### 06 · P2 · Pipeline and run disagree about the planning agent

**Type:** visible fixture inconsistency.

**Evidence:** [Pipeline](../design/previews/Q21HOA.png) v3 assigns Plan to `Planner · read-only` (`NoQKb`). The [task](../design/previews/vpgrJ.png) for Feature delivery v3 lists only Explorer, Builder and Reviewer under Agents on this run (`it5nz`); Explorer is described as `Repository context & plan` (`Gf13q`). Stage names also vary between Implement / Run checks and Build / Verify.

**Impact:** the run does not faithfully explain the displayed pipeline assignment.

**Correction:** align the fixtures and agent mapping, or show an explicit per-run override. Use consistent stage names in editor, run progress and activity.

### 07 · P2 · Variable preview lacks a precise execution context

**Type:** state coverage gap.

**Evidence:** [Instructions](../design/previews/yRKeF.png) selects a task (`miQ0U`) and Builder / Implement (`lQAXf`). The rule's start/resume setting (`UVlYr`) does not select a concrete preview event. The [catalog](../design/previews/EfTiT.png) includes phase-dependent `run.revision` (`Ho9uP`), while the documented catalog includes run ID and attempt. No run/attempt selector or explicit sample context explains which values are being resolved.

**Impact:** the same task can have multiple runs, attempts and injection events with different values and availability.

**Correction:** share an explicit Task / Run or Sample / Attempt / Step / Injection event context across prompt and variable previews. Show an unavailable-variable example with its availability condition and an explicit fallback or suitable injection point.

### 08 · P2 · Add secret demonstrates an unhandled duplicate name

**Type:** visible fixture inconsistency.

**Evidence:** [Add secret](../design/previews/tpF7R.png), frame `tpF7R`, enters `STAGING_DEPLOY_TOKEN` (`f2bgO/ObbY7`) for Kontur / Staging (`hPAIz`, `fbIBM`). The same secret already appears in the underlying list (`nyHPi`). Save (`zMjM4`) appears enabled with no collision message.

**Impact:** it is unclear whether creation makes a duplicate, replaces the existing value or fails.

**Correction:** give the happy-path example a unique name. Add inline duplicate validation linking to the existing secret. Keep credential rotation as the separate Replace value action.

### 09 · P2 · Resource pages duplicate the project selector

**Type:** ownership ambiguity.

**Evidence:** [Tools](../design/previews/ZFwWl.png), frame `ZFwWl`, has a project dropdown (`AgeB9`). [Secrets](../design/previews/R99lt.png), frame `R99lt`, has another (`afGrA`). Both sit inside the already selected project shell (`zo9Py/ZcJDx`).

**Impact:** a second editable selector could mean a global filter, a local owner change or whole-project navigation. Those interpretations do not share the same per-project semantics.

**Correction:** remove the redundant selectors or render a read-only owner label. Keep project switching in the shell.

### 10 · P2 · First-project setup is not covered end to end

**Type:** flow coverage gap.

**Evidence:** recovery gallery `y8TKW8` contains Connect your first project / Connect GitHub (`RlwqW`). The current screen set does not cover project creation, repository selection/authorization, runner connection, agent provider/profile setup or readiness before the first run. Agents and Connections exist as navigation destinations.

**Impact:** the design supports evaluating an already configured project, but not how the initial personal user reaches that state.

**Correction:** design a compact Project → Repository → Runner/agents → Ready path, with an incomplete or failed connection state and a clear way to resume setup.

## Recommended design sequence

1. Align the existing screens: rule visibility, version vocabulary, agent/stage names, unique secret fixture and a single project switcher.
2. Complete the operational loop: plan approval and run controls, precise preview context, and tool access configuration.
3. Extend setup and reuse: first-project onboarding and full import dependency resolution.

The restrained grayscale direction, project-owned resource model and read-only secret-to-tool links remain coherent. The principal findings concern semantics and flow coverage rather than a need to replace the visual direction. Static design inspection does not establish runtime behavior, accessibility conformance or secret isolation.

## Resolution pass — 2026-09-23

The findings above are the historical audit of the original 13 screens. After the user requested fixes, the design expanded to 32 screens, then 33 after a further request for the first-launch screen. Current previews and the [handoff](../design/orchester-workspace.md) describe the corrected design, so they no longer reproduce the original findings.

| Finding | Design resolution |
| --- | --- |
| 01 · Effective rules | `yRKeF`, `fZa3J`: seven individually versioned Builder sources; Reviewer-only exclusion explained. |
| 02 · Versions | Pipeline, instructions, tool and profile editors distinguish saved draft, published version and publish action; new runs use published snapshots. |
| 03 · Import | `W4zGKx` → `QqnDd` → `hF664`: selection, seven dependency mappings, source versions, local reuse/copies, name collision, destination authorization and outcomes. |
| 04 · Tool access | `H97stw`: allowed pipeline/step scopes, environment, per-deployment decision, running-step access window, denied behavior and immediate revocation. Import exposes the resulting policy. |
| 05 · Human gate and controls | `RD4pc`, `x3lVC`, `jgqPe`, `Oqmog`, `J3JZaT`: research, actionable plan approval, running conversation, safe pause/resume and retry-limit recovery. |
| 06 · Agent and stage mapping | Pipeline, creation summary and run agree on Explorer, Planner, Builder and Reviewer and on the six stages. |
| 07 · Variable context | Shared preview context includes historical run/attempt, agent/step and injection event. `m2WjL` demonstrates unavailable revision, required-value blocking and explicit alternatives. |
| 08 · Duplicate secret | Unique happy-path name in `tpF7R`; inline duplicate error, disabled Save and existing-secret link in `m9Dhp`. |
| 09 · Project owner | One project switcher in the shell; resource ownership is read-only. |
| 10 · Setup | `jnpRq`, `W6uXiL`, `w6ODp4`, `x1YpfN`, `Q7htyU`: GitHub authorization, repository, runner/agent assignments, readiness and recoverable blocking. First launch was subsequently added as `ZBoBm`. |

The user also requested direct task communication and mixed providers per role. The design now has a task Conversation panel with recipient and delivery states, project agent profiles (`WPmtS`), project Connections (`a4Lqb`), Add provider (`h6CmJ`) and expired MCP access (`RZUKo`). The example maps Explorer to Claude / Opus and Builder to Codex / Sol. These are proposed interface fixtures, not verified provider integrations.

The independent subagent repeated its read-only review of the 32-screen design and key exports. It found one residual mismatch: the import dialog referred to an existing Deploy preview tool absent from the library. The primary agent added that draft tool to the library and import backgrounds and corrected the count to five tools with two secret bindings. Full-size visual review then corrected list overlap and import-tab spacing. All 32 frames passed depth-9 structural checks; the new first-launch frame and expanded section also passed their own checks and received visual review. The first-launch addition was not part of the independent review.

These resolutions cover the audited static design. Runtime correctness, provider compatibility, persistence, authorization enforcement and accessibility remain implementation verification work. No implementation or external service action occurred.
