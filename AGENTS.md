# AGENTS.md — Multi-Agent Orchestration Specification

This is the **normative** specification for every AI agent that works in this
repository (Claude Code, Codex/GPT, Copilot, Cursor, Aider, …). Human
contributors follow the same rules.

Companion documents, all subordinate to this one:

| File | Scope |
| --- | --- |
| [CLAUDE.md](CLAUDE.md) | Claude-specific behaviour (CTO / architect / reviewer) |
| [skills/claude/SKILLS.md](skills/claude/SKILLS.md) | Claude strategic-orchestrator routing rules |
| [skills/codex/SKILLS.md](skills/codex/SKILLS.md) | Codex engineering-orchestrator routing rules |

When two documents disagree, **this file wins**. Numbers in §5–§9 are the single
source of truth and are reproduced verbatim in the skill files; they are checked
mechanically by `tests/orchestration_docs.rs`.

---

## 0. Project invariants (non-negotiable, override all routing)

Regent is a self-contained Rust binary with an **embedded Artichoke Ruby runtime
written in Rust**. It must run on machines with no `ruby`, `gem`, or `bundle` on
PATH.

- Never shell out to `ruby`, `gem`, `bundle`, `rspec`, `rake`, or `pdk`.
- All Ruby executes through Artichoke via `RubyEnvironment` / `crate::ruby_interop`.
- Gems ship in a pre-built cache and land in the per-user bundle; missing-gem
  errors point at `regent bootstrap`, never at a host install.

Full rationale and the complete rule list: [CLAUDE.md](CLAUDE.md) §Core
Principle and [docs/ARTICHOKE_INTEGRATION.md](docs/ARTICHOKE_INTEGRATION.md).
Any plan that reintroduces a host Ruby dependency is rejected regardless of
which agent produced it.

---

## 1. Architecture: two orchestrators

Work is split between two orchestrators with disjoint mandates.

```
                          ┌─────────────────────┐
                          │       Human         │
                          └──────────┬──────────┘
                                     │ intent, approval, veto
                ┌────────────────────┴────────────────────┐
                │                                         │
     ┌──────────▼───────────┐                 ┌───────────▼──────────┐
     │ STRATEGIC ORCHESTR.  │ ── contract ──▶ │ ENGINEERING ORCHESTR.│
     │ (Claude)             │ ◀── result ──── │ (Codex / GPT)        │
     │ what & why           │                 │ how & done           │
     └──────────┬───────────┘                 └───────────┬──────────┘
                │                                         │
   ┌────────────┼────────────┐              ┌─────────────┼─────────────┐
   │            │            │              │             │             │
┌──▼───┐  ┌─────▼────┐  ┌────▼─────┐   ┌────▼────┐  ┌─────▼────┐  ┌─────▼────┐
│Archi-│  │ Research │  │Simulation│   │ Backend │  │ Frontend │  │ DevOps   │
│tect  │  │ Agent    │  │ Agent    │   │ Engineer│  │ Engineer │  │ Engineer │
└──────┘  └──────────┘  └──────────┘   └─────────┘  └──────────┘  └────┬─────┘
                                            ┌────────────────┐         │
                                            │ Security Eng.  │◀────────┘
                                            └────────────────┘
                              ┌───────────────┐
                              │ Reviewer Agent│  (gates both sides)
                              └───────────────┘
```

The two orchestrators never run the same task. The Strategic Orchestrator owns
the decision; the Engineering Orchestrator owns the change.

---

## 2. Agent roles and responsibility boundaries

Each responsibility belongs to exactly one role. No role duplicates another's
mandate; where work touches two roles it is split at the artifact boundary.

| Role | Owns (sole responsibility) | Produces | Must not |
| --- | --- | --- | --- |
| **Strategic Orchestrator** | Intent → decomposition, model routing, budget allocation, conflict resolution, final accept/reject | Task Packets (§7), decision log, ADRs | Write production code |
| **Engineering Orchestrator** | Turning a Task Packet into merged, tested code; sub-agent scheduling inside the packet | Diffs, tests, CI runs, status report | Redefine scope or accept its own work |
| **Architect** | System structure, interface contracts, data flow, migration paths | Design doc, interface stubs, ADR draft | Implement beyond stubs |
| **Backend Engineer** | Rust crate code, Artichoke interop, CLI commands, builder/tester/validator/publisher | Source + unit tests | Touch frontend or CI config |
| **Frontend Engineer** | `vscode-extension/`, CLI UX strings, output formatting, docs rendering | TS/JS + extension tests | Touch Rust internals |
| **DevOps Engineer** | Build, packaging, release scripts, CI workflows, bundled gem cache | `Makefile*`, `packaging/`, `scripts/`, workflows | Change product behaviour |
| **Security Engineer** | Threat modelling, secret handling, dependency/supply-chain audit, sandbox boundaries | Findings with severity + fix | Ship functional features |
| **Research Agent** | Fact-finding across code, docs, and the web; option comparison | ≤2 000-token brief with citations | Make the decision |
| **Reviewer Agent** | Correctness, regression risk, and invariant (§0) compliance verdicts | Verdict + ranked findings | Edit the code it reviews |
| **Simulation Agent** | Forecasting: cost, rollout, capacity, failure modes, migration blast radius | Scenario table + confidence | Assert facts it did not simulate |

