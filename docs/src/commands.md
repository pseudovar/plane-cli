# Command reference

Generated from `plane-cli --help` by `scripts/gen-command-reference.sh`. Do not edit by hand.

## `plane-cli`

```text
CLI for Plane (self-hosted). Set PLANE_API_BASE_URL, PLANE_API_KEY, PLANE_WORKSPACE_SLUG.

Usage: plane-cli [OPTIONS] <COMMAND>

Commands:
  config       Configure local env file and inspect required PLANE_* settings
  auth         Verify API URL, key, and workspace (GET /api/v1/users/me/)
  projects     Workspace projects
  work-items   Work items and nested resources
  inbox        Work items assigned to you (or the `--as` member) that are not Done or Cancelled, ordered by priority then age and capped for one agent turn (inbox fetch)
  modules      Project modules
  cycles       Project cycles and the work items in them
  labels       Project labels (create, rename, recolour, delete)
  states       Project workflow states (group, colour, sequence, default)
  intake       Project intake queue (turn on with `projects features --intake on`)
  members      Workspace members (project members: `projects members`)
  invitations  Workspace invitations
  tui          Browse and edit projects and work items in a full-screen terminal UI
  completions  Print shell completion script to stdout, or install into conventional paths (`--install`)
  complete     Print completion candidates (one per line). Use with `completions --catalog-hooks` or your shell's compgen. [aliases: completion]
  self         Compare this build to GitHub releases and re-run the installer (public GitHub URLs; no Plane API config)
  help         Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
  -V, --version          Print version
```

## `plane-cli config`

```text
Configure local env file and inspect required PLANE_* settings

Usage: plane-cli config [OPTIONS] <COMMAND>

Commands:
  init     Interactive wizard to write a local env file (defaults to ./.plane-cli.env)
  update   Interactive wizard to update an existing env file (Enter keeps each current value)
  doctor   Show whether required PLANE_* variables are present and whether the API answers
  where    Show config file paths and which env layers exist (no secret values)
  set-key  Store a Plane member's API key for `--as` without echoing it: macOS Keychain item `plane-cli-<member>`, elsewhere `~/.config/plane-cli/keys/<member>` (mode 600). Reads the key from a hidden TTY prompt, or from stdin when stdin is not a terminal
  policy   Per-project write policies enforced on work-item writes (`init`, `show`)
  help     Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli config init`

```text
Interactive wizard to write a local env file (defaults to ./.plane-cli.env)

Usage: plane-cli config init [OPTIONS]

Options:
      --env-file <PATH>  Output env file path (default: ./.plane-cli.env, or user config dir with --global)
      --json             Machine-readable JSON on stdout
      --global           Write ~/.config/plane-cli/.plane-cli.env (or XDG_CONFIG_HOME / %APPDATA%\plane-cli on Windows); cannot combine with `config init --env-file PATH`
      --refresh          Bypass cache reads for catalog-backed commands
      --overwrite        Overwrite env file if it already exists
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli config update`

```text
Interactive wizard to update an existing env file (Enter keeps each current value)

Usage: plane-cli config update [OPTIONS]

Options:
      --env-file <PATH>  Env file path (default: ./.plane-cli.env, or user config dir with --global)
      --json             Machine-readable JSON on stdout
      --global           Edit ~/.config/plane-cli/.plane-cli.env (or XDG_CONFIG_HOME / %APPDATA%\plane-cli on Windows); cannot combine with `config update --env-file PATH`
      --refresh          Bypass cache reads for catalog-backed commands
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli config doctor`

```text
Show whether required PLANE_* variables are present and whether the API answers

Usage: plane-cli config doctor [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --offline          Skip the reachability check (one GET /api/v1/users/me/ with a 5 s timeout)
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli config where`

```text
Show config file paths and which env layers exist (no secret values)

Usage: plane-cli config where [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli config set-key`

```text
Store a Plane member's API key for `--as` without echoing it: macOS Keychain item `plane-cli-<member>`, elsewhere `~/.config/plane-cli/keys/<member>` (mode 600). Reads the key from a hidden TTY prompt, or from stdin when stdin is not a terminal

Usage: plane-cli config set-key [OPTIONS] --member <MEMBER>

Options:
      --json             Machine-readable JSON on stdout
      --member <MEMBER>  Plane member name (e.g. bob)
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli config policy`

```text
Per-project write policies enforced on work-item writes (`init`, `show`)

Usage: plane-cli config policy [OPTIONS] <COMMAND>

Commands:
  init  Write a starter policy for a project (search first, description sections, required fields, member rules)
  show  Show the active policy for a project and the paths searched
  help  Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli config policy init`

```text
Write a starter policy for a project (search first, description sections, required fields, member rules)

Usage: plane-cli config policy init [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. WEB); the file is <PROJECT>.toml
      --owner <MEMBER>     Member allowed to set Todo/Done/Cancelled and to delete (repeatable); without it those rules are written commented out
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --require <FIELD>    Field work-items create must set (repeatable): priority, module, labels, assignees, state, start_date, target_date; default priority
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --label <NAME>       Allowed label name (repeatable); without it any label is allowed
      --local              Write ./.plane-cli/policy/<PROJECT>.toml (repo-local override) instead of the user config dir
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --overwrite          Replace an existing policy file
  -h, --help               Print help
```

#### `plane-cli config policy show`

```text
Show the active policy for a project and the paths searched

Usage: plane-cli config policy show [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. WEB)
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

## `plane-cli auth`

```text
Verify API URL, key, and workspace (GET /api/v1/users/me/)

