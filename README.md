# taodb — Memory for AI Creators

[![CI](https://github.com/taodbhip/taodb/actions/workflows/ci.yml/badge.svg)](https://github.com/taodbhip/taodb/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/taodb)](https://crates.io/crates/taodb)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)


**Your agent forgets everything between sessions.**

You finish Friday's coding session — auth flow finally works. Monday morning you say "continue the auth work." Your agent re-reads the codebase from scratch... and suggests the mutex. The one you already tried. The one that deadlocked. The one you fixed with atomic swap.

taodb gives your agent a memory: what happened, when, and which part of your project it belongs to. Every session picks up exactly where the last one left off.

## What You Build, What Your Agent Remembers

| You build... | Your agent picks up... |
|--------------|------------------------|
| **Code** (vibe coding) | Architecture decisions, bug patterns, module history — your codebase has continuity |
| **Writing & docs** | Research threads, draft evolution, chapter outlines — long-form work stays coherent |
| **Design** (agentic design) | Design system rules, component iterations, client feedback — every revision has context |
| **Video & ads** | Project briefs, edit decisions, platform specs, client revisions — pipeline across shoots |

One tool. Any creative workflow. Any LLM agent. Just bring something to make.

---

## Before & After

**Without taodb — every session, you re-teach:**
```
┌ Session 2 ─────────────────────────────────────────────┐
│ You:     "Continue the auth module refactor."          │
│ Agent:   *reads codebase from scratch*                 │
│ Agent:   "There's a race condition. I'll add a mutex." │
│ You:     "We fixed that last week. The mutex dead-     │
│           locked. We switched to atomic swap."         │
│ Agent:   "I see. Let me re-read everything..."         │
└────────────────────────────────────────────────────────┘
```

**With taodb — your agent walks in knowing your project:**
```
┌ Session 2 (agent auto, before you type anything) ─────┐
│ taodb_recent(1) → "Fixed token rotation with atomic   │
│                    swap. Avoid mutex in auth."         │
│ taodb_recall_constraints → JWT architecture, AppError  │
│                             convention, bcrypt decision│
└────────────────────────────────────────────────────────┘
┌ You: "Add refresh token rotation." ────────────────────┐
│ Agent: "Got context from last session. Implementing    │
│         refresh tokens consistent with existing JWT    │
│         pattern and AppError conventions."             │
└────────────────────────────────────────────────────────┘
```

The same dynamic works for writing, design, and video work — your agent remembers project rules, client briefs, chapter outlines, component decisions, edit notes, all without you repeating yourself.

---

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash
cd your-project && taodb init
# Restart your agent. Done.
```

No API keys. No accounts. No servers to run. Your agent discovers taodb through `.mcp.json` and communicates locally over stdio — zero network, zero tokens. Rust users can also `cargo install taodb`.

First session: agent detects empty taodb → asks "Import your project?" → reads your files and memorizes key facts. Every session after: auto-recalls context, auto-memorizes new work.

Works with Claude Code, Cursor, Windsurf, Hermes, OpenCode — any MCP agent.

[📖 Full Documentation](docs/) — English & 中文

---

## Demo

![taodb — 30-second demo](docs/marketing/demo.gif)

30 seconds, two sessions apart. The agent stores the decision in Session 1, asks about it in Session 2, and taodb surfaces the prior decision with `recall_paths` showing which dimensions matched — `天` (time range) and `地` (container overlap).

Same query against a vector DB returns online tutorials and RFC text. taodb returns the actual decision:

![vector DB vs taodb](docs/marketing/side-by-side.png)

[📖 Full demo storyboard & recording notes](docs/marketing/demo-script.md) · [Show HN kit](docs/marketing/show-hn.md)

---

## How It Works

```
SESSION START (agent auto, every time):
  taodb_stats                     → "224 memories loaded"
  taodb_recent(n=1)               → "Last: final-cut revision 3 notes"
  taodb_recall_constraints        → project rules, design system, client brief

BEFORE WORK:
  taodb_recall(containers=["..."], narrative_span_days=14) → recent history
  taodb_recall(query="...", min_energy=0.5)                → permanent knowledge

AFTER WORK:
  taodb_memorize(text="key outcome", containers=[...], energy_floor=0.3)

PERIODIC:
  taodb_decay → transient details fade, permanent knowledge stays
```

The agent drives the loop. taodb stores and retrieves. The agent decides what matters.

---

## Memory Architecture

Three layers, one principle: **important things stay, transient things fade.**

| Layer | Stores | Behavior |
|-------|--------|----------|
| **Constraint** | Rules, architecture, design system, project briefs, world-building | Never decays. Always in context at session start. |
| **Narrative** | Events, changes, decisions, iterations, drafts | Decays over time. Recent = higher priority. |
| **Sensory** | Textures, patterns, moods — cross-domain triggers | Connects memories across containers, timelines, and projects. |

**Energy floor** controls what stays permanent:

| Floor | Meaning | Example |
|-------|---------|---------|
| `0.7` | Permanent | Design system rules, client brief, world-building docs |
| `0.5` | Semi-permanent | Component library, chapter outlines, brand guidelines |
| `0.3` | Important | Key decisions, bug fixes, plot turning points |
| `0.0` | Transient | Daily work, draft iterations, routine changes |

**Containers** organize memories by how you think about your project:

```
Coding:    feature:auth, module:database, sprint:Q1
Writing:   chapter:3, character:protagonist, scene:opening
Design:    component:navbar, client:acme, version:v2
Video:     project:campaign-q3, platform:tiktok, revision:final-cut
```

---

## MCP Tools

| Tool | Purpose |
|------|---------|
| `taodb_stats` | Session start — check memory state, see container schema |
| `taodb_recent` | Find last position by insertion order |
| `taodb_recall` | Multi-dimensional context recall by time, space, energy, and query |
| `taodb_memorize` | Store memory with containers, energy floor, sensory anchors |
| `taodb_recall_constraints` | Recall permanent rules/decisions — always available |
| `taodb_recall_sensory` | Cross-domain recall by sensory impression (texture, mood, pattern) |
| `taodb_forget` | Remove incorrect or duplicate memories |
| `taodb_decay` | Trigger energy decay — permanent memories protected |

---

## Workflows

### Coding
```
taodb_stats → taodb_recent(1) → taodb_recall_constraints
taodb_recall(containers=["feature:auth"], narrative_span_days=14)
taodb_memorize(text="Replaced mutex with atomic swap in token rotation",
  containers=["feature:auth", "bug:race-condition"], energy_floor=0.3)
```

### Writing & Docs
```
taodb_stats → taodb_recent(1) → taodb_recall(within_days=5)
taodb_recall(containers=["chapter:12", "character:protagonist"])
taodb_memorize(text="Ch12: protagonist discovers the guild emblem is a forgery",
  containers=["chapter:12", "character:protagonist", "plot:reveal"], energy_floor=0.0)
```

### Design
```
taodb_stats → taodb_recall_constraints(min_floor=0.5)
taodb_recall(containers=["component:navbar", "client:acme"], narrative_span_days=30)
taodb_memorize(text="Navbar v3: reduced to 48px, removed dropdown, per client feedback",
  containers=["component:navbar", "client:acme", "version:v3"], energy_floor=0.5)
```

### Video & Advertising
```
taodb_stats → taodb_recent(1)
taodb_recall(containers=["project:spring-campaign", "platform:tiktok"])
taodb_memorize(text="Final cut: swapped shot 3→7, adjusted pacing per client note",
  containers=["project:spring-campaign", "version:final-cut", "client:acme"], energy_floor=0.3)
```

See [USAGE.md](USAGE.md) for detailed workflow templates and `.taodb/instructions.md` customization.

---

## Tech Stack

**Rust** — memory-safe, single binary · **redb** — embedded B-tree storage · **MCP stdio** — agent-native, zero-latency · **axum** — HTTP for non-MCP integrations · Zero external APIs, zero embedding dependencies, zero cloud services.

---

## 中文摘要

taodb 是一个 Rust 编写的 LLM agent 记忆引擎。一个二进制文件，零配置。适用于 coding、写作、设计、视频广告等任何需要 agent 跨会话记忆的场景。

```bash
curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash
cd your-project && taodb init
# 重启 agent。完毕。
```

三层记忆架构：约束层（规则/架构/设计系统 — 永不衰减）+ 叙事层（事件/决策/迭代 — 随时间衰减）+ 感官层（跨域联想 — 纹理/模式/情绪）。能量地板机制保证重要信息不丢失。

适用于 Claude Code、Cursor、Windsurf、Hermes、OpenCode 等任何 MCP 兼容 agent。

[📖 完整文档](docs/) — 中英双语

---

[MIT License](LICENSE) | [Changelog](CHANGELOG.md) | [Contributing](CONTRIBUTING.md)