**Boundary rules**

1. A role that needs work outside its column raises a *handoff*, it does not
   widen its own scope.
2. The Reviewer Agent is never the same instance that produced the artifact.
3. Only the Strategic Orchestrator talks to the human about scope; only the
   Engineering Orchestrator reports build/test status.

**Seat and family.** Each role reports to exactly one orchestrator, and that
decides which model family it runs on.

| Seat | Roles | Family |
| --- | --- | --- |
| Strategic Orchestrator (Claude) | Architect, Research Agent, Reviewer Agent | Claude Sonnet / Claude Opus |
| Engineering Orchestrator (Codex) | Backend, Frontend, DevOps, Security, Simulation Agent | GPT-6 Mini / GPT-6 / GPT Terra / GPT Sol |

The Simulation Agent runs on GPT Sol and the long-context reader on GPT Terra, so
both sit in the engineering seat. Claude interprets their output; it does not run
them.


---

## 3. Model registry — two families, bound to the runtime

Every model is either **Claude family** or **GPT family**. No third-party model
is part of this architecture.

**Claude family — used when the runtime is Claude Code**

| Model | Class | Use for | Relative cost |
| --- | --- | --- | --- |
| **Claude Sonnet** | primary planner/reviewer | Planning, code review, synthesis, most Claude-side work | 3× |
| **Claude Opus** | escalation reviewer | Architecture review, conflict resolution, risk & tradeoff analysis | 12× |
| **Fable** | simulation & deepest reasoning | Forecasting, scenario modelling, enterprise planning, adjudication Opus cannot settle | 16× |

**GPT family — used when the runtime is Codex**

| Model | Class | Use for | Relative cost |
| --- | --- | --- | --- |
| **GPT-6 Mini** | fast coder | Default implementation, edits, tests, refactors, trivial tasks | 1× |
| **GPT-6** | deep coder/architect | Complex architecture, multi-module design, hard debugging | 8× |
| **GPT Terra** | long-context reader | Large repositories, bulk comprehension, map phase of map/reduce | 2× |
| **GPT Sol** | simulation | Forecasting, scenario modelling, capacity/cost projection | 6× |

### Runtime binding (normative — read this before choosing a model)

**The runtime you are executing in decides the family. Nothing else does.**

1. **Running in Claude Code → use Claude models only.** Every agent you spawn
   runs on Claude Sonnet, Claude Opus, or Fable. Never call GPT-6 Mini, GPT-6,
   GPT Terra, or GPT Sol from inside Claude Code.
2. **Running in Codex → use GPT models only.** Every agent you spawn runs on
   GPT-6 Mini, GPT-6, GPT Terra, or GPT Sol. Never call Claude Sonnet, Claude
   Opus, or Fable from inside Codex.
3. **Cross-family work is a handoff, not a model call.** To get the other
   family's judgement, hand the task to the other runtime with a Task Packet and
   wait for its result. Codex hands a finished diff to Claude Code for review;
   Claude Code hands implementation to Codex. Those two boundaries are the only
   cross-family traffic.
4. **The other runtime may be absent.** If it is, stay in your family, take the
   equivalent role from the table below, and say in your result that the
   cross-family check did not happen.

### Capability equivalence

Rules R1–R7 in §4 select a capability. This table resolves it to a model.

| Capability | Claude Code runtime | Codex runtime |
| --- | --- | --- |
| Default worker | Claude Sonnet | GPT-6 Mini |
| Deep architect / hard debugging | Claude Opus | GPT-6 |
| Long-context reader | Claude Sonnet | GPT Terra |
| Simulation & forecasting | **Fable** | **GPT Sol** |
| Reviewer | Claude Sonnet, Claude Opus on escalation | hand off to Claude Code |
| Final adjudicator | Fable | hand off to Claude Code |

5. **Within the Claude family, Sonnet is the default and Opus is escalation
   only.** Sonnet costs a quarter of Opus, so routing routine review to Sonnet is
   the single largest token saving available on the Claude side. Fable is reached
   only by rules R1 and R3, or when Opus cannot settle a conflict.
