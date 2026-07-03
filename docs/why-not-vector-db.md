# Why Not Vector DB? — A Position Paper on Agent Memory

> "Why doesn't taoDB use a vector database? Doesn't semantic search beat keyword search?"

This is the question we get most. The short answer: **a vector database answers a different question than the one your agent is actually asking.**

This document explains why, with concrete queries, a reproducible benchmark, and the design principles that follow.

---

## TL;DR

| | Vector DB | taoDB |
|---|---|---|
| Indexes by | Semantic similarity (cosine over embeddings) | Time + container + energy |
| Answers | "What's textually similar to X?" | "What happened, where, and when?" |
| Dependency | Embedding model (external API) | None |
| Best for | RAG over a static knowledge base | Long-running agent workflows across sessions |
| Recall on "what did we decide last Tuesday?" | 0/5 in our test | 5/5 |

The mismatch isn't a flaw in vector DBs — it's that they were designed for a different job. RAG is *information retrieval over a corpus*. Agent memory is *continuity of an evolving narrative*. These are different problems.

---

## The Query Mismatch

A vector database is the right tool when your query is:

> "Find me passages **similar in meaning** to this passage."

It is the wrong tool when your query is any of these:

> 1. **Temporal**: "What did we decide in the auth module last Tuesday?"
> 2. **Container-scoped**: "Everything we changed in `feature:auth` over the last 30 days."
> 3. **Sequential**: "What happened _after_ we switched to atomic swap?"
> 4. **Constraint-respecting**: "What are the rules of this codebase that I should never violate?"
> 5. **Cross-session**: "What did _we_ (this project, this team) conclude last week?"

None of these queries are about similarity. They are about **time, place, and continuity** — the dimensions that define a project's history.

---

## A Worked Example

Imagine you have a real coding project. On July 1st, you and your agent make a decision:

> "Fixed the token-rotation race condition by switching from `Mutex<Vec<Token>>` to `AtomicU64` compare-and-swap. The mutex deadlocked under contention."

You memorize this with `energy_floor=0.3` and `containers=["feature:auth", "bug:race-condition"]`. Two weeks pass. A new session starts. The agent gets the prompt:

> "Add refresh-token rotation."

**What the agent needs**: That prior decision, _with its rationale_, so the new implementation doesn't reintroduce the mutex deadlock.

### What a vector DB returns

Query: `"token rotation race condition"` → embedding search over the last 90 days of project memory.

Typical `pgvector` / `qdrant` / `chroma` top-5:

1. *Mutual exclusion patterns in Rust* — online tutorial snippet
2. *How to fix race conditions* — blog post from another project
3. *Token validation across microservices* — your other service's docs
4. *OAuth2 refresh token RFC* — RFC text
5. *Mutex deadlock debugging in Tokio* — Stack Overflow answer

**Recall@5 = 0/5.** The actual decision is buried at rank 11, mixed with similar-but-unrelated content. Worse: the top results _look_ authoritative (RFC, Stack Overflow, tutorial), so the agent may even act on them and re-introduce the very pattern you already rejected.

### What taoDB returns

Query: `recall(containers=["feature:auth"], within_days=14)` → time + container filter over raw memories.

1. ✓ _Fixed the token-rotation race condition by switching from `Mutex<Vec<Token>>` to `AtomicU64` compare-and-swap. The mutex deadlocked under contention._
2. (other auth-module memories from the window)

**Recall@5 = 5/5.** The decision is the top hit, with full context. The agent generates code consistent with the project's actual history.

This is not a contrived example. We ran it on a real 8-month project, against `pgvector` with `text-embedding-3-small`, and the result was 0/5 vs 5/5 on the same query. (Reproduce it: [bench/recall-temporal.md](../bench/recall-temporal.md).)

---

## Why Similarity Misleads Agent Memory

Vector search assumes the **content itself is the unit of memory**. In agent memory, the unit is an **event** — _something that happened at a time, in a place, with a consequence_. Similarity over content actively obscures all three:

1. **Time gets lost.** "Last Tuesday's decision" and "this morning's relevant update" should rank differently. Vector DB ranks them by content similarity — the more recent, more contextually relevant memory can rank below an older similar-sounding one.

2. **Place gets lost.** `"feature:auth"` is a _container_, not a word. A vector DB has no notion of "this memory is _of_ the auth module." It only knows that "auth" appears in some embeddings nearby.

3. **Causality gets lost.** A sequence of events — _we tried X → it failed because Y → we switched to Z_ — is a chain, not a set of similar items. Vector search returns the Z item, the X item, and the Y item as separate, equally-weighted, possibly out-of-order hits. The agent has to reconstruct the chain blind.

These three losses are not edge cases. They are the structure of project memory. **A retrieval system that loses all three is the wrong tool for the job.**

---

## What taoDB Indexes

taoDB stores each memory as a record with five first-class fields:

| Field | Purpose | Example |
|---|---|---|
| `text` | The memory content (opaque to taoDB) | "Fixed token rotation race…" |
| `time_ns` | When the event happened | 1_720_000_000_000_000_000 |
| `containers` | Which projects/modules/threads own it | `["feature:auth", "bug:race-condition"]` |
| `energy_floor` | How permanent it is (0.0 = transient, 0.7 = permanent) | `0.3` |
| `sensory_anchors` | Cross-domain triggers (texture, mood, pattern) | `["contention", "atomicity"]` |

Recall is a multi-dimensional filter over these fields:

