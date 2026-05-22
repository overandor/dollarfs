# VALUATION MODEL

## Truth Label

**All dollar values in lfv are internal estimates. They are not guaranteed market prices, resale values, investment returns, or liquidation values.**

## Purpose

Valuation provides:
- Work accountability
- Prioritization signal
- R&D asset tracking
- Internal cost awareness

## Baseline Formula

```
file_value =
  base_file_type_value
+ estimated_work_value
+ complexity_value
+ rnd_bonus
- duplicate_penalty
- security_penalty
- uncertainty_discount
```

## Components

### base_file_type_value

Source code files get higher base values than media or cache files.

| Type | Base Multiplier |
|------|----------------|
| Rust, C, C++, Swift | 1.5x |
| Python, JS, TS, Go | 1.2x |
| Markdown, docs | 1.0x |
| Config, JSON, YAML | 0.8x |
| Media, binaries | 0.1x |

### estimated_work_value

Rough heuristic: `size_bytes / 500.0` minutes of work at `hourly_rate_usd`.

This is intentionally approximate. It captures "this file took effort to create" without claiming precision.

### complexity_value

Higher for files with complex extensions (systems languages > scripting).

### rnd_bonus

If file path contains R&D keywords (membra, semantic, protocol, runtime, etc.), apply `rnd_multiplier`.

### Penalties

- **Duplicate**: file is a duplicate → multiply by `duplicate_penalty` (default 0.1)
- **Security**: file contains secrets → multiply by `security_penalty` (default 0.5)
- **Uncertainty**: low confidence in file type → multiply by `unknown_confidence_discount` (default 0.6)

## Confidence

Each valuation has a confidence score (0.0 to 1.0):

- High (≥0.8): known file type, no security issues, good provenance
- Medium (0.5–0.79): partial information
- Low (<0.5): unknown or uncertain

## Configurable Defaults

Stored in `~/.local_file_value/config.toml` (or settings table):

```toml
hourly_rate_usd = 150
llm_multiplier = 0.35
rnd_multiplier = 2.0
production_multiplier = 2.5
documentation_multiplier = 1.2
test_multiplier = 1.3
security_penalty = 0.5
duplicate_penalty = 0.1
unknown_confidence_discount = 0.6
```

## Output Example

```
Path: ./app.py
Book value: $1,250
Confidence: Medium
Reason: production-facing Python app, reusable, documented, no tests yet
Risks: no test suite, no security review
```

No hype. No fake precision. Rounded dollars with confidence labels.
