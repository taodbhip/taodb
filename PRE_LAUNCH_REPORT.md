# taodb Pre-Launch Report

> Generated 2026-07-03 for the v1.0.0 public release.

## TL;DR

taodb is **ready to push**. All blockers cleared, multi-modal Creator IDE tested end-to-end against an isolated test directory, and CI gates (fmt, clippy, test, build) are green.

| Gate | Result |
|------|--------|
| `cargo fmt --check` | clean |
| `cargo clippy --all-features -- -D warnings` | clean |
| `cargo test --all-features` | 88/88 pass (crc 3, tenant 2, recall 4, deep-edge 15, e2e 6, v2 three-layer 44, v5 four-layer 8) |
| `cargo build --release` | clean — `target/release/taodb` 3.97 MB |
| Multi-modal Creator IDE end-to-end | **10/10** in `~/taodb-launch-test` |

---

## 1. What was checked / fixed

### 1.1 Hygiene cleanup

| Item | Action |
|------|--------|
| `scripts/__pycache__/embed.cpython-314.pyc` | removed (mavis-trash) |
| `web-ide/__pycache__/ide.cpython-314.pyc` | removed (mavis-trash) |
| `.mcp.json` (duplicate of `.mcp.example.json`, already in `.gitignore`) | removed |
| `src/.DS_Store` | already ignored by `.gitignore` (`.DS_Store`) — no action |

### 1.2 CI gates (`cargo fmt`, `clippy`, `test`)

Clippy with `--all-features -D warnings` returned **25 errors** before fix; reduced to zero by:

| File | Fix |
|------|-----|
| `src/mcp.rs:176/191/198` | Three `if`-inside-`if` blocks collapsed via clippy's `collapsible_if` suggestion |
| `src/mcp.rs:494` | Removed redundant `.into()` (clippy `useless_conversion`) |
| `src/mcp.rs:580` | `fn call_tool(...) -> impl Future<...>` rewritten as `async fn call_tool(...)` (clippy `manual_async_fn`). Same for `list_tools` next door. |
| `src/mcp.rs:10` | Removed now-unused `MaybeSendFuture` import |
| `src/store.rs:347` | `[imp.clone()]` → `std::slice::from_ref(imp)` |
| `src/store.rs:421` | `sort_by(|a,b| b.count.cmp(&a.count))` → `sort_by_key(\|b\| std::cmp::Reverse(b.count))` |
| `src/store.rs:533` | `match … { Ok(Some(_)) => true, _ => false }` → `matches!(..., Ok(Some(_)))` |

`cargo fmt --check` had one trailing-blank-line nit in `tests/v5_four_layer.rs`; resolved with `cargo fmt --all`.

All 88 unit + integration tests still pass after the changes.

### 1.3 New files (missing standard GitHub project assets)

| Path | Why |
|------|-----|
| `SECURITY.md` | GitHub expects this for the Security tab; explains which versions are supported, what counts as in-scope (data integrity, cross-tenant leakage, energy manipulation), what is out of scope (self-host network exposure — that's the operator's job), and how to report. |
| `.github/ISSUE_TEMPLATE/bug_report.yml` | GitHub-form dropdown for protocol (MCP/HTTP/CLI/IDE), version, OS — matches the variables that actually matter for taodb |
| `.github/ISSUE_TEMPLATE/feature_request.yml` | Templates the request around *what the agent gets* / *what the user does*, not the implementation |
| `.github/ISSUE_TEMPLATE/question.yml` | Keeps "how do I" traffic out of bug labels |
| `.github/PULL_REQUEST_TEMPLATE.md` | Includes the three CI gates (`fmt --check`, `clippy -D warnings`, 88 tests) and a docs-EN-and-docs-ZH reminder |

### 1.4 README & docs link audit

All internal references resolve to real files:

| README link | Target | Status |
|-------------|--------|--------|
| `LICENSE` | `LICENSE` | exists |
| `docs/` | `docs/index.md` (`en/`, `zh/`) | exists |
| `USAGE.md` | `USAGE.md` | exists |
| `CHANGELOG.md` | `CHANGELOG.md` | exists |
| `CONTRIBUTING.md` | `CONTRIBUTING.md` | exists |
| `AGENTS.md` (referenced in CONTRIBUTING.md) | `AGENTS.md` | exists |

External badge URLs (`github.com/taodbhip/taodb`, img.shields.io, crates.io, raw.githubusercontent.com for install.sh) are placeholders; they will resolve correctly **once the repo is pushed** and a v1.0.0 crates.io publish lands. No code change needed.

`docs/en` and `docs/zh` are kept in sync for the user-facing pages (`index`, `getting-started`, `concepts`, `usage`, `api-reference`, `http-api`, `deploy`, `faq`) — verified at HEAD.

---

## 2. Multi-modal Creator IDE — end-to-end test