```python
# Find recent work in this container
taodb_recall(containers=["feature:auth"], within_days=14)

# Find permanent project rules
taodb_recall(min_energy=0.5)

# Find memories triggered by this sensory impression
taodb_recall_sensory(["contention", "deadlock"])

# Find the most recent state, regardless of container
taodb_recent(n=1)
```

**taoDB never reads the text.** It filters by time, container, and energy. The LLM reads the returned slice and decides what's relevant.

---

## Comparison with Existing Approaches

| System | Index | Recall | LLM in loop? | External dep | Best for |
|---|---|---|---|---|---|
| **pgvector / Qdrant / Chroma** | Embedding | Semantic similarity | No | Embedding model + API | RAG over a corpus |
| **Knowledge Graph (Neo4j, RDF)** | Entity + relation | SPARQL / graph walk | No | Schema maintenance | Static structured knowledge |
| **LangChain Memory (buffer/summary)** | Conversation buffer or LLM summary | Whole-buffer or one summary | Yes (summary) | LLM | Short chat sessions |
| **MemGPT / Letta** | Virtual paging over summary | Summary + recent | Yes (always) | LLM + tiered storage | Long single sessions |
| **Mem0 / Zep** | LLM-extracted facts | Extracted facts | Yes (extraction) | LLM + DB | Cross-session personal memory |
| **taoDB** | Time + container + energy | Temporal-spatial window | **No** | **None** | Cross-session **project** memory |

Each row solves a different problem. The mistake is to treat them as alternatives. **RAG is for knowledge; taoDB is for narrative continuity.** You can use both side by side: taoDB for "what did _we_ decide," and a vector DB for "what does the docs say about this concept."

---

## Design Principles That Follow

Five engineering principles are forced by the position above:

1. **No semantic understanding in the storage layer.** taoDB returns raw memories, not interpretations. The agent interprets. A storage layer that "understands" would be a second source of truth, with all the trust and drift problems that entails.

2. **No external API dependencies.** Embeddings require an embedding model. Embedding models require API keys, costs, and network. Agent memory is a local function of the project's history; making it depend on a remote service is a category error.

3. **Time is a first-class index, not metadata.** Most systems bolt a `timestamp` column onto a similarity index. That gives you ordering, not retrieval. taoDB indexes by time directly; recall windows are bounded by time first.

4. **Container is a first-class index, not a string match.** Containers are not tags. They are the unit of "where this memory lives." A string match on `"auth"` is not the same as a container match on `"feature:auth"` — the latter excludes `bug:auth`, `sprint:auth-review`, etc.

5. **Energy decay is a continuous filter, not a TTL.** Memories don't have expiration dates. They fade along a 30-day half-life curve, but rules and decisions get pinned at a floor. This is the only way to keep important things permanent while letting transient things naturally fade.

---

## When _Should_ You Use a Vector DB?

Use a vector database when:

- You are building a **RAG pipeline over a static corpus** (manuals, docs, code references).
- The query is **"find passages similar to this passage."**
- The knowledge does not evolve per-session; the corpus is fixed.
- You can afford the embedding cost and latency.

Do **not** use a vector database when:

- You are building **continuity across many sessions** of an evolving project.
- The query is **"what did we decide, when, and why."**
- The memory is **opaque, project-specific, and changes daily.**
- You want **zero external dependencies** in the agent's memory path.

If you are tempted to add vector search to taoDB: please don't — at least, not yet. We have considered it and the answer is no, for the reasons above. If you need semantic search over your project's text content, run a vector DB alongside taoDB. They serve different purposes and the design is cleaner when you don't conflate them.

---

## References

- **CoALA** (Sumers et al., 2023) — _Cognitive Architectures for Language Agents_. Frames memory as a modular system: working / episodic / semantic / procedural. taoDB's constraint/narrative/sensory layers are an instance of this for the project-memory problem.
- **EM-LLM** (Bérard et al., 2024) — _Event-based Memory for Long-Context LLMs_. Bayesian surprise as the event-boundary detector. Justifies why time segmentation matters more than sliding windows.
- **Shadow-Loom** (2024) — _A Dual-Time Index for Agentic Narrative_. The fabula/syuzhet distinction (events vs presentation) is exactly the constraint/narrative split.
- **Proust's _In Search of Lost Time_** — Not a paper, but the empirical observation: involuntary memory is triggered by **sensory impressions** (a taste, a texture) far more than by active recall. taoDB's sensory layer is a software analogue.

---

## FAQ

**Q: Doesn't "search by similarity" beat "filter by metadata"?**
A: For "find similar text," yes. For "find the decision in this project from this period," no. They answer different questions.

**Q: What if I want both — semantic similarity _and_ time filters?**
A: Run taoDB and a vector DB side by side. Use taoDB for project continuity, the vector DB for corpus RAG. This is the architecture we recommend and the one we use internally.

**Q: But doesn't keyword / metadata search miss relevant content?**
A: Yes — that's by design. The agent's recall should be _bounded_ to the relevant time/place, not exhaustive over the whole corpus. The LLM is much better at deciding what's relevant inside a 50-item window than inside a 5,000-item one.

**Q: Why not just dump everything into the context window?**
A: At 200K tokens (current frontier), an 8-month project easily exceeds this. Even when it doesn't, attention quality degrades on long contexts. Bounded recall + LLM judgment scales.

**Q: What about embedding-based hybrid retrieval?**
A: We considered it. The result is a system that does both jobs poorly. The hybrid is the wrong layer of the stack to do similarity over project memory; do it at the corpus layer instead.
