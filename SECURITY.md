# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in taodb, please report it privately:

- **Email**: [open an issue request to enable private reporting contact] or use [GitHub Security Advisories](https://github.com/taodbhip/taodb/security/advisories/new)
- **What to include**:
  - Description of the vulnerability (what can an attacker do?)
  - Steps to reproduce (minimal test case if possible)
  - Affected version(s)
  - Potential impact

We will acknowledge receipt within 72 hours and aim to ship a fix within 14 days for critical issues.

## Scope

taodb is a local-first memory engine. Out of scope:

- **Self-hosted deployments**: If you expose the taodb HTTP server (`taodb serve`) on a public network, you are responsible for TLS, authentication, and access control. The default admin token `tk_admin` is for local dev only — **never** keep it in production.
- **Embedded use**: taodb running as an MCP server inside an agent is single-tenant and trusted. Anyone with shell access to that user can read memories.

In scope:

- Memory data integrity (CRC32 verification, file tampering)
- Cross-tenant data leakage between users/projects
- Reconsolidation / energy manipulation attacks
- redb / storage-corruption bugs
- Anything that lets memory lookups return cross-user data

## Design Notes

- **Multi-tenant isolation**: Every memory lives under `users/<user_id>/projects/<project_id>/`. Lookups never span users.
- **CRC32 integrity**: Every serialized memory carries a CRC32 checksum; mismatches return a corruption error rather than silently returning bad data.
- **Cryptographic tokens**: User API tokens are generated from `rand` + OS CSPRNG (via `getrandom`). They are random; tokens cannot be derived from user_id or email.
- **No remote code execution surface**: taodb does not parse memory content as code. Memories are opaque strings/JSON.

## Disclosure Policy

We follow [coordinated disclosure](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure). Please give us a reasonable window to fix issues before public disclosure.

Thanks for helping keep taodb and its users safe.
