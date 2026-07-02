# Concepts

TaoDB's memory model is based on cognitive science — not on vector similarity. Understanding these concepts will help you design effective memory workflows for your agent.

## The Core Insight

LLMs are good at semantic understanding. They are bad at persistent storage and temporal ordering. TaoDB handles the latter. The LLM handles the former.

This division of labor is the single most important design decision in TaoDB:

```
LLM does:  "Is this memory relevant right now?"
           "What does this memory mean for the current task?"

TaoDB does: "What happened in chapter 143-148?"
            "Which memories are about the auth module in the last 30 days?"
            "Keep world rules permanently, let event details fade."
```

## Four-Layer Memory Model

Every memory has a `potential_energy` value (0.0 to 1.0) and an `energy_floor` (minimum energy it can decay to).

| Layer | Energy Range | Behavior |
|-------|-------------|----------|
| **Working** | Full access within time window | `recall(within_days=30)` returns these |
| **Long-term** | Decaying, retrievable above threshold | `recall(min_energy=0.3)` returns high-energy ones |
| **Permanent** | `energy_floor ≥ 0.5` — never decays below floor | Always hit by recall; set for rules, architecture, world-building |
| **Forgotten** | Below threshold | Not actively recalled; trace remains, can be reawakened |

## Energy Floor

When you store a memory, you set its `energy_floor`:

| Floor | Use for |
|-------|---------|
| `0.0` | Ordinary content — chapters, code changes, daily notes. Decays naturally. |
| `0.3` | Important events — turning points, bug fixes, key decisions. |
| `0.5` | Semi-permanent — character profiles, module architecture, team conventions. |
| `0.7` | Permanent — world-building rules, design principles, regulatory requirements. |

A memory with `energy_floor = 0.7` will never drop below 0.7 no matter how old it is. It's always findable via `recall(min_energy=0.5)`.

## Temporal Indexing (Time)

TaoDB uses **narrative time**, not wall-clock time. This matters.

**Narrative time** is the time within your project's timeline. For a novel, it's the chapter number or in-story date. For a codebase, it's the sprint number or release version. The absolute value doesn't matter — only the relative ordering.

```bash
# Chapter 143: time_ns = BASE + (143 - 1) × 86400 × 10^9
# Chapter 144: time_ns = BASE + (144 - 1) × 86400 × 10^9
# Relative order is what matters for recall
```

TaoDB auto-derives `time_ns` from container tags matching time patterns (`第143回`, `sprint-5`, `day-30`). You rarely need to set it manually.

When recalling, taoDB derives the anchor time from your latest matching memory — not from the system clock. This means recall is always relative to your project's current position.

## Spatial Indexing (Space / Containers)

Containers are string tags that organize memories by domain. They're your spatial coordinate system.

Good container conventions:

```
Coding:     module:auth, module:database, feature:oauth, sprint:2025-Q1
Writing:    人物:桑安歌, 场景:邯郸酒肆, 物件:鼓, 第152回
Knowledge:   topic:rust, topic:systems-design, source:paper, source:meeting
```

The engine supports **fuzzy matching**: if you write `module:Auth`, it auto-corrects to `module:auth` if that container already exists. This keeps your tag space consistent without manual cleanup.

## Constraint vs Narrative Layers

Inspired by the Shadow-Loom paper on narrative AI, TaoDB separates memories into two layers:

**Constraint layer** (`energy_floor ≥ 0.5`, `memory_type = "constraint"`):
- World rules, character perception frameworks, architecture decisions
- Never decays
- Always injected into agent context at session start via `taodb_recall_constraints`
- Example: "The auth module uses JWT with 15-minute rotation. Never store tokens in localStorage."

**Narrative layer** (`energy_floor < 0.5`, `memory_type = "narrative"`):
- Chapter events, code changes, meeting notes
- Decays over narrative time
- Retrieved via `taodb_recall()` or `taodb_recall_narrative()`

This separation ensures permanent knowledge stays permanently accessible while transient events naturally fade.

## Sensory Indexing

Inspired by Proust's concept of involuntary memory — a sensory trigger can activate memories across any temporal or spatial boundary.

```bash
# Store a memory with sensory anchors
taodb_memorize({
  "text": "The edge of the drum gave a rough, dry sensation under her fingers.",
  "senses": ["rough", "dry"],
  "containers": ["character:桑安歌", "object:drum", "chapter:152"]
})

# Later — when writing a scene with similar sensory texture
taodb_recall_sensory(["rough", "dry"])
# Returns ALL memories sharing those sensations — regardless of character, chapter, or scene
```

Sensory indexing is cross-container, cross-character, cross-time. A "rough" sensation in chapter 20 can activate a "rough" memory from chapter 152 — enabling the kind of unexpected connection that makes creative work feel alive.

## Reconsolidation Boost

Every time a memory is recalled, its energy increases by 0.05 (capped at 1.0). This models memory reconsolidation — using a memory strengthens it.

Practical effect: frequently recalled memories stay accessible. Forgotten memories fade. The system naturally surfaces what matters.

## Decay

Decay uses a 30-day (narrative) half-life formula:

```
energy = intensity × association / (1 + dt / half_life)
```

Where `dt` is the narrative time distance between the anchor and the memory. A memory from 30 narrative days ago has half the energy of a current memory. At 60 days, it's one-third. At the 365-day mark, barely above zero — unless protected by its `energy_floor`.

Trigger decay periodically:

```bash
taodb_decay  # via MCP
```

Memories protected by `energy_floor` are unaffected.

---

**Next:** [Usage Guide](usage.md) — practical memory workflows.
