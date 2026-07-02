# TaoDB — Memory for AI Creators

## The Problem

LLM agents are amnesiacs. And this isn't just a programmer's problem — AI writers lose their narrative thread, AI designers forget their design system, AI video producers re-explain client briefs every session. Every session starts blank. Claude Code opens your project and doesn't remember what you fixed last week. Cursor Agent doesn't know which architectural decisions led to today's code structure. Windsurf can't recall the bug pattern you solved three sessions ago.

Vector databases don't fix this. They give you "semantically similar" text — but similarity is not continuity. Knowing that a chunk of code is "about authentication" is not the same as knowing that "we patched a token rotation race condition in auth.rs on sprint 3, and it introduced a regression in the login flow."

**LLMs need memory. Not semantic search. Temporal-spatial memory.**

## What TaoDB Is

TaoDB is a **temporal-spatial memory engine** for LLM agents. It stores raw memories and retrieves them by **when** and **where** — not by vector similarity.

```
Write:  LLM produces a memory → taoDB stores it with time + space tags
Recall: LLM needs context → taoDB returns memories from that time window + spatial scope
        → LLM reads the raw memories and decides relevance
```

Three things TaoDB does:

1. **Temporal indexing** — Every memory has a narrative timestamp. Recall by time window: "what happened in the last 5 chapters / 30 days / 3 sprints."
2. **Spatial indexing** — Every memory has container tags: `module:auth`, `character:桑安歌`, `scene:邯郸酒肆`. Recall by spatial scope.
3. **Energy model** — Memories decay over time unless protected. Important facts stay permanent. Irrelevant details fade naturally.


## How It Works

TaoDB runs as a **local MCP server**. No network. No ports. No API tokens.

```
taodb init           # creates .mcp.json in your project
restart your agent   # agent detects .mcp.json, spawns taoDB via MCP stdio
                     # taoDB reads/writes taodb-memory/ locally
                     # the agent has memory. invisible.
```

The agent talks to taoDB over stdin/stdout — the same way it talks to any MCP tool. No server to configure. No token to manage. No firewall to open.


## What TaoDB Is Not

TaoDB is **not** a vector database. It has no embeddings, no similarity search, no FTS ranking. If you need "find passages similar to this one" — use Pinecone or Qdrant.

TaoDB is **not** a search engine. It doesn't do BM25, doesn't rank by relevance, doesn't understand your content. The LLM does the understanding. TaoDB handles what LLMs cannot: persistent storage with temporal-spatial indexing.

TaoDB is **not** an agent. It makes zero decisions. It doesn't auto-extract facts, doesn't summarize, doesn't trigger on events. Your agent drives the loop. TaoDB is the memory, not the brain.

## Who It's For

TaoDB is for **AI Creators** — anyone who uses LLM agents as a creative partner.

**Vibe coders** — Your agent remembers which modules you touched, which bugs you fixed, which design decisions you made. No more re-explaining your codebase every session.

**Novel writers** — Your writing agent recalls character states, object histories, foreshadowing planted in chapter 50. Temporal indexing maps naturally to chapter numbers.

**Knowledge workers** — Research projects, meeting notes, learning journals — all time-indexed. Your agent can answer "what did I learn about this topic in March?" without you organizing anything.

**AI designers** — Your agent remembers design system rules, component iterations, client feedback. Every revision has context. No re-explaining the grid system or brand colors.

**Video & ad producers** — Project briefs, edit decisions, platform specs, client revisions. Pipeline continuity across shoots, campaigns, and platforms.

**Agent developers** — Building an agent that needs persistent memory across sessions. MCP-native, zero-config, embedded. Drop it in and your agent has a hippocampus.

## How It's Different

| | TaoDB | Vector DB (Pinecone/Qdrant) | Mem0 / Zep |
|---|---|---|---|
| **Index** | Time + Space | Vector similarity | Semantic + Chat |
| **Retrieval** | "What happened last week in auth module" | "Text similar to this query" | "Semantically relevant to this message" |
| **Decay** | Yes — energy floor model | No | No |
| **Protocol** | MCP (agent-native, stdio) | REST / gRPC | REST |
| **Dependencies** | Zero (embedded redb) | Cloud / heavy | PostgreSQL / Cloud |

## Quick Start

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash

# Initialize in your project
cd my-project
taodb init

# Restart your agent. Done.
```

No API keys. No accounts. No cloud. Your agent connects via MCP and starts using taoDB immediately.

---

**Next:** [Getting Started](getting-started.md) → walk through your first session.
