# Security Policy

OptiMac is a local macOS maintenance tool. Because it can remove files, request administrator authorization, and inspect local system state, security-sensitive changes need extra review.

## Supported Version

| Version | Supported |
| --- | --- |
| `0.1.x` | Yes |

## Reporting a Vulnerability

Please do not open a public issue for a vulnerability that could lead to data loss, privilege abuse, credential exposure, or remote code execution.

Report privately by emailing the maintainer or opening a GitHub private vulnerability report if the repository has that feature enabled. Include:

- Affected command or TUI flow.
- Steps to reproduce.
- Expected impact.
- Whether the issue needs administrator privileges.
- Any logs or screenshots that do not contain secrets.

## Security Boundaries

OptiMac should not:

- Send local file paths, app names, system identifiers, credentials, or scan results to external services.
- Add external links unless they are project-owned or explicitly allowlisted.
- Download and execute remote code.
- Hide network calls in cleanup, status, uninstall, or optimization flows.
- Prompt for passwords outside macOS authorization prompts.
- Broaden delete scope when a path is ambiguous.
- Delete protected paths such as `/`, `/System`, `/Library`, `/Applications`, `/Users`, the home directory itself, or key user folders.

## Automated Guardrails

The repository includes `scripts/security-scan.sh` and a GitHub Actions workflow that blocks common high-risk contribution patterns:

- Unapproved external URLs.
- `curl` / `wget` pipe-to-shell patterns.
- New shell execution through `sh -c`, `bash -c`, or `zsh -c`.
- New Go network client APIs without review.
- Unreviewed AppleScript administrator prompts.
- HTML/SVG/Markdown credential-capture patterns.

These checks are not a replacement for review. They are a tripwire for suspicious changes.

Run locally:

```bash
make security
```

## Reviewer Checklist

For PRs that touch deletion, sudo/admin authorization, app uninstall, shell commands, launch agents, network behavior, or generated assets:

- Confirm the feature previews before deleting.
- Confirm protected-path validation is still enforced.
- Confirm trash-backed behavior or permanent-delete wording is explicit.
- Confirm the PR does not add telemetry, tracking pixels, credential forms, or unapproved outbound requests.
- Confirm tests cover failure and protected-path cases.