Usage: plane-cli auth [OPTIONS] <COMMAND>

Commands:
  whoami  
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli auth whoami`

```text
Usage: plane-cli auth whoami [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

## `plane-cli projects`

```text
Workspace projects

Usage: plane-cli projects [OPTIONS] <COMMAND>

Commands:
  list       List projects (updates local cache snapshot)
  get        
  context    Agent snapshot: capabilities, grouped states, labels, modules, hints (--json required)
  create     
  update     
  delete     
  archive    Archive a project: hides its work items, states, cycles and modules until unarchived
  unarchive  Restore an archived project
  summary    Counts of work items, cycles, modules, members, states, labels, intake items and pages
  features   Show or switch features: --cycles, --modules, --pages, --intake, --views (on|off)
  members    Project members (add, change role, remove)
  help       Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli projects list`

```text
List projects (updates local cache snapshot)

Usage: plane-cli projects list [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --page-size <N>    
      --cursor <CURSOR>  
      --refresh          Bypass cache reads for catalog-backed commands
      --all              
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli projects get`

```text
Usage: plane-cli projects get [OPTIONS] --id <PROJECT>

Options:
      --id <PROJECT>     Project identifier or UUID
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli projects context`

```text
Agent snapshot: capabilities, grouped states, labels, modules, hints (--json required)

Usage: plane-cli projects context [OPTIONS] --id <PROJECT>

Options:
      --id <PROJECT>     Project identifier or UUID
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli projects create`

```text
Usage: plane-cli projects create [OPTIONS]

Options:
      --body <JSON>                  
      --json                         Machine-readable JSON on stdout
      --name <NAME>                  
      --refresh                      Bypass cache reads for catalog-backed commands
      --description <DESCRIPTION>    
      --env-file <PATH>              Load environment variables from this file before reading config
      --dotenv                       Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --identifier <SHORT_ID>        
      --project-lead <PROJECT_LEAD>  
      --strict-catalog               For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                  Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help                         Print help
```

### `plane-cli projects update`

```text
Usage: plane-cli projects update [OPTIONS] --id <PROJECT>

Options:
      --id <PROJECT>                 Project identifier or UUID
      --json                         Machine-readable JSON on stdout
      --body <JSON>                  
      --refresh                      Bypass cache reads for catalog-backed commands
      --env-file <PATH>              Load environment variables from this file before reading config
      --name <NAME>                  
      --description <DESCRIPTION>    
      --dotenv                       Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --identifier <SHORT_ID>        
      --strict-catalog               For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                  Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --project-lead <PROJECT_LEAD>  
  -h, --help                         Print help
```

### `plane-cli projects delete`

```text
Usage: plane-cli projects delete [OPTIONS] --id <PROJECT>

Options:
      --id <PROJECT>     Project identifier or UUID
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli projects archive`

```text
Archive a project: hides its work items, states, cycles and modules until unarchived

Usage: plane-cli projects archive [OPTIONS] --id <PROJECT>

Options:
      --id <PROJECT>     Project identifier or UUID
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli projects unarchive`

```text
Restore an archived project

Usage: plane-cli projects unarchive [OPTIONS] --id <PROJECT>

Options:
      --id <PROJECT>     Project identifier or UUID
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli projects summary`

```text
Counts of work items, cycles, modules, members, states, labels, intake items and pages

Usage: plane-cli projects summary [OPTIONS] --id <PROJECT>

Options:
      --id <PROJECT>     Project identifier or UUID
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli projects features`

```text
Show or switch features: --cycles, --modules, --pages, --intake, --views (on|off)

Usage: plane-cli projects features [OPTIONS] --id <PROJECT>

Options:
      --id <PROJECT>      Project identifier or UUID
      --json              Machine-readable JSON on stdout
      --cycles <on|off>   Cycles (`cycle_view`)
      --refresh           Bypass cache reads for catalog-backed commands
      --env-file <PATH>   Load environment variables from this file before reading config
      --modules <on|off>  Modules (`module_view`)
      --dotenv            Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --pages <on|off>    Pages (`page_view`)
      --intake <on|off>   Intake (`intake_view`)
      --strict-catalog    For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>       Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --views <on|off>    Saved views (`issue_views_view`)
  -h, --help              Print help
```

### `plane-cli projects members`

```text
Project members (add, change role, remove)

Usage: plane-cli projects members [OPTIONS] <COMMAND>

Commands:
  list    Project members with role and `is_active` (row `id` is the user id)
  add     Add a workspace member to the project; the result `id` is the membership id
  update  Change a member's project role
  remove  Remove a member (Plane deactivates the membership; the API cannot re-add them)
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli projects members list`

```text
Project members with role and `is_active` (row `id` is the user id)

Usage: plane-cli projects members list [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --include-inactive   Also list removed (inactive) members
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --page-size <N>      
      --cursor <CURSOR>    
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --all                
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli projects members add`

```text
Add a workspace member to the project; the result `id` is the membership id

Usage: plane-cli projects members add [OPTIONS] --project <PROJECT> --member <USER>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --member <USER>      Workspace member: user UUID, email or display name
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --role <ROLE>        admin, member or guest [default: member]
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli projects members update`

```text
Change a member's project role

Usage: plane-cli projects members update [OPTIONS] --project <PROJECT> --membership <MEMBERSHIP_UUID> --role <ROLE>

Options:
      --json                          Machine-readable JSON on stdout
      --project <PROJECT>             Project identifier (e.g. ENG) or UUID
      --membership <MEMBERSHIP_UUID>  Membership id returned by `projects members add` (not the user id) [aliases: --id]
      --refresh                       Bypass cache reads for catalog-backed commands
      --env-file <PATH>               Load environment variables from this file before reading config
      --role <ROLE>                   admin, member or guest
      --dotenv                        Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog                For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                   Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help                          Print help
```

#### `plane-cli projects members remove`

```text
Remove a member (Plane deactivates the membership; the API cannot re-add them)

Usage: plane-cli projects members remove [OPTIONS] --project <PROJECT> --membership <MEMBERSHIP_UUID>

Options:
      --json                          Machine-readable JSON on stdout
      --project <PROJECT>             Project identifier (e.g. ENG) or UUID
      --membership <MEMBERSHIP_UUID>  Membership id returned by `projects members add` (not the user id) [aliases: --id]
      --refresh                       Bypass cache reads for catalog-backed commands
      --env-file <PATH>               Load environment variables from this file before reading config
      --dotenv                        Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog                For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                   Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help                          Print help
```

## `plane-cli work-items`

```text
Work items and nested resources

Usage: plane-cli work-items [OPTIONS] <COMMAND>

Commands:
  list         
  get          
  create       
  update       
  delete       
  search       
  activities   
  comments     
  links        
  relations    
  attachments  Files attached to a work item
  help         Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli work-items list`

```text
Usage: plane-cli work-items list [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --state <STATE>      Filter results by state name or UUID
      --assignee <UUID>    Filter results by assignee: user UUID, or workspace member display name or email (repeatable)
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --page-size <N>      
      --cursor <CURSOR>    
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --all                
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli work-items get`

```text
Usage: plane-cli work-items get [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID
      --project <PROJECT>  Required when ITEM is a bare UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli work-items create`

```text
Usage: plane-cli work-items create [OPTIONS] --project <PROJECT>

Options:
      --json                       Machine-readable JSON on stdout
      --project <PROJECT>          Project identifier (e.g. ENG) or UUID
      --body <JSON>                
      --refresh                    Bypass cache reads for catalog-backed commands
      --env-file <PATH>            Load environment variables from this file before reading config
      --name <NAME>                
      --description <DESCRIPTION>  
      --dotenv                     Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --priority <PRIORITY>        
      --strict-catalog             For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --state <STATE>              
      --assignee <UUID>            User UUID to assign (repeatable). Use --body '{"assignees":[]}' to clear.
      --module <MODULE>            Add the new work item to this module (UUID or exact name)
      --allow-duplicate            Create even when the project policy finds an open work item with a close name
      --dry-run                    Show the request and the policy checks without writing anything
  -h, --help                       Print help
```

### `plane-cli work-items update`

```text
Usage: plane-cli work-items update [OPTIONS]

Options:
      --json                       Machine-readable JSON on stdout
      --project <PROJECT>          Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh                    Bypass cache reads for catalog-backed commands
      --work-item <ITEM>           Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --body <JSON>                
      --env-file <PATH>            Load environment variables from this file before reading config
      --dotenv                     Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --name <NAME>                
      --description <DESCRIPTION>  
      --strict-catalog             For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --priority <PRIORITY>        
      --state <STATE>              
      --assignee <UUID>            User UUID to assign (repeatable). Use --body '{"assignees":[]}' to clear.
      --reason <TEXT>              Why: posted as a comment after the update (plain text); the project policy can require it for state, assignee or priority changes
      --dry-run                    Show the request, the comment and the policy checks without writing anything
  -h, --help                       Print help
```

### `plane-cli work-items delete`

```text
Usage: plane-cli work-items delete [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --dry-run            Show the request and the policy checks without deleting
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli work-items search`

```text
Usage: plane-cli work-items search [OPTIONS] --query <QUERY>

Options:
      --json               Machine-readable JSON on stdout
      --query <QUERY>      
      --project <PROJECT>  Limit search to one project (identifier or UUID)
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --limit <LIMIT>      
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli work-items activities`

```text
Usage: plane-cli work-items activities [OPTIONS] <COMMAND>

Commands:
  list  
  get   
  help  Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli work-items activities list`

```text
Usage: plane-cli work-items activities list [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --page-size <N>      
      --cursor <CURSOR>    
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --all                
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items activities get`

```text
Usage: plane-cli work-items activities get [OPTIONS] --id <UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --id <UUID>          Resource UUID
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli work-items comments`

```text
Usage: plane-cli work-items comments [OPTIONS] <COMMAND>

Commands:
  list    
  get     
  create  
  update  
  delete  
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli work-items comments list`

```text
Usage: plane-cli work-items comments list [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --page-size <N>      
      --cursor <CURSOR>    
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --all                
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items comments get`

```text
Usage: plane-cli work-items comments get [OPTIONS] --id <UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --id <UUID>          Resource UUID
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items comments create`

```text
Usage: plane-cli work-items comments create [OPTIONS]

Options:
      --json                         Machine-readable JSON on stdout
      --project <PROJECT>            Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh                      Bypass cache reads for catalog-backed commands
      --work-item <ITEM>             Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --body <JSON>                  
      --env-file <PATH>              Load environment variables from this file before reading config
      --comment-html <COMMENT_HTML>  
      --dotenv                       Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog               For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                  Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help                         Print help
```

#### `plane-cli work-items comments update`

```text
Usage: plane-cli work-items comments update [OPTIONS] --id <UUID>

Options:
      --json                         Machine-readable JSON on stdout
      --project <PROJECT>            Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh                      Bypass cache reads for catalog-backed commands
      --work-item <ITEM>             Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>              Load environment variables from this file before reading config
      --id <UUID>                    Resource UUID
      --body <JSON>                  
      --dotenv                       Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --comment-html <COMMENT_HTML>  
      --strict-catalog               For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                  Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help                         Print help
```

#### `plane-cli work-items comments delete`

```text
Usage: plane-cli work-items comments delete [OPTIONS] --id <UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --id <UUID>          Resource UUID
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli work-items links`

```text
Usage: plane-cli work-items links [OPTIONS] <COMMAND>

Commands:
  list    
  get     
  create  
  update  
  delete  
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli work-items links list`

```text
Usage: plane-cli work-items links list [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --page-size <N>      
      --cursor <CURSOR>    
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --all                
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items links get`

```text
Usage: plane-cli work-items links get [OPTIONS] --id <UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --id <UUID>          Resource UUID
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items links create`

```text
Usage: plane-cli work-items links create [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --body <JSON>        
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --url <URL>          
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --title <TITLE>      
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items links update`

```text
Usage: plane-cli work-items links update [OPTIONS] --id <UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --id <UUID>          Resource UUID
      --body <JSON>        
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --url <URL>          
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --title <TITLE>      
  -h, --help               Print help
```

#### `plane-cli work-items links delete`

```text
Usage: plane-cli work-items links delete [OPTIONS] --id <UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --id <UUID>          Resource UUID
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli work-items relations`

```text
Usage: plane-cli work-items relations [OPTIONS] <COMMAND>

Commands:
  list    
  create  
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli work-items relations list`

```text
Usage: plane-cli work-items relations list [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items relations create`

```text
Usage: plane-cli work-items relations create [OPTIONS]

Options:
      --json                           Machine-readable JSON on stdout
      --project <PROJECT>              Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh                        Bypass cache reads for catalog-backed commands
      --work-item <ITEM>               Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --body <JSON>                    
      --env-file <PATH>                Load environment variables from this file before reading config
      --dotenv                         Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --relation-type <RELATION_TYPE>  
      --issues <ISSUE_UUID>            Issue UUIDs, comma-separated
      --strict-catalog                 For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>                    Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help                           Print help
```

### `plane-cli work-items attachments`

```text
Files attached to a work item

Usage: plane-cli work-items attachments [OPTIONS] <COMMAND>

Commands:
  list    Attachments on a work item
  get     One attachment's metadata and a short-lived `download_url`; `--output` saves the file
  create  Upload a file and attach it
  delete  Delete an attachment
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli work-items attachments list`

```text
Attachments on a work item

Usage: plane-cli work-items attachments list [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items attachments get`

```text
One attachment's metadata and a short-lived `download_url`; `--output` saves the file

Usage: plane-cli work-items attachments get [OPTIONS] --id <UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --id <UUID>          Attachment id
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --output <PATH>      Download the file to PATH (refuses to overwrite)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items attachments create`

```text
Upload a file and attach it

Usage: plane-cli work-items attachments create [OPTIONS] --file <PATH>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --file <PATH>        
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --name <NAME>        File name to show in Plane (default: PATH's name)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --type <MIME>        MIME type (default: guessed from the file name)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli work-items attachments delete`

```text
Delete an attachment

Usage: plane-cli work-items attachments delete [OPTIONS] --id <UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID (UUID needs --project)
      --env-file <PATH>    Load environment variables from this file before reading config
      --id <UUID>          Attachment id
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

## `plane-cli inbox`

```text
Work items assigned to you (or the `--as` member) that are not Done or Cancelled, ordered by priority then age and capped for one agent turn (inbox fetch)

Usage: plane-cli inbox [OPTIONS]

Options:
      --json                  Machine-readable JSON on stdout
      --project <PROJECT>     Only this project (identifier or UUID); default: every project the member belongs to
      --limit <N>             Most items returned in `todo` (the batch cap) [default: 3]
      --refresh               Bypass cache reads for catalog-backed commands
      --blocked-limit <N>     Most items returned in `blocked_reassess` (0 skips the Blocked pass) [default: 1]
      --env-file <PATH>       Load environment variables from this file before reading config
      --dotenv                Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --handlers <PATH>       JSON file: per-project `states` eligible for `todo` and optional `blocked_state` ({"projects":{"OPS":{"states":[...]}}}); projects not listed use the defaults
      --include-backlog       Also treat the `backlog` state group as new work (default: only `unstarted`, e.g. Todo)
      --strict-catalog        For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>           Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --blocked-state <NAME>  State name of the Blocked pass (case-insensitive); a handlers-file `blocked_state` overrides it per project [default: Blocked]
  -h, --help                  Print help
```

## `plane-cli modules`

```text
Project modules

Usage: plane-cli modules [OPTIONS] <COMMAND>

Commands:
  list       
  get        
  create     
  update     
  delete     
  archive    Archive a module (Plane only archives completed or cancelled modules)
  unarchive  Restore an archived module
  issues     
  help       Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli modules list`

```text
Usage: plane-cli modules list [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --archived           List archived modules instead
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --page-size <N>      
      --cursor <CURSOR>    
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --all                
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli modules get`

```text
Usage: plane-cli modules get [OPTIONS] --project <PROJECT> --id <MODULE_UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --id <MODULE_UUID>   Module UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli modules create`

```text
Usage: plane-cli modules create [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --body <JSON>        
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli modules update`

```text
Usage: plane-cli modules update [OPTIONS] --project <PROJECT> --id <MODULE_UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --id <MODULE_UUID>   Module UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --body <JSON>        
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli modules delete`

```text
Usage: plane-cli modules delete [OPTIONS] --project <PROJECT> --id <MODULE_UUID>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --id <MODULE_UUID>   Module UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli modules archive`

```text
Archive a module (Plane only archives completed or cancelled modules)

Usage: plane-cli modules archive [OPTIONS] --project <PROJECT> --module <MODULE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --module <MODULE>    Module UUID or exact name [aliases: --id]
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli modules unarchive`

```text
Restore an archived module

Usage: plane-cli modules unarchive [OPTIONS] --project <PROJECT> --module <MODULE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --module <MODULE>    Module UUID or exact name [aliases: --id]
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli modules issues`

```text
Usage: plane-cli modules issues [OPTIONS] <COMMAND>

Commands:
  list    Work items in the module
  add     Add work items to the module
  remove  Remove work items from the module (one call per item; stops at the first failure)
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli modules issues list`

```text
Work items in the module

Usage: plane-cli modules issues list [OPTIONS] --project <PROJECT> --module <MODULE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --module <MODULE>    Module UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --page-size <N>      
      --cursor <CURSOR>    
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --all                
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli modules issues add`

```text
Add work items to the module

Usage: plane-cli modules issues add [OPTIONS] --project <PROJECT> --module <MODULE> --work-items <ITEM>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --module <MODULE>    Module UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --work-items <ITEM>  Composite ids (PROJ-123) or work item UUIDs, comma-separated
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli modules issues remove`

```text
Remove work items from the module (one call per item; stops at the first failure)

Usage: plane-cli modules issues remove [OPTIONS] --project <PROJECT> --module <MODULE> --work-items <ITEM>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --module <MODULE>    Module UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --work-items <ITEM>  Composite ids (PROJ-123) or work item UUIDs, comma-separated
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

## `plane-cli cycles`

```text
Project cycles and the work items in them

Usage: plane-cli cycles [OPTIONS] <COMMAND>

Commands:
  list       List cycles (adds a computed `status`: draft, upcoming, current, completed)
  get        Show one cycle (adds a computed `status`)
  create     Create a cycle. `--start` and `--end` are dates (YYYY-MM-DD), both or neither
  update     Update (rename, re-date, re-own) a cycle
  delete     Delete a cycle (no confirmation, like `modules delete`)
  archive    Archive a completed cycle
  unarchive  Restore an archived cycle
  transfer   Move the unfinished work items of a completed cycle to another cycle
  items      Work items in a cycle
  help       Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli cycles list`

```text
List cycles (adds a computed `status`: draft, upcoming, current, completed)

Usage: plane-cli cycles list [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --status <STATUS>    Filter by status (Plane `cycle_view`) [possible values: all, current, upcoming, completed, draft, incomplete]
      --archived           List archived cycles instead
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --page-size <N>      
      --cursor <CURSOR>    
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --all                
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli cycles get`

```text
Show one cycle (adds a computed `status`)

Usage: plane-cli cycles get [OPTIONS] --project <PROJECT> --cycle <CYCLE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>      Cycle UUID or exact name [aliases: --id]
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli cycles create`

```text
Create a cycle. `--start` and `--end` are dates (YYYY-MM-DD), both or neither

Usage: plane-cli cycles create [OPTIONS] --project <PROJECT>

Options:
      --json                Machine-readable JSON on stdout
      --project <PROJECT>   Project identifier (e.g. ENG) or UUID
      --name <NAME>         
      --refresh             Bypass cache reads for catalog-backed commands
      --description <TEXT>  
      --env-file <PATH>     Load environment variables from this file before reading config
      --dotenv              Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --start <YYYY-MM-DD>  Start date; Plane stores 00:00 in the project's time zone
      --end <YYYY-MM-DD>    End date; Plane stores 23:59 in the project's time zone
      --strict-catalog      For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>         Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --owner <USER_UUID>   Owner (`owned_by`) user UUID
      --body <PATH|->       
  -h, --help                Print help
```

### `plane-cli cycles update`

```text
Update (rename, re-date, re-own) a cycle

Usage: plane-cli cycles update [OPTIONS] --project <PROJECT> --cycle <CYCLE>

Options:
      --json                Machine-readable JSON on stdout
      --project <PROJECT>   Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>       Cycle UUID or exact name [aliases: --id]
      --refresh             Bypass cache reads for catalog-backed commands
      --env-file <PATH>     Load environment variables from this file before reading config
      --name <NAME>         
      --description <TEXT>  
      --dotenv              Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --start <YYYY-MM-DD>  Start date; Plane stores 00:00 in the project's time zone
      --strict-catalog      For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>         Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --end <YYYY-MM-DD>    End date; Plane stores 23:59 in the project's time zone
      --owner <USER_UUID>   Owner (`owned_by`) user UUID
      --body <PATH|->       
  -h, --help                Print help
```

### `plane-cli cycles delete`

```text
Delete a cycle (no confirmation, like `modules delete`)

Usage: plane-cli cycles delete [OPTIONS] --project <PROJECT> --cycle <CYCLE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>      Cycle UUID or exact name [aliases: --id]
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli cycles archive`

```text
Archive a completed cycle

Usage: plane-cli cycles archive [OPTIONS] --project <PROJECT> --cycle <CYCLE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>      Cycle UUID or exact name [aliases: --id]
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli cycles unarchive`

```text
Restore an archived cycle

Usage: plane-cli cycles unarchive [OPTIONS] --project <PROJECT> --cycle <CYCLE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>      Cycle UUID or exact name [aliases: --id]
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli cycles transfer`

```text
Move the unfinished work items of a completed cycle to another cycle

Usage: plane-cli cycles transfer [OPTIONS] --project <PROJECT> --cycle <CYCLE> --to <CYCLE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>      Source cycle UUID or exact name (must be completed)
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --to <CYCLE>         Target cycle UUID or exact name
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli cycles items`

```text
Work items in a cycle

Usage: plane-cli cycles items [OPTIONS] <COMMAND>

Commands:
  list    Work items in the cycle
  add     Add work items; an item already in another cycle is moved here
  remove  Remove work items from the cycle (one call per item; stops at the first failure)
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli cycles items list`

```text
Work items in the cycle

Usage: plane-cli cycles items list [OPTIONS] --project <PROJECT> --cycle <CYCLE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>      Cycle UUID or exact name
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --page-size <N>      
      --cursor <CURSOR>    
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --all                
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli cycles items add`

```text
Add work items; an item already in another cycle is moved here

Usage: plane-cli cycles items add [OPTIONS] --project <PROJECT> --cycle <CYCLE> --work-items <ITEM>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>      Cycle UUID or exact name
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --work-items <ITEM>  Composite ids (PROJ-123) or work item UUIDs, comma-separated
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

#### `plane-cli cycles items remove`

```text
Remove work items from the cycle (one call per item; stops at the first failure)

Usage: plane-cli cycles items remove [OPTIONS] --project <PROJECT> --cycle <CYCLE> --work-items <ITEM>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --cycle <CYCLE>      Cycle UUID or exact name
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --work-items <ITEM>  Composite ids (PROJ-123) or work item UUIDs, comma-separated
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

## `plane-cli labels`

```text
Project labels (create, rename, recolour, delete)

Usage: plane-cli labels [OPTIONS] <COMMAND>

Commands:
  list    List labels
  create  Create a label (`--name` required)
  update  Rename, recolour, re-parent or re-order a label
  delete  Delete a label (no confirmation; Plane removes it from work items)
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli labels list`

```text
List labels

Usage: plane-cli labels list [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --page-size <N>      
      --refresh            Bypass cache reads for catalog-backed commands
      --cursor <CURSOR>    
      --env-file <PATH>    Load environment variables from this file before reading config
      --all                
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli labels create`

```text
Create a label (`--name` required)

Usage: plane-cli labels create [OPTIONS] --project <PROJECT>

Options:
      --json                Machine-readable JSON on stdout
      --project <PROJECT>   Project identifier (e.g. ENG) or UUID
      --name <NAME>         
      --refresh             Bypass cache reads for catalog-backed commands
      --color <#RRGGBB>     Hex colour, e.g. #F59E0B
      --env-file <PATH>     Load environment variables from this file before reading config
      --description <TEXT>  
      --dotenv              Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --parent <LABEL>      Parent label UUID or exact name (nests this label under it)
      --strict-catalog      For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>         Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --sort-order <N>      Sort position (`sort_order`)
      --body <PATH|->       
  -h, --help                Print help
```

### `plane-cli labels update`

```text
Rename, recolour, re-parent or re-order a label

Usage: plane-cli labels update [OPTIONS] --project <PROJECT> --label <LABEL>

Options:
      --json                Machine-readable JSON on stdout
      --project <PROJECT>   Project identifier (e.g. ENG) or UUID
      --label <LABEL>       Label UUID or exact name [aliases: --id]
      --refresh             Bypass cache reads for catalog-backed commands
      --env-file <PATH>     Load environment variables from this file before reading config
      --name <NAME>         
      --color <#RRGGBB>     Hex colour, e.g. #F59E0B
      --dotenv              Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --description <TEXT>  
      --strict-catalog      For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>         Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --parent <LABEL>      Parent label UUID or exact name (nests this label under it)
      --sort-order <N>      Sort position (`sort_order`)
      --body <PATH|->       
  -h, --help                Print help
```

### `plane-cli labels delete`

```text
Delete a label (no confirmation; Plane removes it from work items)

Usage: plane-cli labels delete [OPTIONS] --project <PROJECT> --label <LABEL>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --label <LABEL>      Label UUID or exact name [aliases: --id]
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

## `plane-cli states`

```text
Project workflow states (group, colour, sequence, default)

Usage: plane-cli states [OPTIONS] <COMMAND>

Commands:
  list    List workflow states
  create  Create a state (`--name` and `--group` required)
  update  Rename, regroup, recolour, reorder a state or make it the default
  delete  Delete a state (Plane refuses the default state and states that still hold work items)
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli states list`

```text
List workflow states

Usage: plane-cli states list [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --page-size <N>      
      --refresh            Bypass cache reads for catalog-backed commands
      --cursor <CURSOR>    
      --env-file <PATH>    Load environment variables from this file before reading config
      --all                
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli states create`

```text
Create a state (`--name` and `--group` required)

Usage: plane-cli states create [OPTIONS] --project <PROJECT>

Options:
      --json                Machine-readable JSON on stdout
      --project <PROJECT>   Project identifier (e.g. ENG) or UUID
      --name <NAME>         
      --refresh             Bypass cache reads for catalog-backed commands
      --env-file <PATH>     Load environment variables from this file before reading config
      --group <GROUP>       State group (board column type) [possible values: backlog, unstarted, started, completed, cancelled, triage]
      --color <#RRGGBB>     Hex colour; create defaults to Plane's colour for the group
      --dotenv              Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --description <TEXT>  
      --strict-catalog      For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>         Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --sequence <N>        Position within the workflow (Plane's defaults step by 10000)
      --default             Make this the project's default state for new work items
      --body <PATH|->       
  -h, --help                Print help
```

### `plane-cli states update`

```text
Rename, regroup, recolour, reorder a state or make it the default

Usage: plane-cli states update [OPTIONS] --project <PROJECT> --state <STATE>

Options:
      --json                Machine-readable JSON on stdout
      --project <PROJECT>   Project identifier (e.g. ENG) or UUID
      --refresh             Bypass cache reads for catalog-backed commands
      --state <STATE>       State UUID or exact name [aliases: --id]
      --env-file <PATH>     Load environment variables from this file before reading config
      --name <NAME>         
      --dotenv              Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --group <GROUP>       State group (board column type) [possible values: backlog, unstarted, started, completed, cancelled, triage]
      --color <#RRGGBB>     Hex colour; create defaults to Plane's colour for the group
      --strict-catalog      For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>         Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --description <TEXT>  
      --sequence <N>        Position within the workflow (Plane's defaults step by 10000)
      --default             Make this the project's default state for new work items
      --body <PATH|->       
  -h, --help                Print help
```

### `plane-cli states delete`

```text
Delete a state (Plane refuses the default state and states that still hold work items)

Usage: plane-cli states delete [OPTIONS] --project <PROJECT> --state <STATE>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --state <STATE>      State UUID or exact name [aliases: --id]
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

## `plane-cli intake`

```text
Project intake queue (turn on with `projects features --intake on`)

Usage: plane-cli intake [OPTIONS] <COMMAND>

Commands:
  list    List intake work items (adds `status_name`)
  get     Show one intake work item
  create  Submit a work item to intake (`--name` required); it starts as pending
  update  Triage (accept, reject, snooze, mark duplicate) or edit an intake work item
  delete  Delete an intake work item (also deletes the work item unless it was accepted)
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli intake list`

```text
List intake work items (adds `status_name`)

Usage: plane-cli intake list [OPTIONS] --project <PROJECT>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --page-size <N>      
      --refresh            Bypass cache reads for catalog-backed commands
      --cursor <CURSOR>    
      --env-file <PATH>    Load environment variables from this file before reading config
      --all                
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli intake get`

```text
Show one intake work item

Usage: plane-cli intake get [OPTIONS] --project <PROJECT> --work-item <ITEM>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID of the intake item [aliases: --id]
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli intake create`

```text
Submit a work item to intake (`--name` required); it starts as pending

Usage: plane-cli intake create [OPTIONS] --project <PROJECT>

Options:
      --json                 Machine-readable JSON on stdout
      --project <PROJECT>    Project identifier (e.g. ENG) or UUID
      --name <TEXT>          Work item title
      --refresh              Bypass cache reads for catalog-backed commands
      --description <HTML>   Work item description (sent as `description_html`)
      --env-file <PATH>      Load environment variables from this file before reading config
      --dotenv               Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --priority <PRIORITY>  [possible values: urgent, high, medium, low, none]
      --body <PATH|->        JSON: `{"issue": {…}}` or the work item fields alone
      --strict-catalog       For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>          Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help                 Print help
```

### `plane-cli intake update`

```text
Triage (accept, reject, snooze, mark duplicate) or edit an intake work item

Usage: plane-cli intake update [OPTIONS] --project <PROJECT> --work-item <ITEM>

Options:
      --json                     Machine-readable JSON on stdout
      --project <PROJECT>        Project identifier (e.g. ENG) or UUID
      --refresh                  Bypass cache reads for catalog-backed commands
      --work-item <ITEM>         Composite id (PROJ-123) or work item UUID of the intake item [aliases: --id]
      --env-file <PATH>          Load environment variables from this file before reading config
      --status <STATUS>          [possible values: pending, rejected, snoozed, accepted, duplicate]
      --dotenv                   Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --snoozed-till <DATETIME>  With --status snoozed: when it comes back (ISO 8601, e.g. 2030-12-01T00:00:00Z)
      --duplicate-to <ITEM>      With --status duplicate: the work item it duplicates (PROJ-123 or UUID)
      --strict-catalog           For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>              Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
      --name <TEXT>              Work item title
      --description <HTML>       Work item description (sent as `description_html`)
      --priority <PRIORITY>      [possible values: urgent, high, medium, low, none]
      --body <PATH|->            
      --reason <TEXT>            Why: posted as a comment on the work item after the update; the project policy can require it for priority changes
  -h, --help                     Print help
```

### `plane-cli intake delete`

```text
Delete an intake work item (also deletes the work item unless it was accepted)

Usage: plane-cli intake delete [OPTIONS] --project <PROJECT> --work-item <ITEM>

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier (e.g. ENG) or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --work-item <ITEM>   Composite id (PROJ-123) or work item UUID of the intake item [aliases: --id]
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

## `plane-cli members`

```text
Workspace members (project members: `projects members`)

Usage: plane-cli members [OPTIONS] <COMMAND>

Commands:
  list  Workspace members with their workspace role (adds `role_name`)
  help  Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli members list`

```text
Workspace members with their workspace role (adds `role_name`)

Usage: plane-cli members list [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

## `plane-cli invitations`

```text
Workspace invitations

Usage: plane-cli invitations [OPTIONS] <COMMAND>

Commands:
  list    Pending workspace invitations
  create  Invite an email address to the workspace (Plane sends the email)
  update  Change the role of a pending invitation
  delete  Withdraw an invitation
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli invitations list`

```text
Pending workspace invitations

Usage: plane-cli invitations list [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli invitations create`

```text
Invite an email address to the workspace (Plane sends the email)

Usage: plane-cli invitations create [OPTIONS] --email <EMAIL>

Options:
      --email <EMAIL>    
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --role <ROLE>      admin, member or guest [default: member]
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli invitations update`

```text
Change the role of a pending invitation

Usage: plane-cli invitations update [OPTIONS] --id <UUID> --role <ROLE>

Options:
      --id <UUID>        Invitation id from `invitations list`
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --role <ROLE>      admin, member or guest
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli invitations delete`

```text
Withdraw an invitation

Usage: plane-cli invitations delete [OPTIONS] --id <UUID>

Options:
      --id <UUID>        Invitation id from `invitations list`
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

## `plane-cli tui`

```text
Browse and edit projects and work items in a full-screen terminal UI

Usage: plane-cli tui [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

## `plane-cli completions`

```text
Print shell completion script to stdout, or install into conventional paths (`--install`)

Usage: plane-cli completions [OPTIONS] [SHELL]

Arguments:
  [SHELL]  bash, zsh, fish, elvish, powershell — omit to infer from `$SHELL` (falls back to bash) [possible values: bash, elvish, fish, powershell, zsh]

Options:
      --json             Machine-readable JSON on stdout
      --list-shells      Print supported shell names and exit (no script on stdout)
      --catalog-hooks    Append catalog-aware tab-completion snippets (`plane-cli complete …`) for bash and zsh
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --install          Write completions under the user data/config dirs and, for bash/zsh, add a small marked block to your rc file (fish needs no rc edit)
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --dry-run          With --install, show paths and rc edits but do not write files
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

## `plane-cli complete`

```text
Print completion candidates (one per line). Use with `completions --catalog-hooks` or your shell's compgen.

Usage: plane-cli complete [OPTIONS] <COMMAND>

Commands:
  projects  Project short identifiers (e.g. ENG), one per line
  states    Workflow state names for `--state` (with `--strict-catalog`), one per line
  labels    Label names, one per line
  modules   Module UUIDs for this project (this CLI addresses modules by id)
  cycles    Cycle names for `--cycle`, one per line (JSON adds id, dates, status)
  help      Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli complete projects`

```text
Project short identifiers (e.g. ENG), one per line

Usage: plane-cli complete projects [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli complete states`

```text
Workflow state names for `--state` (with `--strict-catalog`), one per line

Usage: plane-cli complete states [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli complete labels`

```text
Label names, one per line

Usage: plane-cli complete labels [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli complete modules`

```text
Module UUIDs for this project (this CLI addresses modules by id)

Usage: plane-cli complete modules [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

### `plane-cli complete cycles`

```text
Cycle names for `--cycle`, one per line (JSON adds id, dates, status)

Usage: plane-cli complete cycles [OPTIONS]

Options:
      --json               Machine-readable JSON on stdout
      --project <PROJECT>  Project identifier or UUID
      --refresh            Bypass cache reads for catalog-backed commands
      --env-file <PATH>    Load environment variables from this file before reading config
      --dotenv             Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog     For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>        Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help               Print help
```

## `plane-cli self`

```text
Compare this build to GitHub releases and re-run the installer (public GitHub URLs; no Plane API config)

Usage: plane-cli self [OPTIONS] <COMMAND>

Commands:
  check   Compare this binary to the latest stable GitHub release
  update  Print the README install one-liner; use `--apply` to run it
  skills  Check/install the agent skill (`skills/plane-cli/SKILL.md`) for local coding agents
  help    Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli self check`

```text
Compare this binary to the latest stable GitHub release

Usage: plane-cli self check [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli self update`

```text
Print the README install one-liner; use `--apply` to run it

Usage: plane-cli self update [OPTIONS]

Options:
      --apply            Stream install.sh from GitHub and run it (same as the README one-liner)
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

### `plane-cli self skills`

```text
Check/install the agent skill (`skills/plane-cli/SKILL.md`) for local coding agents

Usage: plane-cli self skills [OPTIONS] <COMMAND>

Commands:
  check    Check whether the `plane-cli` skill is installed in common agent locations
  install  Install the agent skill: default runs `npx skills@latest add …`; `--local` or `--openclaw` write the full bundled skill directory (SKILL.md + CRUD-REFERENCE.md + WORKFLOWS.md)
  help     Print this message or the help of the given subcommand(s)

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli self skills check`

```text
Check whether the `plane-cli` skill is installed in common agent locations

Usage: plane-cli self skills check [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```

#### `plane-cli self skills install`

```text
Install the agent skill: default runs `npx skills@latest add …`; `--local` or `--openclaw` write the full bundled skill directory (SKILL.md + CRUD-REFERENCE.md + WORKFLOWS.md)

Usage: plane-cli self skills install [OPTIONS]

Options:
      --json             Machine-readable JSON on stdout
      --openclaw         Write the full skill bundle for OpenClaw-compatible runners (`~/.openclaw/skills/plane-cli/`)
      --local            Write the full skill bundle into local agent dirs (`~/.agents/skills`, `~/.cursor/skills`, …)
      --refresh          Bypass cache reads for catalog-backed commands
      --env-file <PATH>  Load environment variables from this file before reading config
      --dotenv           Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)
      --strict-catalog   For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)
      --as <MEMBER>      Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS
  -h, --help             Print help
```
