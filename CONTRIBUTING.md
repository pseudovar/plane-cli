# Contributing

Thanks for helping improve plane-cli.

## Before you start

- Small fixes: open a pull request.
- Larger changes (new commands, changed JSON output, new dependencies): [open an issue](https://github.com/pseudovar/plane-cli/issues/new/choose) first so we can agree on the approach.
- Questions: use [Discussions](https://github.com/pseudovar/plane-cli/discussions/categories/q-a).
- Security problems: follow [SECURITY.md](https://github.com/pseudovar/plane-cli/blob/main/SECURITY.md). Do not open a public issue.

## Build and test

You need Rust 1.86 or newer.

```bash
cargo build -p plane-cli
./scripts/check.sh    # cargo fmt --check, clippy, and tests
```

To run the checks before every push:

```bash
git config core.hooksPath .githooks
```

Tests do not need a Plane server. If you test against a live instance, use a throwaway project.

If you change a command or its help text, regenerate the command reference:

```bash
cargo build -p plane-cli
./scripts/gen-command-reference.sh
```

To preview the docs, install [mdBook](https://rust-lang.github.io/mdBook/) and run `mdbook serve docs`.

## Pull requests

- Keep each pull request to one change, and add or update tests.
- The `--json` output and exit codes are a contract for scripts and agents. Call out any change to them.
- Update the docs when behaviour changes, and add a line to `CHANGELOG.md`.
- Never include real API keys, hostnames or workspace data in code, tests, logs or screenshots. Use `plane.example.com`.

## Releases (maintainers)

After CI passes on `main`, `auto-tag.yml` compares the tested commit's crate version with the existing tags. If the version is newer than the latest tag, it tags that commit and dispatches `release.yml`. With no tags at all it does nothing, so the **first release is a manual step**:

1. Optional: add the `PLANE_CLI_SENTRY_DSN` repository secret. Without it, the release works and telemetry is inactive.
2. Confirm CI is green on `main` and both crates have the intended version.
3. Tag it: `git tag -a v0.3.0 -m "Release v0.3.0" && git push origin v0.3.0`.

For later releases, run `./scripts/release.sh patch` (or `minor`, `major`, or a version).

## AI-assisted contributions

AI tools are welcome. You are responsible for what you submit: you must understand the change, be able to explain it, and have run it and its tests yourself. Pull requests that show otherwise may be closed.

## License

By contributing, you agree that your contribution is licensed under the [MIT License](https://github.com/pseudovar/plane-cli/blob/main/LICENSE).
