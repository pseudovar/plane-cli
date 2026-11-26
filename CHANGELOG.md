# Changelog

All notable changes to this project are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-11-26

First public release.

### Added

- Commands for projects, work items, comments, links, relations, attachments, activities, cycles, modules, labels, states, intake, workspace and project members, and invitations.
- A full-screen terminal UI for browsing and editing work items.
- A stable `--json` envelope with documented error codes and exit codes.
- `--as <member>` to act as a Plane member with that member's own key, stored in the macOS Keychain or a protected key file.
- Per-project write policies with `--dry-run`, enforced before a write is sent.
- `inbox`, a capped list of work assigned to a member, for scheduled agents.
- `projects context`, a single preflight snapshot of a project.
- Shell completions, including catalog-aware completion.
- An agent skill, installable with `plane-cli self skills install`.
- Documentation site, contributor guides, issue forms and dependency checks.

### Changed

- `install.sh`, `self check` and `self update` use public GitHub URLs and no longer need the GitHub CLI.
- `install.sh` refuses to install when a checksum is missing or does not match.
- The telemetry endpoint is set at build time. Source builds have none and send nothing.

### Security

- Telemetry no longer reports the machine's hostname or the text of TUI errors.
- Updated `h2`, `rustls`, `rustls-webpki`, `url` and `webpki-roots` to fix published advisories.

[Unreleased]: https://github.com/pseudovar/plane-cli/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/pseudovar/plane-cli/releases/tag/v0.3.0
