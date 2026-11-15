# Contributing to Welkin

Contributions are welcome. This document covers the basics.

## License

Welkin is LGPL-3.0-or-later. By submitting a pull request you agree that your
contribution is licensed under the same terms.

**Important**: Welkin is a clean-room Rust port of OpenSimulator. You may
reference OpenSimulator's BSD-licensed C# source and LibreMetaverse (BSD
3-Clause) for protocol details and algorithms. Do not copy code from
GPL-licensed projects (Firestorm, Aurora Viewer, or their forks) into
Welkin. If you need behavior documented only in GPL source, re-implement
it from the protocol specification or observed behavior.

## Getting started

1. Fork the repo and create a feature branch from `main`.
2. Install the Rust toolchain: `rustup update stable`.
3. Build: `cargo build --workspace`.
4. Test: `cargo test --workspace`.
5. Lint: `cargo clippy --workspace -- -D warnings`.

## Pull requests

- One logical change per PR. Keep it small enough to review in one sitting.
- Fill in the PR template. The OpenSim reference section matters: reviewers
  need to know which C# code you consulted.
- All CI checks must pass before merge.
- PRs require one approving review.

## Reporting bugs

Use the bug report template in the issue tracker. Include your Welkin
version, the viewer you connected with, and steps to reproduce.

## Code style

- `cargo fmt` with the project's `rustfmt.toml`.
- No `unsafe` without a comment explaining why it's necessary.
- Public API items get a one-line doc comment. Internal code doesn't need
  comments unless the reason for a choice is non-obvious.

## Communication

Open an issue or start a discussion for questions, design proposals, or
anything that needs conversation before code.