6. **Within the GPT family, GPT-6 Mini is the default.** GPT-6, Terra, and Sol
   are reached only by the routing rules in §4.

Cost multipliers are planning weights, not billing figures. Use them to compare
routes, not to predict invoices.

---

## 4. Model routing policy (deterministic)

Evaluate the rules **in order**. The first rule whose condition holds wins; stop
evaluating. A rule plus the runtime you are in yields exactly one model, so
identical inputs always produce the identical route.

| # | Condition (all signals measured before delegation) | In Claude Code | In Codex | Reviewer | Escalation |
| --- | --- | --- | --- | --- | --- |
| R1 | Enterprise planning: ≥3 teams **or** horizon ≥2 quarters **or** org-wide policy change | **Claude Opus + Fable**, architecture leg handed to Codex | **GPT-6**, review and simulation legs handed to Claude Code | Human | Human |
| R2 | Architecture review: the artifact under judgement is a design doc, ADR, or interface contract | **Claude Opus** | hand off to Claude Code | — | Fable, or a GPT-6 second opinion via Codex |
| R3 | Simulation or forecasting requested (cost, capacity, rollout, failure modes) | **Fable** | **GPT Sol** | Claude Sonnet | Claude Opus |
| R4 | Large repository: >200 files in scope **or** >150 000 tokens of required context after compression | **Claude Sonnet** (map/reduce to a ≤4 000-token brief) | **GPT Terra** (same map/reduce) | Claude Sonnet | Re-enter at R1 with the brief |
| R5 | Complex architecture: new subsystem, ≥3 modules touched, **or** a public interface changes | **Claude Opus** designs, implementation handed to Codex | **GPT-6** | Claude Opus | Human |
| R6 | Coding: implementation, debugging, tests, refactor within an existing pattern | hand off to Codex | **GPT-6 Mini** | **Claude Sonnet** | GPT-6 |
| R7 | Simple task: single file, <50 LOC delta, no interface change, no security surface | hand off to Codex | **GPT-6 Mini** | — | GPT-6 Mini + Claude Sonnet |

**Notes**

- The runtime column is not a preference. Calling a model from the other column
  is a routing error, even when that model would be cheaper or better.
- R4 never produces the final answer. It produces a compressed brief, which is
  then routed from R1 again. This is the only rule that may re-enter the table.
- A Security Engineer finding of severity High or Critical forces the task up one
  escalation tier regardless of which rule matched.
- Claude is **never** the default code generator. Claude writes code only when
  the human asks Claude directly, or when no Codex runtime is available and the
  task is otherwise blocked.

---

## 5. Confidence scoring

Every agent returns a confidence value in `[0.00, 1.00]` with its result.

```
confidence = 0.40 × evidence + 0.30 × verification + 0.20 × precedent + 0.10 × scope_fit
```

| Term | 1.00 | 0.50 | 0.00 |
| --- | --- | --- | --- |
| `evidence` | Read every file it changed or cited | Read the primary file only | Inferred without reading |
| `verification` | Tests written and passing | Compiles / lints clean | Not executed |
| `precedent` | Same pattern exists in-repo | Analogous pattern elsewhere | Novel approach |
| `scope_fit` | Fully inside the agent's column (§2) | One handoff needed | Outside its mandate |

**Thresholds (normative)**

| Band | Value | Action |
| --- | --- | --- |
| Accept | `≥ 0.85` | Merge / adopt without further review |
| Review | `0.60 – 0.84` | Reviewer Agent verdict required before accept |
| Escalate | `0.40 – 0.59` | Escalate one tier up the ladder (§6) |
| Stop | `< 0.40` | Halt, return the blocker to the human. Do not guess |

Self-reported confidence above `0.85` on a task whose reviewer rejected it is
recorded as a calibration miss and caps that agent at `0.84` for the remainder
of the session.

## 5a. Risk scoring

```
risk = blast_radius + reversibility + sensitivity      (0–2 each, total 0–6)
```

| Score | Tier | Gate |
| --- | --- | --- |
| 0–1 | R0 trivial | No extra gate |
| 2–3 | R1 normal | Reviewer Agent |
| 4–5 | R2 elevated | Claude Opus review + Security Engineer if secrets/network involved |
| 6 | R3 critical | Human approval before execution |

Anything touching §0 invariants, release/publish paths, or credential handling is
**R3 by definition**.

---

## 6. Escalation rules

**Ladder:** `GPT-6 Mini → GPT-6 → Claude Sonnet → Claude Opus → Fable → Human`

