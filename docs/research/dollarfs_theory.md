# DollarFS: Local File Value System

## Abstract

DollarFS (lfv) is a proprietary local-first macOS terminal that makes file work economically observable. It tracks files, changes, authorship, provenance, work sessions, and estimated dollar value.

## 1. Introduction

Traditional file systems lack economic observability. DollarFS makes every file an accountable economic object with content hash, path history, dollar book value, confidence score, and security risk flag.

## 2. Architecture

- **Core Engine**: Rust-based file tracking
- **CLI**: clap-based command interface
- **Database**: SQLite (WAL mode)
- **Hashing**: BLAKE3 for content integrity
- **Config**: TOML-based configuration

## 3. Economic Model

Files are valued based on:
- Content complexity
- Change frequency
- Authorship provenance
- Security risk assessment
- Work session tracking

## 4. Features

- File scanning and indexing
- Incremental updates
- Dollar value estimation
- Secret detection
- Duplicate detection
- Work ledger
- TUI dashboard

## 5. Important Truth

Dollar values are internal estimates for accountability, prioritization, and R&D valuation. They are not guaranteed market prices, resale values, or investment returns.

## 6. Conclusion

DollarFS provides economic observability for local file work.
