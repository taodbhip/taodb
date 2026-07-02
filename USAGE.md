# taodb Usage Guide

How to make taodb work effectively with any MCP-compatible LLM agent.

## The Core Loop

taodb is a memory engine, not a search engine. It doesn't understand your content — your agent does. The value comes from a simple loop:

```
RECALL → WORK → MEMORIZE → (repeat)
```

Your agent reads context from taodb, does creative work, then stores new memories back. taodb handles the "when" and "where" of memories; the agent handles the "what" and "why."

## Setting Up Your Project

### 1. Run `taodb init`

```bash
cd my-project
taodb init --user myname --project myproject
```

This creates:
- `.mcp.json` — connects taodb to your agent
- `.taodb/instructions.md` — project-specific guidance (universal, works with all MCP agents)
- `taodb-memory/` — where memories are stored (added to .gitignore)

### 2. Customize `.taodb/instructions.md`

This is the central file. taodb MCP server reads it on startup and injects it into the MCP `instructions` field. **Every MCP agent sees it automatically** — Claude Code, Cursor, Windsurf, Hermes, OpenCode, or any other MCP client. No agent-specific config files needed.

The template created by `init` covers:
- Session startup (stats → recent → recall)
- Before-work flow (what to recall)
- After-work flow (what to memorize)
- What types of content to store

Adapt these sections for your project. The file is committed to git — your whole team gets the same instructions.

### 3. Restart your agent

The agent receives two layers of guidance through the MCP `instructions` field:

1. **Base layer** (built into taodb binary) — generic session startup, tool usage, energy floor guide
2. **Project layer** (from `.taodb/instructions.md`) — your project's specific recall/memorize patterns, rules, conventions

Both layers together tell the agent exactly how to use taodb for your project.

## First Session: Importing Content

When the agent first connects and sees `memory_count=0`, it should prompt:

> "taodb 记忆为空。要导入项目内容吗？"

If you say yes, the agent reads your project files and calls `taodb_memorize` for each key fact. Import priority:

1. **World-building docs / project specs** — `energy_floor=0.7` (permanent)
2. **Chapter plans / architecture docs** — `energy_floor=0.5` (semi-permanent)
3. **Existing chapters / code files** — `energy_floor=0.0` with `time_ns` (time-indexed)

For large projects (100+ files), use the batch import script:

```bash
# Use the MCP tools directly: taodb_memorize for each key file
```

## Everyday Usage

### Before Writing a Chapter

The agent recalls context in two modes:

**Recent context** (time window):
```
taodb_recall(query="<POV character>", within_days=5, top_k=10)
```
Returns chapters near the current writing point. The `query` string is metadata — filtering happens by time window, not text match. The agent reads all returned memories and decides relevance.

**World lore** (energy threshold):
```
taodb_recall(query="<topic>", min_energy=0.3, top_k=5)
```
Returns permanent memories regardless of time. Use for world-building facts, character profiles, rules.

### After Writing a Chapter

Store 3-5 key memories per chapter. What to store:

- New character appearances
- Object state changes
- Foreshadowing planted/resolved
- Emotional arc turns
- Rule reveals

Format:
```
taodb_memorize(
  text="第N回关键事件：<POV人物>在<地点><做了什么>，
       导致<不可逆变化>。",
  containers=["chapter", "人物:<主角>", "场景:<地点>", "第N回"],
  time_ns=<narrative_timestamp>,
  energy_floor=0.0
)
```

### Setting Narrative Time

For sequential content (chapters, log entries), set `time_ns` so taodb can order memories correctly:

```
time_ns(第N回) = BASE + (N-1) × 86400 × 10^9
```

Where `BASE` is any fixed nanosecond timestamp (e.g., `1700000000000000000`). The absolute value doesn't matter — only the relative ordering.

### Periodic Decay

After major milestones, trigger decay:
```
taodb_decay
```
Memories with `energy_floor` above their current energy are protected. Others decay naturally.

## Project Type Examples

### Novel Writing

```markdown
## TaoDB 会话启动
1. taodb_stats
2. 如果 count=0：提示导入 {世界设定、人物素材库、卷规划、已写章回}
3. 如果 count>0：taodb_recent(1) → taodb_recall(within_days=5)

## 写前
taodb_recall(query="<本回POV>", within_days=5, top_k=10)
taodb_recall(query="<涉及的世界设定>", min_energy=0.3, top_k=5)

## 写后（每回落盘）
taodb_memorize(text="<关键事件摘要>",
  containers=["chapter","人物:<POV>","场景:<地点>"],
  energy_floor=0.0)
# 如果揭示了新的世界规则：
taodb_memorize(text="<新规则>",
  containers=["world_doc","设定"],
  energy_floor=0.7)
```

### Vibe Coding

```markdown
## TaoDB 会话启动
1. taodb_stats
2. 如果 count=0：提示导入 {ARCHITECTURE.md, 各模块 README, 关键代码注释}
3. 如果 count>0：taodb_recent(1) → taodb_recall(within_days=30)

## 写代码前
taodb_recall(query="<相关模块>", within_days=30, top_k=5)
taodb_recall(query="<架构决策>", min_energy=0.5, top_k=5)

## 写代码后（重要决策）
taodb_memorize(text="<决策：为什么这样做>",
  containers=["design_decision","<模块>"],
  energy_floor=0.5)
```

### Knowledge Base

```markdown
## 每次会话
taodb_stats
# 如果为空，导入已有笔记/文档
taodb_recall(query="<当前话题>", within_days=90, top_k=10)
taodb_recall(query="<核心概念>", min_energy=0.5, top_k=5)

## 每次学到新东西
taodb_memorize(text="<新知识点>",
  containers=["knowledge","<领域>","<话题>"],
  energy_floor=0.3)
```

## Troubleshooting

**Agent doesn't use taodb tools:**
- Check `.mcp.json` is valid JSON
- Check `taodb` is in PATH (`which taodb`)
- Restart the agent

**Recall returns irrelevant memories:**
- Narrow `within_days` (default 30 is wide)
- Add more specific `containers` to memories
- Set proper `time_ns` values so temporal ordering works

**Memories disappear too fast:**
- Raise `energy_floor` for important content
- Recall more frequently (each recall boosts energy)

**Too many memories, recall is noisy:**
- Run `taodb_decay` periodically
- Be more selective about what you memorize
- Use `containers` to create spatial groupings

---

[README](README.md) | [Contributing](CONTRIBUTING.md)
