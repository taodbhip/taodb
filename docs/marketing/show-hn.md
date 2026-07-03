# Show HN Submission Kit

> Everything you need to submit taodb to Hacker News.
> Read this once, then submit. Don't over-edit the pitch after publishing.

---

## Title (pick one)

| # | Title | Why |
|---|---|---|
| 1 | `Show HN: taodb – Long-term memory for LLM agents without embeddings or vector DB` | **Recommended.** Direct, factual, hooks the "without embeddings" counter-intuitive. |
| 2 | `Show HN: taodb – Persistent memory for LLM agents, indexed by time and space` | Falls back if HN mods reject #1 for length. |
| 3 | `Show HN: I built a memory engine for LLM agents that doesn't do embeddings – here's why` | Storytelling format. Less searchable. |
| 4 | `Show HN: taodb – An open-source hippocampus for LLM agents` | "Hippocampus" is too cute for HN. Avoid. |
| 5 | `Show HN: taodb – A single-binary memory engine for AI agents` | Too generic. Doesn't explain why. |

**Pick #1 unless the HN title is already taken.**

---

## Pitch (paste verbatim into the submission text box)

```
Hey HN,

I built taodb (https://github.com/taodbhip/taodb) — a single-binary memory
engine for LLM agents that doesn't use vector embeddings. The reason: when an
agent asks "what did we decide about auth last Tuesday?", that's a temporal-
spatial query, not a similarity query. We benchmarked recall@5 against
pgvector on a real 8-month project and got 0/5 (vector) vs 5/5 (taodb). The
5 vector hits were online tutorials, RFC text, and Stack Overflow — the actual
decision was buried at rank 11.

taodb stores raw time-stamped events with energy decay (30-day narrative half-
life, `energy_floor` for permanent rules) and a container index (the project's
actual modules / threads / chapters). Recall returns a time-bounded,
container-filtered slice of raw memories; the LLM does the semantic work.
~1,200 lines of Rust, single 1.7 MB binary, zero external dependencies,
MCP stdio transport.

Install:  curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash
Why not vector DB:  https://github.com/taodbhip/taodb/blob/main/docs/why-not-vector-db.md

I would love feedback on the recall design and the energy_floor mechanics —
both are detailed in the repo. The design draws on CoALA, EM-LLM, and
Shadow-Loom.
```

**Do not edit** the pitch between this file and the HN submission. The wording has been balanced for HN's format and tested against common Show HN critiques (over-claim, vague, missing technical depth).

---

## FAQ (reply-paste answers, in priority order)

The first hour after submission, the questions will cluster. Pre-write answers
and paste them, not paraphrased — paste the canonical version.

### Q1: How is this different from Mem0 / Letta / Zep?

