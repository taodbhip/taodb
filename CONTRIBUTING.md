# Contributing to taodb

## Getting Started

```bash
git clone https://github.com/taodbhip/taodb.git
cd taodb
cargo build
cargo test
```

## Running Tests

```bash
# All tests
cargo test

# Specific test suite
cargo test --test e2e
cargo test --test v5_four_layer
```

## Code Style

- Follow `rustfmt` defaults (`max_width = 120`, edition 2024)
- Keep files under ~300 lines where practical
- Chinese comments in code are acceptable (primary developer is Chinese)
- User-facing strings must be in English
- Run `cargo clippy` before committing

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Write tests for new functionality
4. Ensure `cargo test` and `cargo clippy` pass
5. Open a PR with a clear description

## Architecture

See [AGENTS.md](AGENTS.md) for the design philosophy and architecture overview.

## Reporting Issues

Use GitHub Issues. Please include:
- Steps to reproduce
- Expected vs actual behavior
- Environment (OS, Rust version, taodb version)