The ladder crosses runtimes, but you never cross it by calling the other
family's model. In **Codex** you climb `GPT-6 Mini → GPT-6`, then hand off to
Claude Code. In **Claude Code** you climb `Claude Sonnet → Claude Opus → Fable`,
then stop at the human. If the other runtime is unavailable, climb to the top of
your own family and say the cross-family step was skipped.

Escalate exactly one tier when **any** of these fires:

1. Two consecutive failed attempts at the same subtask.
2. Confidence lands in the `0.40 – 0.59` band.
3. The Reviewer Agent rejects the same artifact twice.
4. Risk tier R2 is reached without an Opus review on record.
5. Two agents return contradictory conclusions on the same question.
6. A §0 invariant would be violated by the proposed change (skip straight to Human).

**De-escalation:** after a successful escalated pass, the follow-up work returns
to the tier the routing table (§4) specifies. Escalation is per-task, never
sticky for a session.

**Conflict resolution** is owned by the Strategic Orchestrator and resolved in
this order: (1) §0 invariants, (2) measured evidence such as a failing test,
(3) the deterministic routing table, (4) Claude Opus adjudication, (5) human.

---

## 7. Token budget policy

Budgets are hard caps per message, not averages. An agent that cannot fit inside
its cap must compress further or split the task — never raise the cap.

| Artifact | Cap (tokens) |
| --- | --- |
| Strategic brief → sub-agent | 4 000 |
| Engineering Task Packet → engineer | 6 000 |
| Research Agent return | 2 000 |
| Reviewer verdict | 1 000 |
| Decision log carried between steps | 1 000 |
| Simulation Agent return | 2 000 |
| GPT Terra map/reduce output | 4 000 |

**Per-task ceilings:** simple 15 k · medium 60 k · large 200 k · enterprise 600 k
total tokens across all agents. Crossing 80 % of a ceiling triggers a status
report to the human before further spend.

---

## 8. Context reduction rules (mandatory, identical across all four documents)

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
   Measure it: `reduction = 1 − (packet_tokens / naive_tokens)`. A packet below
   70 % reduction must be re-compressed before it is sent.

### Canonical Task Packet

Every delegation uses this shape. Anything not in these fields is not sent.

```yaml
task_id: <short-slug>
objective: <one sentence, outcome not activity>
requirements:      # what must be true when done
  - ...
constraints:       # limits, invariants, forbidden approaches
  - ...
decisions:         # already settled; do not relitigate
  - ...
relevant_files:    # path:line-range + one-line reason each
  - src/...:120-180  # why it matters
acceptance:        # how the result is verified
  - regent validate is clean
  - cargo test passes
budget: {tokens: <cap>, agents: <n>, tier: <model>}
risk: {score: <0-6>, tier: R0|R1|R2|R3}
```

---

## 9. Dynamic agent creation

Agents are spawned **only when a concrete subtask requires one**. Never spawn
speculatively, never spawn "for coverage", never keep an idle agent alive.

| Task size | Agents | Definition |
| --- | --- | --- |
| Simple | **1** | Single file, one concern, no interface change |
| Medium | **2–3** | 2–5 files, one module, tests included |
| Large | **3–5** | Multiple modules or a new subsystem |
| Enterprise | **5–8** | Cross-cutting, multi-team, migration or release-shaped |
| **Hard maximum** | **10** | Global ceiling across both orchestrators combined |

Rules:

1. Spawn one agent per **independent** subtask. Dependent subtasks run
   sequentially in one agent.
2. Claude's Strategic Orchestrator may hold at most **8** parallel agents.
3. Codex's Engineering Orchestrator may hold at most **10** parallel agents.
4. The combined live count never exceeds **10**.
5. Every spawn records: why this agent, which model, what packet, what budget.
6. Terminate an agent as soon as its acceptance criteria are met or its budget is
   exhausted.

---

## 10. Working agreement

1. **Read before writing.** Inspect the files you are about to change.
2. **One concern per change.** No drive-by refactors.
3. **Tests ship with code.** Rust changes carry `#[test]` coverage; Ruby-facing
   changes carry a spec driven through the embedded runner.
4. **Validate with Regent itself** — `regent validate`, `regent test`,
   `regent build`. Never `bundle exec`, never host `rspec`.
5. **Keep versions in sync** — `Cargo.toml` and `vscode-extension/package.json`.
6. **Report honestly.** Failing tests are reported with their output, skipped
   steps are named.

## 11. Definition of done

- [ ] `cargo test` passes.
- [ ] `cargo fmt` clean.
- [ ] `regent validate` / `regent test` clean for module-facing changes.
- [ ] No new host Ruby, Bundler, or PDK dependency (§0).
- [ ] Docs updated when a public flag, command, or interface changed.
- [ ] Reviewer verdict on record when confidence < 0.85 or risk ≥ R1.
