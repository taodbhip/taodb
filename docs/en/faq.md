# FAQ

## General

**Q: What's the difference between taoDB and a vector database?**

Vector databases (Pinecone, Qdrant, Weaviate) index by semantic similarity. You store embeddings and query by "find similar vectors." TaoDB indexes by time and space. You store memories with time stamps and container tags, and query by "what happened in the auth module last month."

Vector DBs answer "what's similar to X." TaoDB answers "what happened when and where." They're complementary, not competing.

**Q: Is taoDB a search engine?**

No. TaoDB has no full-text search, no BM25 ranking, no relevance scoring based on content. The LLM reads raw memories and decides relevance. TaoDB just provides the right subset of memories at the right time.

**Q: Does taoDB use embeddings or LLMs internally?**

No. TaoDB is a pure storage engine. It uses redb (embedded B-tree), bincode serialization, and CRC32 integrity checks. No embeddings, no LLM calls, no external API dependencies.

**Q: Can I use taoDB without an LLM agent?**

You can use the HTTP API from any application. But taoDB is designed to be consumed by LLMs — the memories are raw text meant to be read and interpreted by an AI. Using it as a general-purpose database is possible but misses the point.

**Q: How is this different from just using a SQLite database with timestamps?**

TaoDB adds several layers on top of raw storage:
- Energy floor model with automatic decay
- Multi-dimensional recall scoring (time × space × energy × body/emotion × objects)
- Constraint vs narrative layer separation
- Sensory cross-indexing
- Reconsolidation boost on recall
- MCP protocol integration

You could build these on SQLite. TaoDB packages them into a purpose-built engine.

## Installation & Setup

**Q: Do I need Rust installed?**

No. The install script downloads a prebuilt binary. No Rust toolchain required.

**Q: macOS says the binary is from an unidentified developer.**

TaoDB binaries are ad-hoc signed but not notarized (notarization requires an Apple Developer account). Right-click the binary in Finder → Open, or run:

```bash
xattr -d com.apple.quarantine /usr/local/bin/taodb
```

**Q: Does taoDB phone home?**

No network calls at runtime. The binary has zero telemetry, zero analytics, zero network dependencies. It reads and writes local files only. (The install script calls GitHub's API to find the latest release, but that's the install script, not taoDB itself.)

**Q: Can I use taoDB in CI/CD?**

Yes. Since it's a single binary with no dependencies, it works in CI environments. Use the install script or download directly from GitHub Releases in your CI pipeline.

## Usage

**Q: How many memories can taoDB store?**

The embedded redb database scales to millions of records. The in-memory cache loads all memories at startup — performance depends on available RAM. At 10K memories (roughly 10KB each), memory usage is about 100MB. At 100K, about 1GB.

**Q: When should I run decay?**

After major milestones: end of sprint, end of volume, monthly. Not every session. Decay is a batch operation, not a per-session cleanup.

**Q: My agent isn't using taoDB. What should I check?**

1. Is `.mcp.json` present and valid JSON in your project root?
2. Is `taodb` in PATH? Run `which taodb`.
3. Did you restart the agent after `taodb init`?
4. Check the agent's MCP logs for connection errors.

**Q: Can multiple agents share the same taoDB instance?**

For MCP (local mode): one agent per project directory. The redb database is file-locked to a single process.

For HTTP (server mode): multiple agents can connect to `taodb serve`. Use API tokens and project IDs for isolation.

**Q: How do I back up my memories?**

Copy the `taodb-memory/` directory. Everything is stored there as redb database files. No external dependencies, no export needed.

**Q: Can I move memories between machines?**

Copy `taodb-memory/` to the new machine. The MCP server will pick it up on next restart.

## Design

**Q: Why redb instead of SQLite?**

redb is an embedded B-tree engine written in pure Rust. It provides ACID transactions without SQL overhead, fits the "zero dependencies" philosophy, and has simpler semantics for the key-value access patterns taoDB uses.

**Q: Why not add vector search as an optional feature?**

TaoDB's core thesis is that temporal-spatial indexing is fundamentally different from semantic search. Adding vector search would blur that distinction and make the product harder to explain. If you need vector search, use a vector database alongside taoDB — they serve different purposes.

**Q: Why MCP instead of just HTTP?**

MCP (Model Context Protocol) is the emerging standard for LLM agent tool integration. MCP stdio transport means zero network overhead, zero configuration, zero authentication setup. The agent discovers taoDB via `.mcp.json` and starts using it immediately. HTTP is available as a secondary protocol for non-MCP integrations.

**Q: What's the "energy" actually computing?**

Energy is a float 0.0–1.0 computed from: emotional intensity of the original memory, temporal distance from the narrative anchor, and association strength. The formula uses a 30-day (narrative) half-life. The `energy_floor` acts as a hard minimum — a memory with `energy_floor = 0.7` is always at least 0.7 regardless of age.

## Contributing

**Q: How do I report a bug?**

Open an issue on GitHub. Include the taoDB version (`taodb --version`) and steps to reproduce.

**Q: Can I contribute code?**

Yes. See [CONTRIBUTING.md](../CONTRIBUTING.md). PRs welcome — especially for container schema templates, agent instructions for different workflows, and platform support.

**Q: Is there a roadmap?**

The design philosophy is in [DESIGN.md](../DESIGN.md). Current focus: stability, documentation, and growing the MCP ecosystem integration. Cloud hosting and Python SDK are planned but not priority.
