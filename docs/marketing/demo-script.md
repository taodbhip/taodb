# taodb Demo Script — 30-Second Recording

> **Purpose**: A demo gif / mp4 that fits into the README, the Show HN post, and the Reddit / X cross-posts. The viewer should understand what taodb does without reading any text.
>
> **Time budget**: 30 seconds. Output ≤ 10 MB, ≤ 1280×720.
>
> **Mode**: MCP stdio only. The demo never touches the HTTP API — taodb's primary path is local MCP transport, and the demo should show that path exactly as a real agent runs it.

---

## The 30-Second Storyboard

| Time | Screen action | Voiceover (optional) |
|---|---|---|
| 0:00 – 0:03 | Title card: **"taodb — memory for LLM agents"** | "Every new agent session, your agent re-reads the codebase from scratch." |
| 0:03 – 0:08 | Terminal: `taodb init --user demo --project auth-service` | "One command. taodb initializes inside your project, zero config." |
| 0:08 – 0:13 | `python3 demo_agent.py memorize "..." --containers ... --energy 0.3` | "Session 1: the agent stores the auth-module decision after debugging a deadlock." |
| 0:13 – 0:15 | "··· two weeks later ···" caption | "Two weeks pass." |
| 0:15 – 0:18 | `python3 demo_agent.py ask "what did we decide about auth?"` | "Session 2: new agent, same project, asks: what did we decide about auth?" |
| 0:18 – 0:24 | Output: the prior decision is the top hit, with `energy=0.35 (floor=0.30)` and `recall_paths` showing `天` (time) + `地` (container) hits | "The decision is the top hit. With the energy floor showing it's protected, and the recall paths showing which dimensions matched." |
| 0:24 – 0:28 | Side-by-side: vector DB result vs taodb result | "Same query against a vector DB: 0/5. Top hits are RFCs, tutorials, Stack Overflow." |
| 0:28 – 0:30 | `github.com/taodbhip/taodb` | "taodb. Open source. MIT. Single binary, zero config." |

**Critical frame**: 0:18–0:24. The viewer must see the prior decision surface as the **top hit** with the `recall_paths` block underneath. Don't cut that frame short.

---

## How the Demo Runs (under the hood)

The demo uses three files in this directory:

| File | Role |
|---|---|
| `demo_agent.py` | The "agent" — a tiny Python script that opens a stdio MCP session to `taodb mcp` and prints the conversation in human-friendly form. |
| `demo.sh` | A 60-second pre-flight that sets up the project state (one-time, **not recorded**). |
| `demo.tape` | The vhs script that records the 30-second footage. |

`demo_agent.py` is the only piece that talks to taodb. It does **not** use the HTTP API — it spawns `taodb mcp` as a stdio subprocess and exchanges JSON-RPC frames, exactly the way Claude Code or Cursor does.

---

## Pre-Flight Setup (60 seconds, before you record)

Run this once to put the project in the right state. Don't record any of it.

```bash
# 1. Clean demo project dir
rm -rf ~/demo/auth-service
mkdir -p ~/demo/auth-service/src
cd ~/demo/auth-service

# 2. Add a few files so the project looks real
cat > Cargo.toml <<'EOF'
[package]
name = "auth-service"
version = "0.1.0"
edition = "2021"
EOF

cat > src/lib.rs <<'EOF'
// Minimal auth stub — the real project has more
pub fn rotate_token(_t: u64) -> u64 { 0 }
EOF

# 3. Initialize taodb (creates .mcp.json, .taodb/, taodb-memory/)
taodb init --user demo --project auth-service

# 4. Verify the agent script can talk to taodb
python3 "$(dirname "$0")/demo_agent.py" recall "smoke test" --containers feature:auth --days 1
# Should print: "no memories returned"  (the db is empty, that's correct)
```

You should now be in `~/demo/auth-service`, with `taodb init` having created `.mcp.json`, `.taodb/instructions.md`, and `taodb-memory/`. The smoke test should succeed silently.

**Do not** record the pre-flight. Recording starts at the `taodb init` step below.

> **Note**: `taodb init` is included in the recording — it shows the "one command" magic. If you already ran it in pre-flight, delete `.mcp.json` and re-run.

---

## The vhs Script (preferred)

