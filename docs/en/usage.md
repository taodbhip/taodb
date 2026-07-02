# Usage Guide

Practical workflows for using taoDB with your LLM agent.

## The Core Loop

```
RECALL → WORK → MEMORIZE → (repeat)
```

Your agent reads context from taoDB, does work, then stores new memories. This repeats every session, every task.

## Session Startup (Every Session)

The agent should always start with this sequence:

```
1. taodb_stats — check memory count and container distribution
2. If count = 0: prompt user to import project content
   If count > 0: proceed
3. taodb_recent(n=1) — find last position
4. taodb_recall_constraints() — load permanent rules into context
5. taodb_recall(within_days=5, top_k=10) — recent context
6. taodb_recall(min_energy=0.3, top_k=5) — permanent knowledge
```

Don't skip `taodb_stats`. It shows `container_distribution` — your agent uses this to align with existing naming conventions before writing new memories.

## Before Working

Two separate recall calls — not one combined call:

```bash
# Recent context: what happened recently in this area?
taodb_recall(containers=["module:auth"], narrative_span_days=30, dimensions=["天","地"])

# Permanent knowledge: rules, architecture, principles
taodb_recall(query="authentication architecture", min_energy=0.5, dimensions=["道"])
```

Why separate calls? Because time-window recall and energy-threshold recall have different scoring dynamics. Combining them dilutes both signals.

## After Working

Store 3-5 memories per work session. Not more. Be selective.

What to store:
- State changes — "User model now has a `last_login_ip` field."
- Decisions made — "Chose PostgreSQL over MongoDB because JOIN performance is critical for this query pattern."
- Bugs fixed — "Fixed token rotation race: was reading token from stale cache. Now reads from DB atomically."
- Foreshadowing planted (writing) — "Mentioned the guild emblem is actually a map fragment."
- Rules revealed — "The auth middleware runs before rate limiting. Order matters for DDoS protection."

What NOT to store:
- Transient details — "Changed variable name from `x` to `user_count`."
- Duplicate facts — Already stored? Don't store again.
- Every action — Not a log. Not a journal. Curated memories.

## Container Naming Conventions

Consistent containers are the key to good recall. Establish conventions early and stick to them.

**Coding projects:**
```
module:auth, module:database, module:api, module:frontend
feature:oauth, feature:rate-limiting, feature:search
sprint:2025-Q1, sprint:2025-Q2
bugfix, design-decision, tech-debt
```

**Novel writing:**
```
人物:桑安歌, 人物:柏正则, 人物:葵儿
场景:邯郸酒肆, 场景:骊山, 场景:老槐院
物件:鼓, 物件:凿子, 物件:剑
第1回, 第2回 ... 第152回
world_rule, character_profile, plot_point
```

**Knowledge management:**
```
topic:rust, topic:systems-design, topic:machine-learning
source:paper, source:meeting, source:article
project:taodb, project:hermes-agent
status:draft, status:published, status:archived
```

The engine fuzzy-matches containers, so minor typos (`module:Auth` → `module:auth`) self-correct. Use `taodb_stats` to see existing container names.

## Energy Floor Guide

Memory importance → energy floor:

| Floor | When to use | Example |
|-------|------------|---------|
| `0.0` | Default. Normal content. | "Changed variable name in auth.rs" |
| `0.3` | Worth remembering. | "Fixed race condition in token refresh" |
| `0.5` | Important reference. | "Auth module architecture: JWT + OAuth2" |
| `0.7` | Permanent. Never forget. | "Never store secrets in environment variables — use vault" |

Rule of thumb: if you'd put it in your project's README or ARCHITECTURE.md, use `0.7`. If it's a decision that will influence future code, use `0.5`. If it's an event worth recalling next week, use `0.3`.

## Setting Narrative Time

For sequential content, `time_ns` is auto-derived from containers matching time patterns. TaoDB recognizes:

- `第N回` → time_ns based on chapter number
- `sprint-N` → future: time_ns based on sprint number

Manual `time_ns` only needed when containers don't include time patterns:

```bash
taodb_memorize({
  "text": "Key event",
  "time_ns": 1701302400000000000,  # explicit narrative timestamp
  "containers": ["chapter:5"]
})
```

The absolute value doesn't matter. Only relative ordering between memories.

## Periodic Decay

Trigger after major milestones:

```bash
taodb_decay
```

```bash

Memories with `energy_floor ≥ current_energy` are protected. Others decay toward their floor.

Frequency: after every sprint, after every volume (writing), or monthly. Not every session — decay is a batch operation.

## Recall Dimensions

The `dimensions` parameter weights the scoring system:

| Dimension | Meaning | Weight effect |
|-----------|---------|---------------|
| `天` (time) | Temporal proximity | Boosts time-distance score |
| `地` (space) | Container overlap | Boosts spatial-match score |
| `道` (energy) | Energy / permanence | Boosts energy score |
| `人` (body/emotion) | Sensory richness | Boosts for memories with body_state or emotional_mark |
| `物` (objects) | Object chain matching | Boosts for memories sharing object references |

Default dimensions when none specified:
- With containers: `["天","地","道"]`
- Without containers: `["天","道"]`

Customize to match your recall intent:

```bash
# "What happened recently in auth?" → time + space
taodb_recall(containers=["module:auth"], dimensions=["天","地"])

# "What are the core rules?" → energy + permanence
taodb_recall(query="architecture", min_energy=0.5, dimensions=["道"])

# "What character moments have emotional weight?" → sensory + body
taodb_recall(containers=["character:桑安歌"], dimensions=["人"])
```

## Using Sensory Recall

Sensory recall is intentionally separate from narrative recall. Use it when creative resonance matters more than temporal proximity:

```bash
# Writing a scene about a rough, dry texture
taodb_recall_sensory(["rough", "dry"])
# Returns all memories with these sensations, regardless of where/when they occurred
```

Sensory anchors are strings: `"rough"`, `"dry"`, `"cold"`, `"bright"`, `"sharp"`, `"soft"`, `"loud"`, `"silent"`, `"bitter"`, `"sweet"`, etc. Use whatever sensory vocabulary your domain needs.

---

**Next:** [API Reference](api-reference.md) — complete tool and endpoint reference.