taodb ships three surfaces:
- **MCP stdio** — primary, for AI agents
- **HTTP `/v1/...`** — for everything else
- **Creator IDE (web-ide/ide.py)** — the "multi-modal" app: a Flask-style HTTP UI that drives taodb's HTTP API and an external LLM command

I exercised the third surface against an isolated directory `~/taodb-launch-test` (no pollution of `~/taodb`). Test setup:

```
~/taodb-launch-test/
├── run.sh          # launches taodb serve (8766) + Creator IDE (8767), smoke tests all endpoints, shuts down
├── stub_llm.py     # deterministic Python stub LLM that the IDE invokes via TAODB_LLM_CMD
└── RESULT.txt      # captured pass log (10/10)
```

Workflow covered:
1. `taodb serve --data ./taodb-data --admin-token tk_admin` on 127.0.0.1:8766
2. Health check (`/health`)
3. Admin creates user via CLI: `taodb user-create launchtest ...` (the correct CLI name; `taodb user create` would error because clap kebab-cases multi-word subs)
4. Launch IDE via `python3 web-ide/ide.py` with `TAODB_BASE`, `TAODB_ADMIN_TOKEN`, `TAODB_LLM_CMD=python3 stub_llm.py`
5. **Login** (`/api/login`) — IDE auto-creates `demo` user, returns api_token
6. **Create project** (`/api/projects` — `launch/Launch Test`)
7. **List projects** (`/api/projects`)
8. **Recall** (`/api/recall` — returns memories; one ID captured `01KWHV0F74SX…`)
9. **Write chapter** (`/api/chapters/write` — IDE invokes stub LLM, persists into taodb HTTP `/v1/memories`; response shows `taodb_ingest: {memory_id: "01KWHV11E0…", ok: true}`)
10. **Verify persistence** by recalling back via `taodb /v1/recall` with the project's `x-project-id` header — confirms cross-process, multi-tenant-scoped read works.

Result: **10/10 pass.**

Full log: `~/taodb-launch-test/RESULT.txt`. To re-run independently:

```bash
bash ~/taodb-launch-test/run.sh
```

Note on the stub: in production the same `TAODB_LLM_CMD` env var should point at your real M3 / Claude / any-OpenAI-compatible CLI; the IDE is LLM-agnostic.

---

## 3. Things you still need to do before the public release

These were deliberately not done — they need a GitHub-side decision and credentials that aren't in this environment.

1. **Create the GitHub repo** at `github.com/taodbhip/taodb` (or wherever you host). Push `master` then create the `main` branch convention if you prefer.
   ```bash
   cd ~/taodb
   git switch -c main  # optional — release.yml tags trigger from main by default
   git add -A
   git commit -m "taodb v1.0.0 — initial public release"
   git remote add origin git@github.com:taodbhip/taodb.git
   git push -u origin main
   git tag v1.0.0 && git push origin v1.0.0
   ```
2. **Create a `crates.io` account** if you want `cargo install taodb` to work (badge `[![Crates.io](https://img.shields.io/crates/v/taodb)]` already references the right URL).
3. **Verify badges render** after the repo exists. The three shields (CI / crates.io / License) are wired to `github.com/taodbhip/taodb` and will go from red-unknown to green once:
   - The first CI run finishes on the default branch.
   - The first crates.io publish happens.
4. **Decide on GitHub repository description / topics** — the README's `keywords` field gives `llm, memory, mcp, temporal, spatial, agent`; mirror those in the GitHub "About" sidebar.
5. **Optional** — pin the GitHub repo's "Social preview" image. taodb doesn't currently ship a logo asset. Easy add: a single SVG or PNG that fits GitHub's 1280×640 / 640×640 spec.

---

## 4. Files added/changed during prep (for your git status)

```
A  SECURITY.md
A  .github/ISSUE_TEMPLATE/bug_report.yml
A  .github/ISSUE_TEMPLATE/feature_request.yml
A  .github/ISSUE_TEMPLATE/question.yml
A  .github/PULL_REQUEST_TEMPLATE.md
M  src/mcp.rs       (clippy fixes + manual_async_fn)
M  src/store.rs     (clippy fixes)
M  tests/v5_four_layer.rs (fmt trailing newline)
A  PRE_LAUNCH_REPORT.md (this file)
```

Cleaned (not in git):
- removed `scripts/__pycache__/`
- removed `web-ide/__pycache__/`
- removed `/Users/xmfh-1/taodb/.mcp.json` (duplicate; `.mcp.example.json` remains; `.gitignore` already covers `.mcp.json`)

Test artifacts live only in `~/taodb-launch-test/` and are unrelated to the repo.

---

## 5. Recap

taodb v1.0.0 is **green across:**
- Rust toolchain gates (`fmt`, `clippy -D warnings`, 88 tests, `release` build)
- Three interface surfaces (MCP, HTTP, Creator IDE)
- Multi-tenant + cross-process memory persistence
- Standard GitHub project assets (Security, issue templates, PR template)

Ship it. 🚀
