# plane-cli

A command-line client and terminal UI for self-hosted [Plane](https://plane.so/), built for people, scripts and AI agents.

> plane-cli is an unofficial, community-built command-line client for the Plane project tracker. It is not affiliated with, sponsored by, or endorsed by Plane Software, Inc. "Plane" is a trademark of its owner and is used only to describe what this tool works with.

[![CI](https://github.com/pseudovar/plane-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/pseudovar/plane-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/pseudovar/plane-cli)](https://github.com/pseudovar/plane-cli/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Docs](https://img.shields.io/badge/docs-mdBook-informational)](https://pseudovar.github.io/plane-cli/)

## Example

```console
$ plane-cli work-items list --project ENG
{
  "next_cursor": null,
  "next_page_results": false,
  "results": [
    {
      "assignees": [],
      "id": "7e1c4a90-2b5f-4d83-9c26-e8a3f0b6d152",
      "name": "Fix login redirect",
      "priority": "high",
      "sequence_id": 42,
      ...
    }
  ],
  "total_results": 1
}
```

With `--json`, every response is one object on stdout with an `ok` field, ready for scripts:

```console
$ plane-cli --json work-items list --project ENG
{"ok":true,"data":{"next_cursor":null,"next_page_results":false,"results":[{"id":"7e1c4a90-2b5f-4d83-9c26-e8a3f0b6d152","name":"Fix login redirect","priority":"high","sequence_id":42}],"total_results":1}}

$ plane-cli --json work-items get --work-item ENG-999
{"ok":false,"error":{"code":"not_found","message":"..."}}
```

## Install

Linux x86_64 and macOS (arm64, x86_64). Needs `curl` or `wget`, `tar` and a SHA-256 tool:

```bash
curl -fsSL https://raw.githubusercontent.com/pseudovar/plane-cli/main/scripts/install.sh | bash
```

The script verifies the release checksum and installs to `~/.local/bin`. See the [install guide](https://pseudovar.github.io/plane-cli/install.html) for pinned versions and other options.

From source (Rust 1.86+):

```bash
cargo build --release -p plane-cli
```

## Setup

Create an API key in Plane (profile settings, *API tokens*), then:

```bash
plane-cli config init --global   # prompts for URL, key and workspace
plane-cli config doctor          # checks the settings and that the API answers
```

## Quick start

```bash
plane-cli projects list
plane-cli work-items list --project ENG --state "In Progress"
plane-cli work-items create --project ENG --name "Fix login redirect" --priority high
plane-cli work-items update --work-item ENG-42 --state "In Review"
plane-cli work-items comments create --work-item ENG-42 --comment-html '<p>Ready for review.</p>'
plane-cli tui
```

## Commands

| Command | Purpose |
|---------|---------|
| `config` | Setup wizard, diagnostics, key storage, write policy |
| `auth` | Check the URL, key and workspace |
| `projects` | Projects, features, members, context, summary, archive |
| `work-items` | Work items, comments, links, relations, attachments, activity |
| `inbox` | Work assigned to you, in batches |
| `cycles`, `modules` | Plan work and manage membership |
| `labels`, `states` | Project catalogs |
| `intake` | Intake queue |
| `members`, `invitations` | Workspace membership |
| `tui` | Full-screen terminal UI |
| `completions`, `complete` | Shell completion |
| `self` | Check for updates, update, install the agent skill |

Full list: [command reference](https://pseudovar.github.io/plane-cli/commands.html).

## Configuration and policy

Settings come from the environment or from env files, and `plane-cli config where` shows which file set what. Keys can live in the macOS Keychain or a protected key file instead of an env file. See [authentication and configuration](https://pseudovar.github.io/plane-cli/configuration.html).

A per-project [write policy](https://pseudovar.github.io/plane-cli/policy.html) can require fields, limit who sets a state or deletes, and refuse a write before it is sent:

```bash
plane-cli config policy init --project ENG --owner alice --require priority
```

## JSON contract and exit codes

| Exit code | Meaning |
|-----------|---------|
| `0` | Success |
| `1` | HTTP or transport failure, or another runtime error |
| `2` | Usage or validation error, including a policy refusal |

Read `ok` and branch on `error.code`. The codes are listed in [JSON output and exit codes](https://pseudovar.github.io/plane-cli/agent-contract.html).

## Using with AI agents

plane-cli ships a skill that teaches agents its commands and safe write habits:

```bash
plane-cli self skills install --local
```

Each agent can act as its own Plane member, so the activity log shows who did what:

```bash
plane-cli config set-key --member ci-bot
plane-cli --json --as ci-bot inbox
```

See [Agents and `--as`](https://pseudovar.github.io/plane-cli/agent-identity.html) and [Inbox](https://pseudovar.github.io/plane-cli/inbox.html).

## Telemetry

Error telemetry is **off by default**. It runs only if you set `PLANE_CLI_ANON_TELEMETRY=1` on a release binary. Source builds have no endpoint and send nothing.

- **Sent when on:** command names, durations, error codes, HTTP status and method, API paths with all names and ids removed, version, OS details, and panic stack traces (function names and build-machine source paths).
- **Never sent:** API keys, your Plane hostname or URL, workspace data (names, emails, ids, content), local paths, HTTP response bodies, or your machine's hostname.
- **Turn it off:** unset the variable or set it to `0`.

The [telemetry page](https://pseudovar.github.io/plane-cli/telemetry.html) lists exactly what is sent.

## Compatibility

Tested on **Plane Community Edition 1.4.2**. Some limits come from Plane's public API:

- Relations cannot be removed through the API.
- There is no notifications endpoint, so `inbox` cannot include @mentions.
- Assignee filtering happens in the client, up to 5,000 work items per project.
- Members added in the Plane UI cannot be changed through the API, and removed members cannot be re-added through it.

See [compatibility and API limits](https://pseudovar.github.io/plane-cli/compatibility.html).

## Documentation

The full docs are at **<https://pseudovar.github.io/plane-cli/>**.

## Development

```bash
cargo build -p plane-cli
./scripts/check.sh   # format, lint and test
```

## Contributing and support

- Read [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).
- Questions: [Discussions](https://github.com/pseudovar/plane-cli/discussions/categories/q-a).
- Bugs and features: [issues](https://github.com/pseudovar/plane-cli/issues/new/choose).
- Security: [SECURITY.md](SECURITY.md).

## License and trademark

MIT. See [LICENSE](LICENSE).

plane-cli is an unofficial, community-built command-line client for the Plane project tracker. It is not affiliated with, sponsored by, or endorsed by Plane Software, Inc. "Plane" is a trademark of its owner and is used only to describe what this tool works with.
