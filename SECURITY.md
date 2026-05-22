# SECURITY

## Secret Handling

lfv detects secrets using local regex patterns. It **never** uploads secrets.

## Rules

1. Secrets are detected locally only.
2. Secret-containing files are **not** sent to LLMs.
3. Terminal output shows **redacted previews** only.
4. Full secrets are never printed.
5. Secret findings are stored in local SQLite only.

## Detected Patterns

- AWS keys
- GitHub tokens
- Private keys (RSA, EC, DSA, OpenSSH)
- API keys (generic)
- Bearer tokens
- JWT tokens
- Database connection strings with credentials
- Ethereum private keys
- Bitcoin WIF
- OpenAI / Stripe API keys
- Google API keys
- Discord bot tokens
- URL-embedded passwords

## Output Format

```
HIGH RISK: possible API key in ./app.py
Preview: sk-****abcd
Action: remove secret and use .env
```

## Quarantine

Future versions may support `lfv quarantine <file>` to move secret-bearing files to an encrypted local vault.

## Audit

All security findings are logged in the `security_findings` table with:
- file_id
- finding_type
- line_number
- severity
- detected_at

## Responsibility

lfv is a detection tool, not a remediation service. Users must act on findings.
