//! Consistency checks for the dual-orchestrator specification.
//!
//! Four documents describe the same architecture:
//!
//!   AGENTS.md                 normative multi-agent contract
//!   CLAUDE.md                 Claude as strategic orchestrator (CTO / reviewer)
//!   skills/claude/SKILLS.md   Claude routing rules
//!   skills/codex/SKILLS.md    Codex routing rules
//!
//! Routing is only deterministic if the shared constants — model registry,
//! confidence bands, risk tiers, token caps, agent limits, escalation ladder —
//! are stated identically everywhere. These tests fail when one document drifts.
//!
//! The same four documents are scaffolded into every new module by
//! `regent new`; those copies are checked by the unit tests in `src/cli/new.rs`.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const ROOT_DOCS: [&str; 4] = [
    "AGENTS.md",
    "CLAUDE.md",
    "skills/claude/SKILLS.md",
    "skills/codex/SKILLS.md",
];

const TEMPLATE_DOCS: [&str; 4] = [
    "templates/agents/AGENTS.md.template",
    "templates/agents/CLAUDE.md.template",
    "templates/agents/skills_claude_SKILLS.md.template",
    "templates/agents/skills_codex_SKILLS.md.template",
];

fn read(rel: &str) -> String {
    let path: PathBuf = repo_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every document that participates in orchestration: the four at the repo root
/// plus the four templates shipped into generated modules.
fn all_docs() -> Vec<(&'static str, String)> {
    ROOT_DOCS
        .iter()
        .chain(TEMPLATE_DOCS.iter())
        .map(|rel| (*rel, read(rel)))
        .collect()
}

#[test]
fn all_four_documents_exist_at_the_repo_root() {
    for rel in ROOT_DOCS {
        assert!(
            Path::new(&repo_root().join(rel)).is_file(),
            "{rel} is required by the dual-orchestrator architecture"
        );
    }
}

#[test]
fn only_claude_and_gpt_family_models_appear() {
    // The architecture is restricted to two families. A third-party model in
    // any document means the routing table can send work outside them.
    const FOREIGN: [&str; 5] = ["Gemini", "Llama", "Mistral", "Grok", "Command R"];
    for (rel, body) in all_docs() {
        for model in FOREIGN {
            assert!(
                !body.contains(model),
                "{rel} mentions {model}; only Claude-family and GPT-family models are allowed"
            );
        }
    }
}

#[test]
fn the_runtime_binds_the_model_family() {
    // The runtime decides the family: Claude Code runs Claude models, Codex runs
    // GPT models, and crossing over is a handoff rather than a model call.
    let agents = read("AGENTS.md");
    for needle in [
        "Running in Claude Code → use Claude models only",
        "Running in Codex → use GPT models only",
        "Cross-family work is a handoff, not a model call",
        "| Simulation & forecasting | **Fable** | **GPT Sol** |",
    ] {
        assert!(
            agents.contains(needle),
            "AGENTS.md must state the runtime binding rule {needle:?}"
        );
    }

    let claude = read("CLAUDE.md");
    assert!(claude.contains("you run on Claude models: Sonnet, Opus, and"));
    assert!(claude.contains("Never call a GPT model from here"));

    let claude_skills = read("skills/claude/SKILLS.md");
    assert!(claude_skills.contains("You are running in Claude Code, so you run on Claude models"));
    assert!(claude_skills.contains(
        "never by
calling them from here"
    ));

    let codex_skills = read("skills/codex/SKILLS.md");
    assert!(codex_skills.contains("You are running in Codex, so you run on GPT models"));
    assert!(
        codex_skills.contains("Never call Claude Sonnet, Claude Opus, or Fable from inside Codex")
    );
}

#[test]
fn fable_is_claude_family_and_reached_only_by_rule() {
    // Fable is the most expensive model in the registry; it must never be a
    // default, and it must never appear as a Codex-seat model.
    let agents = read("AGENTS.md");
    assert!(
        agents.contains("**Fable** | simulation & deepest reasoning"),
        "Fable belongs to the Claude-family registry table"
    );
    let claude_skills = read("skills/claude/SKILLS.md");
    assert!(claude_skills.contains("Rules R1 and R3, or a conflict Opus cannot settle"));

    let codex_skills = read("skills/codex/SKILLS.md");
    for line in codex_skills.lines() {
        let is_registry_row = line.starts_with("| **GPT") || line.starts_with("| 1 |");
        assert!(
            !(is_registry_row && line.contains("Fable")),
            "Codex must not list Fable as one of its own models: {line}"
        );
    }
}

#[test]
fn each_seat_states_its_family_discipline() {
    // Claude interactions run on Claude models, Codex interactions on GPT
    // models; the families meet only at an orchestrator handoff.
    for rel in [
        "AGENTS.md",
        "CLAUDE.md",
        "skills/claude/SKILLS.md",
        "skills/codex/SKILLS.md",
        "templates/agents/AGENTS.md.template",
        "templates/agents/CLAUDE.md.template",
        "templates/agents/skills_claude_SKILLS.md.template",
        "templates/agents/skills_codex_SKILLS.md.template",
    ] {
        let body = read(rel);
        assert!(
            body.contains("Runtime binding")
                || body.contains("Family discipline")
                || body.contains("family discipline"),
            "{rel} must state which model family the acting agent runs on"
        );
    }

    let claude = read("skills/claude/SKILLS.md");
    assert!(claude.contains("every acting agent in this seat runs on a Claude model"));
    let codex = read("skills/codex/SKILLS.md");
    assert!(codex.contains("every acting agent in this seat runs on a GPT model"));
}

#[test]
fn every_document_lists_the_full_model_registry() {
    // A document that omits a model cannot route to it.
    const MODELS: [&str; 7] = [
        "GPT-6 Mini",
        "GPT-6",
        "GPT Terra",
        "GPT Sol",
        "Claude Sonnet",
        "Claude Opus",
        "Fable",
    ];
    for (rel, body) in all_docs() {
        for model in MODELS {
            assert!(
                body.contains(model),
                "{rel} never mentions {model}; the model registry must be complete"
            );
        }
    }
}

#[test]
fn token_optimization_rules_are_identical_everywhere() {
    // These four sentences are the normative context-reduction policy
    // (AGENTS.md §8). They are reproduced verbatim in every other document.
    const RULES: [&str; 4] = [
        "**Never pass entire chat history.** Pass the decision log only (≤1 000 tokens).",
        "**Compress before delegation.**",
        "**Share only these four things:** requirements · constraints · decisions ·",
        "**Target 70–80 % context reduction** versus naive full-context delegation.",
    ];
    for (rel, body) in all_docs() {
        for rule in RULES {
            assert!(
                body.contains(rule),
                "{rel} does not restate the token rule verbatim: {rule:?}"
            );
        }
    }
}

#[test]
fn confidence_and_risk_scales_match_everywhere() {
    const BANDS: [&str; 5] = [
        "`≥ 0.85`",
        "`0.60 – 0.84`",
        "`0.40 – 0.59`",
        "`< 0.40`",
        "blast_radius + reversibility + sensitivity",
    ];
    for (rel, body) in all_docs() {
        for band in BANDS {
            assert!(
                body.contains(band),
                "{rel} is missing the shared scoring band {band:?}"
            );
        }
    }
}

#[test]
fn the_escalation_ladder_is_stated_identically_everywhere() {
    const LADDER: &str = "GPT-6 Mini → GPT-6 → Claude Sonnet → Claude Opus → Fable → Human";
    for (rel, body) in all_docs() {
        assert!(
            body.contains(LADDER),
            "{rel} must state the escalation ladder verbatim"
        );
    }
}

#[test]
fn agent_limits_are_consistent() {
    // 8 for Claude, 10 for Codex, 10 combined. Stated in AGENTS.md §9 and
    // repeated in both skill files; the Claude documents also carry the caps.
    let agents = read("AGENTS.md");
    for needle in [
        "| Simple | **1** |",
        "| Medium | **2–3** |",
        "| Large | **3–5** |",
        "| Enterprise | **5–8** |",
        "| **Hard maximum** | **10** |",
        "at most **8** parallel agents",
        "at most **10** parallel agents",
        "never exceeds **10**",
    ] {
        assert!(agents.contains(needle), "AGENTS.md is missing {needle:?}");
    }

    let claude_skills = read("skills/claude/SKILLS.md");
    assert!(claude_skills.contains("**Maximum parallel agents in this seat: 8.**"));
    assert!(claude_skills.contains("orchestrators: **10**"));

    let codex_skills = read("skills/codex/SKILLS.md");
    assert!(codex_skills.contains("**Hard limit in this seat: 10 agents.**"));
    assert!(codex_skills.contains("orchestrators: **10**"));

    // The sizing ladder is the same table in both skill files.
    for (rel, body) in [
        ("skills/claude/SKILLS.md", &claude_skills),
        ("skills/codex/SKILLS.md", &codex_skills),
    ] {
        for row in [
            "| Simple | 1 |",
            "| Medium | 2–3 |",
            "| Large | 3–5 |",
            "| Enterprise | 5–8 |",
            "| Hard maximum | 10 |",
        ] {
            assert!(
                body.contains(row),
                "{rel} is missing the sizing row {row:?}"
            );
        }
    }
}

#[test]
fn routing_rules_are_numbered_and_ordered_the_same_way() {
    // Determinism comes from evaluating R1..R7 in order and stopping at the
    // first match. Any document that reorders or drops a rule breaks that.
    for rel in [
        "AGENTS.md",
        "skills/claude/SKILLS.md",
        "skills/codex/SKILLS.md",
        "templates/agents/AGENTS.md.template",
        "templates/agents/skills_claude_SKILLS.md.template",
        "templates/agents/skills_codex_SKILLS.md.template",
    ] {
        let body = read(rel);
        let mut cursor = 0usize;
        for rule in [
            "| R1 |", "| R2 |", "| R3 |", "| R4 |", "| R5 |", "| R6 |", "| R7 |",
        ] {
            let at = body[cursor..].find(rule).unwrap_or_else(|| {
                panic!("{rel} is missing routing rule {rule:?} after R-{cursor}")
            });
            cursor += at + rule.len();
        }
        assert!(
            body.contains("first match wins") || body.contains("first rule whose condition holds"),
            "{rel} must state that routing stops at the first match"
        );
    }
}

#[test]
fn responsibilities_do_not_overlap_between_the_two_seats() {
    // Each skill file claims its own column and explicitly disclaims the other's.
    let claude = read("skills/claude/SKILLS.md");
    let codex = read("skills/codex/SKILLS.md");

    assert!(
        claude.contains("Explicitly **not** owned here"),
        "the Claude seat must disclaim engineering work"
    );
    assert!(
        codex.contains("Explicitly **not** owned here"),
        "the Codex seat must disclaim strategic work"
    );
    assert!(
        claude.contains("Never *how*"),
        "the Claude seat owns what and why, never how"
    );
    assert!(
        codex.contains("Never redefine scope"),
        "the Codex seat must not redefine scope"
    );
}

#[test]
fn claude_is_never_the_default_code_generator() {
    for rel in [
        "AGENTS.md",
        "CLAUDE.md",
        "templates/agents/AGENTS.md.template",
        "templates/agents/CLAUDE.md.template",
    ] {
        let body = read(rel);
        assert!(
            body.contains("never** the default code generator")
                || body.contains("Claude is not the default code generator"),
            "{rel} must state that Claude does not generate code by default"
        );
    }
}

#[test]
fn the_no_host_ruby_invariant_survives_the_refactor() {
    // The dual-orchestrator docs must not weaken the project's core rule.
    let agents = read("AGENTS.md");
    assert!(agents.contains("embedded Artichoke Ruby runtime"));
    assert!(agents.contains("regent bootstrap"));

    let claude = read("CLAUDE.md");
    for needle in [
        "No shelling out to host Ruby tooling",
        "RubyEnvironment",
        "regent bootstrap",
        "missing_dependency_hint",
    ] {
        assert!(
            claude.contains(needle),
            "CLAUDE.md lost the embedded-Ruby rule mentioning {needle:?}"
        );
    }

    // Generated modules get the same invariant, phrased for a Puppet module.
    for rel in TEMPLATE_DOCS {
        let body = read(rel);
        assert!(
            body.contains("regent validate") && body.contains("regent test"),
            "{rel} must point agents at the Regent workflow"
        );
    }
}

#[test]
fn templates_carry_a_module_name_placeholder() {
    for rel in TEMPLATE_DOCS {
        let body = read(rel);
        assert!(
            body.contains("{{MODULE_NAME}}"),
            "{rel} must use the {{{{MODULE_NAME}}}} placeholder so `regent new` can render it"
        );
    }
}

#[test]
fn documents_cross_reference_each_other() {
    let agents = read("AGENTS.md");
    for link in [
        "CLAUDE.md",
        "skills/claude/SKILLS.md",
        "skills/codex/SKILLS.md",
    ] {
        assert!(agents.contains(link), "AGENTS.md must link to {link}");
    }

    // The subordinate documents all defer to AGENTS.md.
    for rel in [
        "CLAUDE.md",
        "skills/claude/SKILLS.md",
        "skills/codex/SKILLS.md",
        "templates/agents/CLAUDE.md.template",
        "templates/agents/skills_claude_SKILLS.md.template",
        "templates/agents/skills_codex_SKILLS.md.template",
    ] {
        let body = read(rel);
        assert!(
            body.contains("AGENTS.md wins"),
            "{rel} must defer to AGENTS.md on conflicts"
        );
    }
}
