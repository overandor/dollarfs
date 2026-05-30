# Security Advisory: ADVISORY-2026-001

## DollarFS Economic-State Corruption and Local DoS

**Severity:** Critical  
**Affected versions:** <= 0.2.0  
**Patched version:** 0.3.0  
**Date:** 2026-05-30  
**CVSS (estimated):** 7.5 (High) — integrity + availability  

---

## Summary

Two separate but related vulnerabilities were identified in the DollarFS valuation and scanning engine:

1. **Economic-state corruption:** The valuation engine computed `minutes_estimated = file.size / 500.0` with **no upper bound**. An attacker could create a large or sparse file and inflate its collateral value to hundreds of millions or billions of USD. At the default `$150/hour`, a 100 GB file produced an estimated **~$537M** book value. At `$300/hour`, it exceeded **$1B**.

2. **Local/remote DoS:** The security scanner used `std::fs::read_to_string(path)`, reading entire files into memory. A single large file could exhaust scanner memory before valuation even began.

---

## Affected Components

- `crates/dfs-valuation/src/lib.rs` — `value_file`, `estimate_file_type_value`
- `crates/dfs-security/src/lib.rs` — `scan_file`, `scan_directory_raw`
- `crates/dfs-core/src/scanner.rs` — `index_file`

---

## Root Cause

The core DollarFS / MEMBRA doctrine was violated: **file size was treated as economic labor value.**

The formula:
```
minutes_estimated = file.size / 500.0
base_score = hourly_rate_usd * (minutes_estimated / 60.0)
book_value = base_score + complexity + (file.size * 0.0001)
```

This is linear in bytes. It has no relationship to actual work, complexity, originality, or marketability.

---

## Exploit Scenarios

### Scenario A: Sparse-file inflation
```bash
dd if=/dev/zero of=fake_asset.rs bs=1 count=1 seek=100G
```
This creates a ~100 GB sparse file with almost no actual disk allocation. DollarFS would value it at ~$537M.

### Scenario B: Memory exhaustion
A 10 GB file in a watched directory causes the security scanner to attempt `read_to_string`, crashing the process.

---

## Mitigations Applied (v0.3.0)

| Control | Description |
|---------|-------------|
| **Cap minutes** | `MAX_MINUTES_PER_FILE = 480` (8 hours). No single file can claim more than 8 hours of estimated labor. |
| **Remove raw size component** | Deleted `file.size * 0.0001` from `book_value`. |
| **Sparse file detection** | Compares logical size vs allocated blocks via `std::os::unix::fs::MetadataExt`. Sparse files get **zero** collateral value. |
| **Entropy scoring** | Shannon entropy computed during streaming hash. Files with entropy < 1.0 bits/byte are penalized to zero. Files > 6.0 get a small bonus. |
| **Generated-file suppression** | Expanded `DEFAULT_EXCLUDED` to include `vendor`, `generated`, `*.min.js`, `*.lock`, `go.sum`, `Cargo.lock`, etc. |
| **Max file size** | `MAX_FILE_SIZE_BYTES = 1 GB`. Files larger than 1 GB are skipped during indexing. |
| **Streaming security scan** | Replaced `std::fs::read_to_string` with `BufReader` line-by-line reading + 100 MB max scan size. |
| **Schema versioning** | New `schema_version` and `is_legacy` fields on `valuations`. All pre-0.3.0 valuations are automatically marked `is_legacy = 1`. |

---

## New Valuation Doctrine

> **No collateral value without proof.**  
> **No proof without bounded scanning.**  
> **No valuation without complexity checks.**  
> **No liquidity claim without market/recovery evidence.**

A file's collateral value **cannot** be derived from size alone. Size may be a weak supporting signal, but only after **content validity, entropy, originality, complexity, ownership, build proof, and marketability** checks pass.

---

## Impact on Existing Ledgers

All valuations created before schema version 0.3.0 are marked **legacy / unsafe estimates** in the database. They remain visible for audit purposes but must **not** be used for collateralization, lending, or trading until rescanned under the corrected methodology.

---

## Credit

Report identified by external security researcher. DollarFS confirms the described class of issue is valid and treats it as critical because it affects both economic integrity and scanner availability.

---

## References

- Commit: `24bbbd1` and subsequent security patch commits
- `crates/dfs-valuation/src/lib.rs` — capped minutes, entropy bonus, sparse penalty
- `crates/dfs-security/src/lib.rs` — streaming scan with max size
- `crates/dfs-core/src/scanner.rs` — max file size, entropy, sparse detection
- `crates/dfs-core/src/db.rs` — schema migration, legacy marking
