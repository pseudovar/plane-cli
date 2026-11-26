# Quick start

You need a Plane instance URL, a workspace slug and an API key. Create the key in Plane under your profile settings, in *API tokens*.

```bash
plane-cli config init --global   # prompts for URL, key and workspace
plane-cli config doctor          # checks the settings and that the API answers
plane-cli auth whoami
```

Look around:

```bash
plane-cli projects list
plane-cli work-items list --project ENG --state "In Progress"
plane-cli work-items get --work-item ENG-42
plane-cli tui
```

Make changes:

```bash
plane-cli work-items create --project ENG --name "Fix login redirect" --priority high
plane-cli work-items update --work-item ENG-42 --state "In Review"
plane-cli work-items comments create --work-item ENG-42 --comment-html '<p>Ready for review.</p>'
```

Add `--json` for machine-readable output:

```bash
plane-cli --json work-items get --work-item ENG-42
```

Every JSON response has an `ok` field. On failure, `error.code` says what went wrong. See [JSON output and exit codes](agent-contract.md).