> `vhs` produces deterministic, pixel-perfect terminal recordings. Use it.
>
> Install: `brew install vhs` (macOS) or see [github.com/charmbracelet/vhs](https://github.com/charmbracelet/vhs).

Save this as `~/demo/auth-service/demo.tape`:

```vhs
# taodb canonical 30s demo
# Render:  vhs demo.tape
# Output:  demo.gif (~7 MB) + demo.mp4 (~1.5 MB)

Output demo.gif
Output demo.mp4

Set Shell bash
Set FontSize 16
Set Width 1280
Set Height 720
Set Theme "Tokyo Night"
Set Padding 20
Set TypingSpeed 60ms

# ── 0:00 — Title card ──
Type "# taodb — memory for LLM agents"
Sleep 1500ms
Enter
Sleep 1000ms

# ── 0:03 — Project init ──
Type "taodb init --user demo --project auth-service"
Sleep 500ms
Enter
Sleep 2500ms

# ── 0:08 — Session 1: agent stores the decision ──
Type "# Session 1 — agent finishes a debugging session"
Sleep 1000ms
Enter
Sleep 500ms

Type `python3 demo_agent.py memorize "Fixed token-rotation race; switched mutex to atomic swap. Mutex deadlocked under contention." --containers feature:auth,bug:race-condition --energy 0.3`
Sleep 500ms
Enter
Sleep 4000ms

# ── 0:13 — Time passes ──
Type "# ... two weeks pass ..."
Sleep 1000ms
Enter
Sleep 2000ms

# ── 0:15 — Session 2: agent asks "what did we decide?" ──
Type "# Session 2 — new agent, new session, same project"
Sleep 1000ms
Enter
Sleep 500ms

Type `python3 demo_agent.py ask "what did we decide about auth last week?"`
Sleep 500ms
Enter
Sleep 5000ms

# ── 0:24 — Side-by-side (post-prod cutaway) ──
# This frame is added in post-production. See "Adding the side-by-side" below.

# ── 0:28 — GitHub URL ──
Type "# github.com/taodbhip/taodb"
Sleep 1000ms
Enter
Sleep 2000ms
```

**Render**:
```bash
cd ~/demo/auth-service
vhs demo.tape
# Produces: demo.gif + demo.mp4
```

**Important**: `demo_agent.py` reads the user/project from `.mcp.json` automatically. As long as `taodb init` was run with `--user demo --project auth-service`, the script uses the same identity and the recall hits the just-stored memory.

---

## The `ask` Command (alias for clarity)

The script exposes `recall` as `ask` for demo readability. The mapping is one-line in `demo_agent.py`. If you want a different name, just `sed` it or pass an alias.

The actual JSON-RPC the script sends is:

```json
{
  "jsonrpc": "2.0", "id": 2, "method": "tools/call",
  "params": {
    "name": "taodb_recall",
    "arguments": {
      "query": "what did we decide about auth last week?",
      "containers": ["feature:auth"],
      "narrative_span_days": 14,
      "top_k": 3
    }
  }
}
```

This is **exactly** the JSON-RPC frame a real MCP client (Claude Code, Cursor) sends. The demo is showing the real protocol — not a mock.

---

## Adding the Side-by-Side (the most important frame)

VHS can't render a true side-by-side in a single terminal, so this is a post-production step.

```
┌────────────────────────┬──────────────────────────────┐
│  VECTOR DB (pgvector)  │       taodb (MCP stdio)      │
│  query: "auth"         │  query: containers=auth      │
│                        │                              │
│  1. Mutex patterns Rust│  1. ✓ Fixed token rot. race; │
│  2. Fix race cond.    │     switched mutex → atomic  │
│  3. OAuth2 RFC         │     swap. Mutex deadlocked.  │
│  4. Token validation   │                              │
│  5. Mutex deadlock SO  │  energy=0.35 (floor=0.30)    │
│                        │  recall_paths:               │
│  recall@5 = 0/5        │    天: 2 hits, 地: 2 hits    │
│                        │                              │
│                        │  recall@5 = 5/5              │
└────────────────────────┴──────────────────────────────┘
```

Generate it once (Figma, Keynote, ffmpeg drawtext, matplotlib — your call) and cut it in at 0:24 with a 4-second duration.

```bash
# Cut in the side-by-side at 0:24
ffmpeg -i demo.mp4 -i side-by-side.mp4 \
  -filter_complex "
    [0:v]trim=0:24,setpts=PTS-STARTPTS[v0];
    [1:v]trim=0:4,setpts=PTS-STARTPTS[v1];
    [v0][v1]concat=n=2:v=1[outv]
  " \
  -map "[outv]" -map 0:a? \
  demo-final.mp4

# Re-encode to gif
ffmpeg -i demo-final.mp4 -vf "fps=15,scale=1280:-1" -pix_fmt rgb24 demo-final.gif
gifsicle -O3 --lossy=80 demo-final.gif -o demo-final.optimized.gif
```

If you don't have ffmpeg, the fallback is to record the side-by-side as a **second vhs tape** and concat in iMovie / DaVinci Resolve.

---

## Fallback: asciinema (no ffmpeg)

```bash
brew install asciinema
cd ~/demo/auth-service
asciinema rec demo.cast \
  --title "taodb — 30s demo" \
  --cols 120 --rows 32 \
  --command "bash demo-runner.sh"
```

`demo-runner.sh`:

```bash
#!/usr/bin/env bash
set -e
cd ~/demo/auth-service

# === Session 1 ===
echo "taodb — memory for LLM agents"
sleep 1.5

taodb init --user demo --project auth-service
sleep 2.5

echo "# Session 1 — agent finishes a debugging session"
sleep 1
python3 demo_agent.py memorize \
  "Fixed token-rotation race; switched mutex to atomic swap. Mutex deadlocked under contention." \
  --containers feature:auth,bug:race-condition --energy 0.3
sleep 2

echo "# ... two weeks pass ..."
sleep 2

# === Session 2 ===
echo "# Session 2 — new agent, same project"
sleep 1
python3 demo_agent.py ask "what did we decide about auth last week?"
sleep 4

echo "# github.com/taodbhip/taodb"
sleep 2
```

**Caveat**: asciinema is text-mode. Fine for asciinema.org and a link, but for embedding a `.gif` in Reddit or X, **use vhs**.

---

## Fallback: Manual Screen Recording (zero tooling)

```bash
# macOS
# 1. Open QuickTime Player → File → New Screen Recording (⌃⌘N)
# 2. Select terminal, hit record
# 3. Run demo-runner.sh by hand
# 4. Stop, save as demo.mov
# 5. Convert to gif:
#    - Drop into ezgif.com (online, free)
#    - Or: ffmpeg -i demo.mov -vf "fps=15,scale=1024:-1" demo.gif
```

Manual is fine. Don't speak. Don't explain. Keep total runtime ≤ 30 seconds.

---

## Embedding the Demo

### In README.md

```markdown
![taodb — 30s demo](docs/marketing/demo.gif)
```

Place it right under the **Install** section. The first thing every visitor sees after the value-prop should be the demo.

### In Show HN pitch

Show HN doesn't render images. Link to it:

```
Demo: https://github.com/taodbhip/taodb/blob/main/docs/marketing/demo.gif
```

### In X thread

Upload `demo.mp4` to the first tweet. Twitter plays inline.

### In Reddit

Upload `demo.gif` directly to the post body.

---

## QA Checklist (run before publishing)

- [ ] Total runtime is **28-32 seconds**.
- [ ] The decision text appears in the recall output **verbatim** ("Fixed token-rotation race; switched mutex to atomic swap...").
- [ ] The `feature:auth` container label is visible at least once.
- [ ] The `recall_paths` block is visible (this is the **only** place we show the dimensional hit counts — keep it).
- [ ] File size is **< 10 MB** (GitHub README limit; X limit is 15 MB for mp4).
- [ ] First frame is readable as a still.
- [ ] `github.com/taodbhip/taodb` is on screen for **≥ 2 seconds** at the end.
- [ ] Terminal font is **monospace ≥ 16px**.
- [ ] No PII, no real project names, no real API tokens.

---

## Common Recording Mistakes

1. **Recording the pre-flight.** If viewers see `taodb user-create` and `taodb project-create` first, they'll think taodb is a multi-tenant SaaS. **Start at `taodb init`.**
2. **Recording a real Claude Code session.** Real agent sessions have weird pauses, retries, half-typed output, JSON errors. **Use the explicit `demo_agent.py` calls.** The demo is about taodb, not about Claude Code.
3. **Forgetting the side-by-side.** Without the vector DB comparison, the demo is "we have an API." With it, the demo is "we solve a problem you actually have."
4. **Long pauses.** VHS and asciinema render empty terminals literally. Tighten `Sleep` durations.
5. **Music.** No background music on a Show HN demo. Instant skip.
6. **Showing the HTTP API.** taodb's primary mode is MCP stdio. The HTTP server is internal-only and not the user-facing path. The demo must not show it.
7. **Showing the `--data`, `--user`, `--project` flags explicitly.** Use `taodb init` + `demo_agent.py` (which reads `.mcp.json`). The whole point of the install is that these are zero-effort.

---

## What Goes Where

| Output | Goes into |
|---|---|
| `demo.gif` | README.md (under Install), Reddit post body, X tweet (as image) |
| `demo.mp4` | X thread (first tweet), Discord channels that support video |
| `demo.cast` | GitHub Discussions, Discord channels (text link) |
| `side-by-side.mp4` | Cut into demo-final.mp4 at 0:24, 4 seconds |
| The still frame at 0:18 | "Screenshot" badge in README and docs |
