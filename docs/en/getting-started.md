# Getting Started

## Prerequisites

- A terminal (macOS or Linux)
- An MCP-compatible LLM agent: Claude Code, Cursor, Windsurf, or any MCP client
- No Rust toolchain needed. No API keys. No accounts.


> **You do NOT need to configure any server or API.** After `taodb init`, your agent discovers taoDB via `.mcp.json` and communicates over local stdio — zero network, zero tokens.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash
```

This downloads the prebuilt binary for your platform and installs it to `/usr/local/bin/taodb`.

Verify:

```bash
taodb --version
```

## Initialize Your Project

```bash
cd your-project
taodb init
```

What this creates:

```
your-project/
├── .mcp.json                  # agent auto-discovers taoDB
├── .taodb/instructions.md     # agent behavior guide (editable, git-tracked)
└── taodb-memory/              # where memories live (gitignored)
```

`.mcp.json` tells your agent "hey, there's a taoDB MCP server here." `.taodb/instructions.md` tells your agent *how* to use it — what to recall before working, what to memorize after.

You can customize `.taodb/instructions.md` for your project. It's committed to git, so your whole team shares the same memory patterns.

## First Session

Restart your agent (Claude Code, Cursor, Windsurf, etc.). The agent detects taoDB via `.mcp.json` and follows the instructions flow:

**Step 1 — Agent checks memory state**

```
Agent calls: taodb_stats
Result: memory_count = 0
```

**Step 2 — Agent prompts you to import**

```
Agent says: "taodb memory is empty. Import project content?"
You say: yes
```

**Step 3 — Agent reads your files and memorizes key facts**

For each important file, the agent calls `taodb_memorize()`:

```json
// Architecture decisions (permanent)
taodb_memorize({
  "text": "Project uses Clean Architecture. Services never access DB directly.",
  "containers": ["architecture", "design-decision"],
  "energy_floor": 0.7
})

// Module descriptions (semi-permanent)
taodb_memorize({
  "text": "Auth module handles JWT tokens and OAuth2 flows. Located in src/auth/.",
  "containers": ["module:auth", "tech-stack"],
  "energy_floor": 0.5
})

// Existing code files (normal decay, time-indexed)
taodb_memorize({
  "text": "Chapter 1: Protagonist enters the city, notices guild emblem on gate.",
  "containers": ["chapter", "chapter:1", "scene:city-gate"],
  "energy_floor": 0.0,
  "time_ns": 1700000000000000000
})
```

**Step 4 — Every session after, context loads automatically**

```
Session start:
  taodb_stats     → "224 memories, container_distribution: module:auth(12), chapter(150)..."
  taodb_recent(1) → "Last memory: Fixed token rotation race condition in auth.rs"
  taodb_recall(within_days=5)  → recent context
  taodb_recall(min_energy=0.3) → permanent knowledge
```

## Customize Agent Behavior

Edit `.taodb/instructions.md`. The template covers session startup, before-work recall, and after-work memorization. Adapt these sections for your workflow.

Example for a coding project:

```markdown
## Session Startup
1. taodb_stats
2. If count=0: prompt to import ARCHITECTURE.md, README files, key docs
3. If count>0: taodb_recent(1) → taodb_recall(modules=['current feature'], within_days=14)

## Before Coding
taodb_recall(modules=['affected module'], within_days=30)
taodb_recall(modules=['architecture'], min_energy=0.5)

## After Coding
taodb_memorize(
  text="key decision or change",
  modules=["affected module", "sprint-N"],
  energy_floor=0.3 if important else 0.0
)
```

## Troubleshooting

**Agent doesn't use taoDB tools**
- Check `.mcp.json` is valid JSON
- Check `taodb` is in PATH (`which taodb`)
- Restart the agent

**"taodb memory is empty" every session**
- Check `taodb-memory/` exists and has files
- Agent may not have permission to read the directory

**macOS Gatekeeper blocks the binary**
```bash
xattr -d com.apple.quarantine /usr/local/bin/taodb
```

---

**Next:** [Concepts](concepts.md) — understand the memory model.
