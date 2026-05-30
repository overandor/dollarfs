# lfv — Local File Value System

A proprietary local-first macOS terminal that makes file work economically observable.

## What it does

lfv tracks files, file changes, authorship, provenance, work sessions, and estimated dollar value of work performed on a macOS computer.

Every file becomes an accountable economic object with:

- Content hash (BLAKE3)
- Path history
- Dollar book value estimate
- Confidence score
- Security risk flag
- Valuation reason

## Commands

```bash
lfv init                          # Initialize database and config
lfv scan <path>                   # Scan and index files
lfv scan <path> --incremental     # Only index changed files
lfv value <path>                  # Show estimated dollar value
lfv top                           # List most valuable files
lfv day                           # Today's work ledger
lfv secrets <path>                # Scan for leaked secrets
lfv secrets <path> --detail      # Show line numbers and redacted previews
lfv watch <paths...>              # Watch directories for changes
lfv duplicates                   # Detect and group duplicate files
lfv export markdown              # Generate report
lfv tui                          # Open terminal UI dashboard
lfv status                       # System status
lfv doctor                       # Run diagnostics
```

## Important truth label

**Dollar values are internal estimates for accountability, prioritization, and R&D valuation. They are not guaranteed market prices, resale values, or investment returns.**

## Installation

Requires Rust 1.83+ and macOS.

```bash
cd local-file-value-system
cargo build --release
# Binary at target/release/lfv
lfv init
```

## Stack

- Core engine: Rust
- CLI: clap
- Database: SQLite (WAL mode)
- Hashing: BLAKE3
- Config: TOML
- Reports: Markdown, JSON

## License

Proprietary. See `LICENSE-PROPRIETARY.md`.
