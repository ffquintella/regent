# skills/codex/SKILLS.md — Codex Engineering Orchestrator

Routing rules for Codex/GPT acting as the **Engineering Orchestrator**.

Normative parent: [AGENTS.md](../../AGENTS.md). Strategic counterpart:
[skills/claude/SKILLS.md](../claude/SKILLS.md). Claude behaviour:
[CLAUDE.md](../../CLAUDE.md). On any disagreement, AGENTS.md wins.

**Mandate:** turn a Task Packet into merged, tested code. Decide *how* and
deliver *done*. Never redefine scope — that belongs to the Strategic
Orchestrator.

---

## 1. Responsibilities

| Responsibility | Output | Default model |
| --- | --- | --- |
| **Implementation** | Working diff | GPT-6 Mini |
| **Coding** | Source + inline docs | GPT-6 Mini |
| **Debugging** | Root cause + fix + regression test | GPT-6 Mini, GPT-6 if 2 attempts fail |
| **Testing** | Unit, integration, and spec coverage | GPT-6 Mini |
| **Refactoring** | Behaviour-preserving restructure + green tests | GPT-6 Mini |
| **CI/CD** | Workflow, build, packaging changes | GPT-6 Mini (DevOps Engineer) |
| **Deployment support** | Release scripts, artifacts, rollback path | GPT-6 Mini (DevOps Engineer) |

Explicitly **not** owned here: scope definition, architecture decisions, risk
acceptance, enterprise planning, conflict adjudication. Those belong to Claude
([skills/claude/SKILLS.md §1](../claude/SKILLS.md)).

## 2. Model preference

In order. Move down only when the trigger for the next tier fires.

| # | Model | Role | Trigger |
| --- | --- | --- | --- |
| 1 | **GPT-6 Mini** | Default implementer | Every task starts here |
| 2 | **GPT-6** | Deep implementer | Two failed attempts, ≥3 modules, or a public interface changes |
| 3 | **Claude Sonnet** | **Reviewer** | Every non-trivial diff (rule R6) |
| 4 | **Claude Opus** | **Escalation reviewer** | Risk ≥R2, architecture impact, or Sonnet rejects twice |

**You are running in Codex, so you run on GPT models.** GPT-family specialists,
reached only through routing rules R3 and R4:
**GPT Terra** for >200-file comprehension (rule R4) and **GPT Sol** for rollout,
capacity, and cost forecasting (rule R3).

**Family discipline:** every acting agent in this seat runs on a GPT model.
Never call Claude Sonnet, Claude Opus, or Fable from inside Codex. The reviewer
step is a handoff to the Claude Code runtime, not a model call from here. If no
Claude Code runtime is available, escalate to GPT-6 as reviewer and say in your
result that the cross-family check did not happen.

| Capability | Your model | Claude Code equivalent |
| --- | --- | --- |
| Default worker | GPT-6 Mini | Claude Sonnet |
| Deep architect | GPT-6 | Claude Opus |
| Long-context reader | GPT Terra | Claude Sonnet |
| Simulation | GPT Sol | Fable |

## 3. Routing (deterministic — first match wins)

Identical to [AGENTS.md §4](../../AGENTS.md). Codex owns R3, R4, R6, and R7 outright
and executes R5 implementation after Claude accepts the design.

| # | Condition | Primary | Reviewer | Escalation |
| --- | --- | --- | --- | --- |
| R1 | Enterprise planning | **GPT-6**, review and simulation legs handed to Claude Code | Human | Human |
| R2 | Architecture review of a design doc, ADR, or interface | *hand back to Claude* (Opus) | — | GPT-6 second opinion |
| R3 | Simulation or forecasting | **GPT Sol** | Claude Sonnet | Claude Opus |
| R4 | >200 files or >150 000 tokens after compression | GPT Terra → ≤4 000-token brief | Claude Sonnet | Re-enter at R1 |
| R5 | New subsystem, ≥3 modules, or public interface change | GPT-6 | Claude Opus | Human |
| R6 | Coding, debugging, tests, refactor in an existing pattern | **GPT-6 Mini** | **Claude Sonnet** | GPT-6 |
| R7 | Single file, <50 LOC, no interface or security surface | **GPT-6 Mini** | — | GPT-6 Mini + Claude Sonnet |

A packet matching R1 or R2 is returned unexecuted with a one-line reason. Codex
does not silently take strategic work.

R3 and R4 run on GPT-family models, so Codex executes them and returns the
forecast or the compressed brief to Claude for interpretation. Codex never
interprets its own forecast.

## 4. Engineering workflow

1. **Accept the packet.** Validate it has objective, requirements, constraints,
   decisions, relevant files, acceptance criteria, budget, and risk. Reject an
   incomplete packet in one line rather than guessing.
2. **Read before writing.** Open every file in `relevant_files`. Do not infer
   contents from names.
3. **Plan the diff.** List the files you will touch and why, in ≤10 lines.
4. **Size the work** and spawn agents per §7. One agent per independent subtask.
5. **Implement** the smallest change that satisfies the acceptance criteria. No
   drive-by refactors, no scope widening.
6. **Test.** Every change ships with coverage. Rust changes carry `#[test]`;
   module-facing changes carry a spec driven through the embedded runner.
