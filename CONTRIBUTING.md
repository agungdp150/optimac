# Contributing

Thanks for helping improve OptiMac.

## Development

```bash
asdf install
make security
make test
make build
```

## Pull Request Rules

Keep changes reviewable and scoped. For security-sensitive work, explain the risk model in the PR description.

Security-sensitive changes include:

- File deletion, trash, restore, or cleanup scope.
- Sudo/admin authorization.
- Shell command execution.
- LaunchAgents, LaunchDaemons, login items, or app uninstall behavior.
- Network requests, external URLs, downloads, or generated assets.
- Anything that prompts for secrets or credentials.

## Blocked Contribution Patterns

The security scan is expected to fail if a PR adds:

- Unapproved external URLs.
- Remote downloads piped directly into a shell.
- Hidden Go HTTP clients or websocket/grpc clients.
- `sh -c`, `bash -c`, or `zsh -c` execution.
- Password/token input forms in Markdown, HTML, or SVG assets.
- New administrator AppleScript prompts outside reviewed files.

If a legitimate change needs one of these patterns, open an issue first and document why it is necessary, what data it touches, and how users stay in control.

## Safety Expectations

- Destructive commands must preview by default.
- Permanent deletion must be clearly labeled.
- Trash-backed operations should log an operation ID.
- Protected paths must stay protected.
- Tests should cover both success and refusal cases.
