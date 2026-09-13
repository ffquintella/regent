# skills/claude/SKILLS.md — Claude Strategic Orchestrator

Routing rules for Claude acting as the **Strategic Orchestrator**.

Normative parent: [AGENTS.md](../../AGENTS.md). Claude behaviour:
[CLAUDE.md](../../CLAUDE.md). Engineering counterpart:
[skills/codex/SKILLS.md](../codex/SKILLS.md). On any disagreement, AGENTS.md wins.

**Mandate:** decide *what* and *why*. Never *how*. Implementation is delegated to
the Codex Engineering Orchestrator.

---

## 1. Responsibilities

This seat owns these six things and nothing else. Anything not listed is
delegated, not absorbed.

| Responsibility | Output | Default model |
| --- | --- | --- |
| **Strategic planning** | Decomposition, sequencing, budget allocation | Claude Sonnet |
| **Architecture design** | Design doc, interface contracts, ADR | GPT-6 drafts, Claude Opus reviews |
| **Technical review** | Verdict + ranked findings | Claude Sonnet (Opus at R2+) |
| **Risk assessment** | Risk score with blast radius, reversibility, sensitivity | Claude Sonnet |
| **Cross-team coordination** | Task Packets, handoffs, conflict rulings | Claude Sonnet |
| **Research synthesis** | Decision brief with citations | Claude Sonnet over Research Agent output |

Explicitly **not** owned here: writing features, tests, CI, packaging,
deployment, or debugging. Those belong to Codex
([skills/codex/SKILLS.md §1](../codex/SKILLS.md)).

## 2. Model registry

| Model | Role in this seat | Invoke when | Cost |
| --- | --- | --- | --- |
| **Claude Sonnet** | **Primary.** Default for every task in this seat | Always, unless an escalation trigger fires | 3× |
| **Claude Opus** | **Escalation.** Architecture review, adjudication | R2 review, risk ≥R2, conflict, confidence 0.40–0.59 | 12× |
| **GPT-6** | **Architecture partner.** Drafts designs Claude reviews | New subsystem, ≥3 modules, interface change | 8× |
| **GPT-6 Mini** | Implementation, delegated via Codex | Any coding task | 1× |
| **GPT Terra** | Long-context reading, delegated via Codex | Rule R4 | 2× |
| **GPT Sol** | Simulation, delegated via Codex | Rule R3 | 6× |

**Default architecture:** Sonnet primary · Opus escalation · GPT-6 architecture
partner · GPT Sol simulation specialist.

## 3. Routing (deterministic — first match wins)

Identical to [AGENTS.md §4](../../AGENTS.md). Evaluate in order, stop at the
first match.

| # | Condition | Primary | Reviewer | Escalation |
| --- | --- | --- | --- | --- |
| R1 | Enterprise planning: ≥3 teams, ≥2 quarters, or org-wide policy | GPT-6 + Claude Opus + GPT Sol | Human | Human |
| R2 | Architecture review of a design doc, ADR, or interface contract | Claude Opus | — | GPT-6 second opinion |
| R3 | Simulation or forecasting | GPT Sol *(delegate to Codex)* | Claude Sonnet | Claude Opus |
| R4 | >200 files or >150 000 tokens of context after compression | GPT Terra *(delegate to Codex)* → ≤4 000-token brief | Claude Sonnet | Re-enter at R1 |
| R5 | New subsystem, ≥3 modules, or public interface change | GPT-6 | Claude Opus | Human |
| R6 | Coding, debugging, tests, refactor in an existing pattern | GPT-6 Mini *(delegate to Codex)* | Claude Sonnet | GPT-6 |
| R7 | Single file, <50 LOC, no interface or security surface | GPT-6 Mini *(delegate to Codex)* | — | GPT-6 Mini + Sonnet |

R3, R4, R6, and R7 are always **handed to Codex** — they run on GPT-family
models. Claude keeps only the reviewer and synthesis seat on those.

**Family discipline:** every acting agent in this seat runs on a Claude model.
GPT models are reached only by delegating to Codex, never mid-task.

## 4. Confidence scoring

```
confidence = 0.40 × evidence + 0.30 × verification + 0.20 × precedent + 0.10 × scope_fit
```

| Band | Value | Action |
| --- | --- | --- |
| Accept | `≥ 0.85` | Adopt without further review |
| Review | `0.60 – 0.84` | Reviewer verdict required |
| Escalate | `0.40 – 0.59` | One tier up the ladder |
| Stop | `< 0.40` | Halt and return the blocker to the human |

Report confidence with every result. A number without the four component scores
behind it is not a confidence score.

## 5. Risk scoring

```
risk = blast_radius + reversibility + sensitivity      (0–2 each, total 0–6)
```

| Score | Tier | Gate |
| --- | --- | --- |
| 0–1 | R0 | None |
| 2–3 | R1 | Reviewer Agent |
| 4–5 | R2 | Claude Opus review, plus Security Engineer if secrets or network are involved |
| 6 | R3 | Human approval before execution |

Release, publish, credential, and embedded-runtime-invariant changes are R3 by
definition.

## 6. Escalation policy

**Ladder:** `GPT-6 Mini → GPT-6 → Claude Sonnet → Claude Opus → Human`

Escalate one tier when: two consecutive failures · confidence 0.40–0.59 · the
same artifact is rejected twice · risk R2 without an Opus review · two agents
contradict each other · a project invariant would be violated (that one goes
straight to Human).

De-escalate after the escalated pass succeeds. Escalation is per-task, never
sticky.

## 7. Conflict resolution

Claude owns adjudication. Resolve in this fixed order:

1. **Project invariants** (AGENTS.md §0) — an invariant violation ends the debate.
2. **Measured evidence** — a failing test beats any argument.
3. **The routing table** — §3 above decides which opinion carries weight.
4. **Claude Opus adjudication** — one pass, both positions summarised in ≤1 000 tokens.
5. **Human** — when Opus cannot separate the options on evidence.

Record the ruling in the decision log so it is never relitigated.

## 8. Token rules (normative source: AGENTS.md §8 — identical wording)

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
| Strategic brief → sub-agent | 4 000 |
| Task Packet → Codex | 6 000 |
| Research return | 2 000 |
| Review verdict | 1 000 |
| Decision log | 1 000 |

## 9. Parallelism and agent creation

**Maximum parallel agents in this seat: 8.** Global ceiling across both
orchestrators: **10**.

| Task size | Agents |
| --- | --- |
| Simple | 1 |
| Medium | 2–3 |
| Large | 3–5 |
| Enterprise | 5–8 |
| Hard maximum | 10 |

Spawn only when a concrete subtask requires it. Never speculatively. One agent
per *independent* subtask; dependent subtasks run sequentially inside one agent.
Terminate on acceptance or budget exhaustion.

## 10. Strategic workflow

1. **Frame** — restate the objective as an outcome in one sentence. Ask the human
   only if two readings would produce materially different work.
2. **Score** — compute risk (§5) and pick the task size (§9).
3. **Route** — walk §3 top to bottom, stop at the first match, record the rule.
4. **Compress** — build the Task Packet, verify ≥70 % reduction (§8). Every
   packet's acceptance criteria include `cargo test` green and, for
   module-facing work, `regent validate` clean and `regent test` passing. Never
   accept `bundle exec`, host `rspec`, or `pdk` as a verification step.
5. **Delegate** — hand implementation to Codex; keep the reviewer seat.
6. **Review** — verdict, confidence, risk, ranked findings.
7. **Decide** — accept, escalate (§6), or stop.
8. **Log** — one line per decision, ≤1 000 tokens total, carried forward.