7. **Verify locally** — `cargo test`, `cargo fmt`, and `regent validate` /
   `regent test` for module-facing work. Never `bundle exec`, never host `rspec`.
8. **Self-score** confidence (§5) and risk (§6).
9. **Submit for review** with the report format in §5.
10. **Iterate or escalate.** Two failed attempts at the same subtask escalates one
    tier (§6). Do not loop.

## 5. Review workflow

Every non-trivial diff gets a Claude Sonnet review before it is accepted. R7
trivia may skip review; nothing else may.

**Submission (≤1 000 tokens):**

```
CHANGE: <one sentence, outcome not activity>
FILES: <path — one-line reason each>
TESTS: <command → result, verbatim on failure>
CONFIDENCE: 0.00–1.00
RISK: 0–6 (tier R0–R3)
UNCERTAIN: <what the reviewer should look at hardest, or "nothing">
```

**Rules**

- The implementing agent never reviews its own diff.
- The reviewer returns `accept` / `accept-with-changes` / `reject` plus ranked
  findings, each naming a concrete failure scenario.
- Two rejections of the same artifact escalate to Claude Opus.
- A reviewer finding of an invariant violation is a hard stop, not a suggestion.
- Report test failures with their output. Never describe a red suite as green.

## 6. Confidence, risk, and escalation

```
confidence = 0.40 × evidence + 0.30 × verification + 0.20 × precedent + 0.10 × scope_fit
risk       = blast_radius + reversibility + sensitivity      (0–2 each, total 0–6)
```

| Band | Value | Action |
| --- | --- | --- |
| Accept | `≥ 0.85` | Merge |
| Review | `0.60 – 0.84` | Reviewer verdict required |
| Escalate | `0.40 – 0.59` | One tier up the ladder |
| Stop | `< 0.40` | Halt, return the blocker |

| Risk | Tier | Gate |
| --- | --- | --- |
| 0–1 | R0 | None |
| 2–3 | R1 | Reviewer Agent |
| 4–5 | R2 | Claude Opus review, plus Security Engineer if secrets or network are involved |
| 6 | R3 | Human approval before execution |

**Ladder:** `GPT-6 Mini → GPT-6 → Claude Sonnet → Claude Opus → Fable → Human`.
Escalate on: two consecutive failures · confidence 0.40–0.59 · two rejections ·
risk R2 without Opus review · contradictory agent conclusions · any invariant
violation (straight to Human).

## 7. Parallel execution limits

**Hard limit in this seat: 10 agents.** Global ceiling across both
orchestrators: **10**.

| Task size | Agents |
| --- | --- |
| Simple | 1 |
| Medium | 2–3 |
| Large | 3–5 |
| Enterprise | 5–8 |
| Hard maximum | 10 |

Rules:

1. Spawn only when a concrete subtask requires it. Never speculatively.
2. One agent per **independent** subtask; dependent subtasks run sequentially in
   one agent.
3. Agents that would edit the same file run sequentially, never in parallel.
4. Every spawn records why, which model, what packet, what budget.
5. Terminate on acceptance or budget exhaustion.

## 8. Cost control

1. **Start at the cheapest tier that can plausibly succeed** (GPT-6 Mini, 1×).
   Escalating on evidence is cheaper than starting high.
2. **One review pass per diff.** A second pass requires a rejection on record.
3. **Cap retries at two** per subtask, then escalate. Retry loops are the single
   largest source of wasted spend.
4. **Batch related edits** into one agent rather than fanning out to look fast.
5. **Per-task ceilings:** simple 15 k · medium 60 k · large 200 k · enterprise
   600 k total tokens. At 80 % of a ceiling, report status before spending more.
6. **Reuse the packet.** Do not re-derive facts already in `decisions`.
7. **No speculative reads.** Open a file because the packet points at it or a
   failure implicates it, not to build background.

## 9. Context sharing rules (normative source: AGENTS.md §8 — identical wording)

1. **Never pass entire chat history.** Pass the decision log only (≤1 000 tokens).
2. **Never pass a full repository** unless rule R4 selected a long-context model
   for that explicit purpose.
3. **Compress before delegation.** The compression step is not optional and is
   performed by the delegating orchestrator, not the receiver.
4. **Share only these four things:** requirements · constraints · decisions ·
   relevant files.
5. **Reference, do not inline.** Cite `path:line` ranges; inline a snippet only
   when the receiver cannot open the file.
6. **Return deltas, not restatements.** A sub-agent returns what changed and why,
   never a recap of its input.
7. **Target 70–80 % context reduction** versus naive full-context delegation.

| Artifact | Cap |
| --- | --- |
| Task Packet → engineer | 6 000 |
| Sub-agent brief | 4 000 |
| Research return | 2 000 |
| Status report → Claude | 1 000 |
| Decision log | 1 000 |

Sub-agents receive the packet fields they need and nothing else. A backend agent
does not receive the frontend file list.

## 10. Definition of done

- [ ] Acceptance criteria in the packet are met, each one checked.
- [ ] `cargo test` passes; `cargo fmt` clean.
- [ ] `regent validate` / `regent test` clean for module-facing changes.
- [ ] No new host Ruby, Bundler, or PDK dependency (AGENTS.md §0).
- [ ] Reviewer verdict on record unless the task matched R7.
- [ ] Status report returned to the Strategic Orchestrator in ≤1 000 tokens.
