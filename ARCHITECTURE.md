# ARCHITECTURE

## Overview

lfv is a Rust workspace with modular crates.

## Crate Layout

```
crates/
  dfs-core/       Database, models, scanner
  dfs-cli/        CLI binary (lfv)
  dfs-valuation/  Dollar valuation engine
  dfs-security/   Secret/credential scanner
```

## Data Flow

1. `lfv scan <path>` → `dfs-core::scanner` walks directory tree
2. Files hashed with BLAKE3, metadata stored in SQLite
3. `lfv value` → `dfs-valuation` reads files, computes value, writes valuations
4. `lfv secrets` → `dfs-security` scans content against regex patterns
5. `lfv day` → aggregates events and values into daily ledger

## Database

SQLite with WAL mode. Schema in `dfs-core/src/db.rs`.

## Valuation Model

See `VALUATION_MODEL.md`.

## Security Model

See `SECURITY.md`.

## Performance Targets

- CLI startup: < 100ms
- Scan: 100k files without crash
- Memory: < 100MB
- No network by default
