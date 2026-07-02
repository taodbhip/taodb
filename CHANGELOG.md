# Changelog

All notable changes to taodb will be documented in this file.

## [1.0.0] - 2026-07-03

### Added
- Initial public release — built for AI Creators
- MCP server with 8 tools (`taodb_memorize`, `taodb_recall`, `taodb_recent`, `taodb_forget`, `taodb_stats`, `taodb_decay`)
- HTTP REST API (`/v1/memories`, `/v1/recall`, `/v1/recent`, `/v1/decay`, `/v1/stats`, `/v1/users`, `/v1/projects`)
- Three-layer memory architecture: Constraint (permanent rules) / Narrative (decaying events) / Sensory (cross-domain triggers)
- Multi-tenant user/project isolation
- CRC32 data integrity verification
- Temporal-spatial indexing via redb B-tree
- Reconsolidation boost on recall (+0.05 energy)
- Energy floor mechanism for permanent memories (0.0 transient → 0.7 permanent)
- Dual protocol support (MCP + HTTP)
- Cryptographic token generation (rand + OS CSPRNG)
- Sensory indexing (Proust involuntary memory — cross-container, cross-timeline recall by texture/mood/pattern)
- `taodb_recall_constraints` and `taodb_recall_sensory` tools
- Multi-dimensional recall (道/天/地/人/物) with weighted scoring
- English and Chinese documentation