> Those are session-scope "short-term" memory layers that extract and summarize
> messages, then surface them back to the LLM. taodb is project-scope "long-term"
> memory that stores raw time-stamped events with energy decay. Different scale,
> different purpose. If your agent loses context after 50 messages, use Mem0.
> If your agent loses context after 50 sessions, use taodb.
>
> There's a longer breakdown in the README ("Memory Architecture" section) and
> the position paper: [docs/why-not-vector-db.md](https://github.com/taodbhip/taodb/blob/main/docs/why-not-vector-db.md).

### Q2: Why no embeddings? Doesn't semantic search beat keyword search?

> Embeddings are great for "find content similar to this passage" — that's RAG.
> But "what did we decide about X last week" isn't a similarity query, it's a
> temporal-spatial query. Semantic similarity actively misleads the agent:
> similar code patterns from other modules look identical to the actual decision
> you made. We tested recall@5 on a real 8-month project — vector DB returns
> 0/5, taodb returns 5/5. Full benchmark methodology is here:
> [docs/why-not-vector-db.md](https://github.com/taodbhip/taodb/blob/main/docs/why-not-vector-db.md).

### Q3: How does recall work without an LLM in the loop?

> taodb returns a time-bounded, container-filtered slice of raw memories. The
> agent reads them. The agent decides what's relevant. This is by design —
> putting an LLM in the recall path adds latency, cost, and a second source
> of truth. taodb's storage is a pure key-value index over time + container +
> energy; the LLM does the semantic work it was trained to do.

### Q4: What about privacy / data ownership?

> Memories live in a redb file inside your project (`./taodb-memory/`). Same
> gitignore handling as `.env`. They never leave your machine. MCP transport
> is stdio — no network, no telemetry. The README and source are auditable in
> 1,200 lines of Rust, no vendored dependencies beyond what's in Cargo.toml
> (redb, axum, rmcp, bincode, crc32, ulid — all auditable).
>
> install.sh makes exactly one outbound HTTP call: to api.github.com to fetch
> the latest version, and one HTTPS download from github releases. No
> analytics, no install beacons, no version pings after install.

### Q5: What's the install footprint?

> One 1.7 MB binary. Zero runtime dependencies. Optional ghcr.io Docker
> image (`docker pull ghcr.io/taodbhip/taodb:v1.0.0`). No daemon, no database
> server, no Python virtualenv. Verified clean on macOS (arm64 + x86_64) and
> Linux x86_64. Release artifacts: https://github.com/taodbhip/taodb/releases/tag/v1.0.0

---

## Submission Checklist

### Timing
- **Best window:** Tuesday / Wednesday / Thursday, **14:00–16:00 UTC** (US East morning, EU afternoon).
- **Avoid:** Mondays (low visibility), Fridays (mods thin), weekends (off-peak).
- **Don't schedule for 0 stars.** Submit when you have 2-3 hours free to reply in-thread — the first hour determines front-page status.

### Mechanics
- [ ] Title uses **#1** verbatim.
- [ ] Pitch pasted **verbatim** — no edit, no "thanks for checking this out" closer.
- [ ] URL field is the **repo root** (`https://github.com/taodbhip/taodb`), not the release page, not the docs.
- [ ] You are logged in to an account with > 50 karma, or one that has done 1+ successful Show HN before. New accounts with no karma get deprioritized.
- [ ] The repo has a **demo gif** in the README or linked from the pitch. (See `demo-script.md`.)

### The first hour
- [ ] Stay at the keyboard. Reply to every comment in the first 60 minutes.
- [ ] Reply to **criticism with specifics**, not "thanks, will think about it."
- [ ] If asked "why not just use Postgres," answer with the temporal-spatial query, not "we're different."
- [ ] Do not edit the pitch after submission — HN shows edit timestamps and it reads as panic.
- [ ] If the post is going down (negative rank after 30 min), do NOT delete and repost. The post is dead either way; deleting makes a re-post harder.

### What to ignore
- "Just use Postgres" / "just use a vector DB" — answer once, then stop. Don't re-argue with the third repetition.
- "This is what LangChain already does" — answer once with the scale distinction (MemGPT/Letta vs taodb).
- "Why Rust?" — answer: "single binary, zero runtime, embedded redb, MCP stdio, all native."
- Generic AI hype / slop — don't engage. HN readers will down-vote it for you.

---

## After the First Hour

| Outcome | Action |
|---|---|
| **Front page, top 30** | Pin a 1-line comment: "FAQ + benchmark details in [docs/why-not-vector-db.md](…). Will be here for questions." Then reply to substantive comments only. |
| **Top of /new, slowly climbing** | Standard. Keep replying. Don't pin anything. |
| **Stuck on /new, not climbing** | Don't edit. Wait 24h, then write a "lessons learned" reply in-thread. |
| **Down-voted off /new** | Don't delete. Write a "thanks for the feedback" reply, then **submit a follow-up Show HN 30 days later** after addressing the top 3 critiques. |

---

## Synchronized Cross-Posts (do these within 1 hour of HN submission)

The point of cross-posting within 60 minutes is **search-engine signal density** —
HN, Reddit, X, and Discord all index to the same day, which makes the project
look "alive" to anyone searching.

| Channel | Format | Title |
|---|---|---|
| **Reddit r/LocalLLaMA** | Self-post, body = pitch (verbatim) | "Taodb: An open-source memory layer for LLM agents, no embeddings needed" |
| **Reddit r/rust** | Self-post, body = pitch + a line on "why Rust was the right call" | "Taodb: A single-binary memory engine for LLM agents written in Rust" |
| **X thread** | 6-tweet thread, each tweet one idea from the pitch | Tweet 1: "I built a memory engine for LLM agents that doesn't use vector embeddings…" |
| **Discord: LangChain, Hugging Face, AI Agent Community** | Channel-specific: lead with the angle | "open-source memory layer for agents, time-indexed, no embeddings" |

**Do not** post to:
- r/MachineLearning (academic, will reject)
- r/Programming (too generic, will down-vote)
- Multiple subreddits in the same hour (looks like spam)

---

## Talking Points Cheat Sheet (pin to your second monitor)

If you get a hostile or skeptical comment, the only thing that matters is
giving a **specific, falsifiable** answer. Memorize these:

1. **"Why not vector DB?"** → Recall mismatch. Worked example in `docs/why-not-vector-db.md`.
2. **"Why not Postgres + tsvector?"** → tsvector is keyword, not temporal-spatial; no container model; no energy decay.
3. **"Why not MemGPT?"** → Session-scale, summary-based, LLM in loop. Project-scale, raw events, LLM out of loop.
4. **"How big is the project?"** → ~1,200 LOC Rust, 1.7 MB binary, 88 tests, single binary release.
5. **"What's the differentiator?"** → Time + container + energy as the index. No LLM in recall. Zero external dependencies.
6. **"Who's the user?"** → Engineers with multi-session agent workflows where continuity matters (vibe coders, long-running creative projects, technical writing).
7. **"Why open source?"** → Agent memory is too load-bearing to be a SaaS. It has to be inspectable.
