mod aliases;
mod attachments;
mod cycles;
mod features;
mod helpers;
mod inbox;
mod intake;
mod labels;
mod members;
mod modules;
pub(crate) mod policy_gate;
mod projects;
mod states;

pub use attachments::AttachmentsCmd;
pub use cycles::CyclesCmd;
pub use features::FeaturesArgs;
pub use inbox::InboxArgs;
pub use intake::IntakeCmd;
pub use labels::LabelsCmd;
pub use members::{InvitationsCmd, MembersCmd, ProjectMembersCmd};
pub use modules::{ModuleIssuesCmd, ModulesCmd};
pub use policy_gate::{CreatePolicyArgs, UpdatePolicyArgs};
pub use states::StatesCmd;

pub use aliases::{flag_alias_warnings, forward_command_alias};
pub use helpers::prevalidate;

use crate::body;
use crate::output;
use clap::{Parser, Subcommand, ValueHint};
use plane_core::{
    effective_json_output, load_json_source, merge_objects, resolve_project_id, CoreError,
    PageParams, PlaneApi,
};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "plane-cli",
    version,
    about = "CLI for Plane (self-hosted). Set PLANE_API_BASE_URL, PLANE_API_KEY, PLANE_WORKSPACE_SLUG."
)]
pub struct Cli {
    #[arg(
        long,
        id = "output_json",
        global = true,
        help = "Machine-readable JSON on stdout"
    )]
    pub json: bool,
    #[arg(
        long,
        global = true,
        help = "Bypass cache reads for catalog-backed commands"
    )]
    pub refresh: bool,
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        value_hint = ValueHint::AnyPath,
        help = "Load environment variables from this file before reading config"
    )]
    pub env_file: Option<std::path::PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Load `.env` from the current directory (optional; runs after --env-file / PLANE_DOTENV)"
    )]
    pub dotenv: bool,
    #[arg(
        long,
        global = true,
        help = "For work-item create/update, resolve state and labels via project catalogs (names → UUIDs)"
    )]
    pub strict_catalog: bool,
    #[arg(
        long = "as",
        id = "as_member",
        global = true,
        value_name = "MEMBER",
        help = "Act as this Plane member with its own API key (PLANE_API_KEY_<MEMBER> in the process env, then Keychain item plane-cli-<member> on macOS or ~/.config/plane-cli/keys/<member>); also PLANE_CLI_AS"
    )]
    pub as_member: Option<String>,
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    /// Machine-readable JSON on stdout: `--json` or `PLANE_CLI_JSON` (see the agent contract in the docs).
    #[inline]
    pub fn json_mode(&self) -> bool {
        effective_json_output(self.json)
    }

    /// Subcommand path for telemetry, e.g. `work-items comments list`.
    pub fn command_path(&self) -> String {
        use Commands::*;
        match &self.command {
            Config { cmd } => format!("config {}", config_cmd_name(cmd)),
            Auth { cmd } => format!("auth {}", auth_cmd_name(cmd)),
            Projects { cmd } => format!("projects {}", projects_cmd_name(cmd)),
            WorkItems { cmd } => format!("work-items {}", work_items_cmd_name(cmd)),
            Inbox(_) => "inbox".into(),
            Modules { cmd } => format!("modules {}", modules::modules_cmd_name(cmd)),
            Cycles { cmd } => format!("cycles {}", cycles::cycles_cmd_name(cmd)),
            Labels { cmd } => format!("labels {}", labels::labels_cmd_name(cmd)),
            States { cmd } => format!("states {}", states::states_cmd_name(cmd)),
            Intake { cmd } => format!("intake {}", intake::intake_cmd_name(cmd)),
            Members { cmd } => format!("members {}", members::members_cmd_name(cmd)),
            Invitations { cmd } => {
                format!("invitations {}", members::invitations_cmd_name(cmd))
            }
            Tui => "tui".into(),
            Completions { .. } => "completions".into(),
            Complete { cmd } => format!("complete {}", complete_cmd_name(cmd)),
            SelfManage { cmd } => format!("self {}", self_cmd_name(cmd)),
            StateAlias { .. } => "alias:state".into(),
            WorkItemAlias { .. } => "alias:work-item".into(),
            IssueAlias { .. } => "alias:issue".into(),
            TicketAlias { .. } => "alias:ticket".into(),
            ProjectAlias { .. } => "alias:project".into(),
        }
    }
}

fn config_cmd_name(cmd: &ConfigCmd) -> &'static str {
    match cmd {
        ConfigCmd::Init { .. } => "init",
        ConfigCmd::Update { .. } => "update",
        ConfigCmd::Doctor { .. } => "doctor",
        ConfigCmd::Where => "where",
        ConfigCmd::SetKey { .. } => "set-key",
        ConfigCmd::Policy { cmd } => crate::policy_cmd::policy_cmd_name(cmd),
    }
}

fn auth_cmd_name(cmd: &AuthCmd) -> &'static str {
    match cmd {
        AuthCmd::Whoami => "whoami",
    }
}

fn projects_cmd_name(cmd: &ProjectsCmd) -> String {
    match cmd {
        ProjectsCmd::List { .. } => "list".into(),
        ProjectsCmd::Get { .. } => "get".into(),
        ProjectsCmd::Context { .. } => "context".into(),
        ProjectsCmd::Create { .. } => "create".into(),
        ProjectsCmd::Update { .. } => "update".into(),
        ProjectsCmd::Delete { .. } => "delete".into(),
        ProjectsCmd::Archive { .. } => "archive".into(),
        ProjectsCmd::Unarchive { .. } => "unarchive".into(),
        ProjectsCmd::Summary { .. } => "summary".into(),
        ProjectsCmd::Features(_) => "features".into(),
        ProjectsCmd::Members { cmd } => {
            nested_cmd_name("members", cmd, members::project_members_cmd_name)
        }
    }
}

fn complete_cmd_name(cmd: &CompleteSubCmd) -> &'static str {
    match cmd {
        CompleteSubCmd::Projects => "projects",
        CompleteSubCmd::States { .. } => "states",
        CompleteSubCmd::Labels { .. } => "labels",
        CompleteSubCmd::Modules { .. } => "modules",
        CompleteSubCmd::Cycles { .. } => "cycles",
    }
}

fn self_cmd_name(cmd: &SelfManageCmd) -> String {
    match cmd {
        SelfManageCmd::Check => "check".into(),
        SelfManageCmd::Update { .. } => "update".into(),
        SelfManageCmd::Skills { cmd } => format!("skills {}", self_skills_cmd_name(cmd)),
    }
}

fn self_skills_cmd_name(cmd: &SelfSkillsCmd) -> &'static str {
    match cmd {
        SelfSkillsCmd::Check => "check",
        SelfSkillsCmd::Install { .. } => "install",
    }
}

fn nested_cmd_name<T>(name: &str, cmd: &T, sub: fn(&T) -> &'static str) -> String {
    format!("{name} {}", sub(cmd))
}

fn work_items_cmd_name(cmd: &WorkItemsCmd) -> String {
    match cmd {
        WorkItemsCmd::List { .. } => "list".into(),
        WorkItemsCmd::Get { .. } => "get".into(),
        WorkItemsCmd::Create { .. } => "create".into(),
        WorkItemsCmd::Update { .. } => "update".into(),
        WorkItemsCmd::Delete { .. } => "delete".into(),
        WorkItemsCmd::Search { .. } => "search".into(),
        WorkItemsCmd::Activities { cmd } => nested_cmd_name("activities", cmd, activities_cmd_name),
        WorkItemsCmd::Comments { cmd } => nested_cmd_name("comments", cmd, comments_cmd_name),
        WorkItemsCmd::Links { cmd } => nested_cmd_name("links", cmd, links_cmd_name),
        WorkItemsCmd::Relations { cmd } => nested_cmd_name("relations", cmd, relations_cmd_name),
        WorkItemsCmd::Attachments { cmd } => {
            nested_cmd_name("attachments", cmd, attachments::attachments_cmd_name)
        }
    }
}

fn activities_cmd_name(cmd: &ActivitiesCmd) -> &'static str {
    match cmd {
        ActivitiesCmd::List { .. } => "list",
        ActivitiesCmd::Get { .. } => "get",
    }
}

fn comments_cmd_name(cmd: &CommentsCmd) -> &'static str {
    match cmd {
        CommentsCmd::List { .. } => "list",
        CommentsCmd::Get { .. } => "get",
        CommentsCmd::Create { .. } => "create",
        CommentsCmd::Update { .. } => "update",
        CommentsCmd::Delete { .. } => "delete",
    }
}

fn links_cmd_name(cmd: &LinksCmd) -> &'static str {
    match cmd {
        LinksCmd::List { .. } => "list",
        LinksCmd::Get { .. } => "get",
        LinksCmd::Create { .. } => "create",
        LinksCmd::Update { .. } => "update",
        LinksCmd::Delete { .. } => "delete",
    }
}

fn relations_cmd_name(cmd: &RelationsCmd) -> &'static str {
    match cmd {
        RelationsCmd::List { .. } => "list",
        RelationsCmd::Create { .. } => "create",
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Configure local env file and inspect required PLANE_* settings
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Verify API URL, key, and workspace (GET /api/v1/users/me/)
    Auth {
        #[command(subcommand)]
        cmd: AuthCmd,
    },
    /// Workspace projects
    Projects {
        #[command(subcommand)]
        cmd: ProjectsCmd,
    },
    /// Work items and nested resources
    WorkItems {
        #[command(subcommand)]
        cmd: WorkItemsCmd,
    },
    /// Work items assigned to you (or the `--as` member) that are not Done or Cancelled,
    /// ordered by priority then age and capped for one agent turn (inbox fetch)
    Inbox(InboxArgs),
    /// Project modules
    Modules {
        #[command(subcommand)]
        cmd: ModulesCmd,
    },
    /// Project cycles and the work items in them
    Cycles {
        #[command(subcommand)]
        cmd: CyclesCmd,
    },
    /// Project labels (create, rename, recolour, delete)
    Labels {
        #[command(subcommand)]
        cmd: LabelsCmd,
    },
    /// Project workflow states (group, colour, sequence, default)
    States {
        #[command(subcommand)]
        cmd: StatesCmd,
    },
    /// Project intake queue (turn on with `projects features --intake on`)
    Intake {
        #[command(subcommand)]
        cmd: IntakeCmd,
    },
    /// Workspace members (project members: `projects members`)
    Members {
        #[command(subcommand)]
        cmd: MembersCmd,
    },
    /// Workspace invitations
    Invitations {
        #[command(subcommand)]
        cmd: InvitationsCmd,
    },
    /// Browse and edit projects and work items in a full-screen terminal UI
    Tui,
    /// Print shell completion script to stdout, or install into conventional paths (`--install`)
    Completions {
        /// bash, zsh, fish, elvish, powershell — omit to infer from `$SHELL` (falls back to bash)
        #[arg(value_enum)]
        shell: Option<clap_complete::Shell>,
        #[arg(
            long,
            help = "Print supported shell names and exit (no script on stdout)"
        )]
        list_shells: bool,
        #[arg(
            long,
            help = "Append catalog-aware tab-completion snippets (`plane-cli complete …`) for bash and zsh"
        )]
        catalog_hooks: bool,
        #[arg(
            long,
            help = "Write completions under the user data/config dirs and, for bash/zsh, add a small marked block to your rc file (fish needs no rc edit)"
        )]
        install: bool,
        #[arg(
            long,
            help = "With --install, show paths and rc edits but do not write files"
        )]
        dry_run: bool,
    },
    /// List candidate values for shell tab completion (uses workspace cache / API; requires PLANE_* env)
    #[command(
        name = "complete",
        visible_alias = "completion",
        about = "Print completion candidates (one per line). Use with `completions --catalog-hooks` or your shell's compgen."
    )]
    Complete {
        #[command(subcommand)]
        cmd: CompleteSubCmd,
    },
    /// Compare this build to GitHub releases and re-run the installer (public GitHub URLs; no Plane API config)
    #[command(name = "self")]
    SelfManage {
        #[command(subcommand)]
        cmd: SelfManageCmd,
    },

    // Hidden redirect commands — agents sometimes use wrong names; we emit a structured hint.
    /// Hidden: redirect "state …" → "states …"
    #[command(name = "state", hide = true)]
    StateAlias {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _args: Vec<String>,
    },
    /// Hidden: redirect "work-item …" → "work-items …"
    #[command(name = "work-item", hide = true)]
    WorkItemAlias {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _args: Vec<String>,
    },
    /// Hidden: redirect "issue …" → "work-items …"
    #[command(name = "issue", alias = "issues", hide = true)]
    IssueAlias {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _args: Vec<String>,
    },
    /// Hidden: redirect "ticket …" → "work-items …"
    #[command(name = "ticket", hide = true)]
    TicketAlias {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _args: Vec<String>,
    },
    /// Hidden: redirect "project …" → "projects …"
    #[command(name = "project", hide = true)]
    ProjectAlias {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Interactive wizard to write a local env file (defaults to ./.plane-cli.env)
    Init {
        #[arg(
            long,
            value_name = "PATH",
            value_hint = ValueHint::AnyPath,
            help = "Output env file path (default: ./.plane-cli.env, or user config dir with --global)"
        )]
        env_file: Option<std::path::PathBuf>,
        #[arg(
            long,
            help = "Write ~/.config/plane-cli/.plane-cli.env (or XDG_CONFIG_HOME / %APPDATA%\\plane-cli on Windows); cannot combine with `config init --env-file PATH`"
        )]
        global: bool,
        #[arg(long, help = "Overwrite env file if it already exists")]
        overwrite: bool,
    },
    /// Interactive wizard to update an existing env file (Enter keeps each current value)
    Update {
        #[arg(
            long,
            value_name = "PATH",
            value_hint = ValueHint::AnyPath,
            help = "Env file path (default: ./.plane-cli.env, or user config dir with --global)"
        )]
        env_file: Option<std::path::PathBuf>,
        #[arg(
            long,
            help = "Edit ~/.config/plane-cli/.plane-cli.env (or XDG_CONFIG_HOME / %APPDATA%\\plane-cli on Windows); cannot combine with `config update --env-file PATH`"
        )]
        global: bool,
    },
    /// Show whether required PLANE_* variables are present and whether the API answers
    Doctor {
        #[arg(
            long,
            help = "Skip the reachability check (one GET /api/v1/users/me/ with a 5 s timeout)"
        )]
        offline: bool,
    },
    /// Show config file paths and which env layers exist (no secret values)
    Where,
    /// Store a Plane member's API key for `--as` without echoing it: macOS Keychain item
    /// `plane-cli-<member>`, elsewhere `~/.config/plane-cli/keys/<member>` (mode 600).
    /// Reads the key from a hidden TTY prompt, or from stdin when stdin is not a terminal.
    #[command(name = "set-key")]
    SetKey {
        #[arg(long, value_name = "MEMBER", help = "Plane member name (e.g. bob)")]
        member: String,
    },
    /// Per-project write policies enforced on work-item writes (`init`, `show`)
    Policy {
        #[command(subcommand)]
        cmd: crate::policy_cmd::PolicyCmd,
    },
}

#[derive(Subcommand)]
pub enum SelfManageCmd {
    /// Compare this binary to the latest stable GitHub release
    Check,
    /// Print the README install one-liner; use `--apply` to run it
    Update {
        #[arg(
            long,
            help = "Stream install.sh from GitHub and run it (same as the README one-liner)"
        )]
        apply: bool,
    },
    /// Check/install the agent skill (`skills/plane-cli/SKILL.md`) for local coding agents
    Skills {
        #[command(subcommand)]
        cmd: SelfSkillsCmd,
    },
}

#[derive(Subcommand)]
pub enum SelfSkillsCmd {
    /// Check whether the `plane-cli` skill is installed in common agent locations
    Check,
    /// Install the agent skill: default runs `npx skills@latest add …`; `--local` or `--openclaw` write the full bundled skill directory (SKILL.md + CRUD-REFERENCE.md + WORKFLOWS.md)
    Install {
        /// Write the full skill bundle for OpenClaw-compatible runners (`~/.openclaw/skills/plane-cli/`)
        #[arg(long, conflicts_with = "local")]
        openclaw: bool,
        /// Write the full skill bundle into local agent dirs (`~/.agents/skills`, `~/.cursor/skills`, …)
        #[arg(long, conflicts_with = "openclaw")]
        local: bool,
    },
}

#[derive(Subcommand)]
pub enum CompleteSubCmd {
    /// Project short identifiers (e.g. ENG), one per line
    Projects,
    /// Workflow state names for `--state` (with `--strict-catalog`), one per line
    States {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        project: Option<String>,
        /// Positional fallback: agents sometimes write `complete states <PROJECT>` (emits warning)
        #[arg(value_name = "EXTRA", hide = true, num_args = 0..=2)]
        extra: Vec<String>,
    },
    /// Label names, one per line
    Labels {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        project: Option<String>,
        /// Positional fallback: agents sometimes write `complete labels <PROJECT>` (emits warning)
        #[arg(value_name = "EXTRA", hide = true, num_args = 0..=2)]
        extra: Vec<String>,
    },
    /// Module UUIDs for this project (this CLI addresses modules by id)
    Modules {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        project: Option<String>,
        /// Positional fallback: agents sometimes write `complete modules <PROJECT>` (emits warning)
        #[arg(value_name = "EXTRA", hide = true, num_args = 0..=2)]
        extra: Vec<String>,
    },
    /// Cycle names for `--cycle`, one per line (JSON adds id, dates, status)
    Cycles {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        project: Option<String>,
        /// Positional fallback: agents sometimes write `complete cycles <PROJECT>` (emits warning)
        #[arg(value_name = "EXTRA", hide = true, num_args = 0..=2)]
        extra: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum AuthCmd {
    Whoami,
}

#[derive(Subcommand)]
pub enum ProjectsCmd {
    /// List projects (updates local cache snapshot)
    List {
        #[command(flatten)]
        page: PageArgs,
    },
    Get {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        id: String,
    },
    /// Agent snapshot: capabilities, grouped states, labels, modules, hints (--json required)
    Context {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        id: String,
    },
    Create {
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        name: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        description: Option<String>,
        #[arg(long, value_name = "SHORT_ID", value_hint = ValueHint::Other)]
        identifier: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        project_lead: Option<String>,
    },
    Update {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        id: String,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        name: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        description: Option<String>,
        #[arg(long, value_name = "SHORT_ID", value_hint = ValueHint::Other)]
        identifier: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        project_lead: Option<String>,
    },
    Delete {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        id: String,
    },
    /// Archive a project: hides its work items, states, cycles and modules until unarchived
    Archive {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        id: String,
    },
    /// Restore an archived project
    Unarchive {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        id: String,
    },
    /// Counts of work items, cycles, modules, members, states, labels, intake items and pages
    Summary {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier or UUID"
        )]
        id: String,
    },
    /// Show or switch features: --cycles, --modules, --pages, --intake, --views (on|off)
    Features(FeaturesArgs),
    /// Project members (add, change role, remove)
    Members {
        #[command(subcommand)]
        cmd: ProjectMembersCmd,
    },
}

#[derive(Subcommand)]
pub enum WorkItemsCmd {
    List {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: Option<String>,
        /// Positional fallback: agents sometimes omit --project flag (emits warning)
        #[arg(value_name = "PROJECT_POSITIONAL", hide = true)]
        project_positional: Option<String>,
        /// Extra unexpected positionals absorbed gracefully (emits warning)
        #[arg(value_name = "EXTRA", hide = true, num_args = 0..)]
        extra_positional: Vec<String>,
        /// Filter by state name or UUID (client-side; combine with --strict-catalog for name resolution)
        #[arg(
            long,
            value_name = "STATE",
            value_hint = ValueHint::Other,
            help = "Filter results by state name or UUID"
        )]
        state: Option<String>,
        /// Filter by assignee UUID (repeatable)
        #[arg(
            long = "assignee",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            action = clap::ArgAction::Append,
            help = "Filter results by assignee: user UUID, or workspace member display name or email (repeatable)"
        )]
        assignee: Vec<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    Get {
        #[arg(
            long,
            value_name = "ITEM",
            value_hint = ValueHint::Other,
            help = "Composite id (PROJ-123) or work item UUID"
        )]
        work_item: Option<String>,
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Required when ITEM is a bare UUID"
        )]
        project: Option<String>,
        /// Positional fallback: [PROJECT] [WORK_ITEM] or just [WORK_ITEM] (emits warning)
        #[arg(value_name = "POSITIONAL", hide = true, num_args = 0..=2)]
        positional: Vec<String>,
    },
    Create {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        name: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        description: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        priority: Option<String>,
        #[arg(long, value_name = "STATE", value_hint = ValueHint::Other)]
        state: Option<String>,
        #[arg(
            long = "assignee",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            action = clap::ArgAction::Append,
            help = "User UUID to assign (repeatable). Use --body '{\"assignees\":[]}' to clear."
        )]
        assignee: Vec<String>,
        #[command(flatten)]
        policy: CreatePolicyArgs,
    },
    Update {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        name: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        description: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        priority: Option<String>,
        #[arg(long, value_name = "STATE", value_hint = ValueHint::Other)]
        state: Option<String>,
        #[arg(
            long = "assignee",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            action = clap::ArgAction::Append,
            help = "User UUID to assign (repeatable). Use --body '{\"assignees\":[]}' to clear."
        )]
        assignee: Vec<String>,
        #[command(flatten)]
        policy: UpdatePolicyArgs,
    },
    Delete {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(long, help = "Show the request and the policy checks without deleting")]
        dry_run: bool,
    },
    Search {
        #[arg(long, value_name = "QUERY", value_hint = ValueHint::Other)]
        query: String,
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Limit search to one project (identifier or UUID)"
        )]
        project: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    Activities {
        #[command(subcommand)]
        cmd: ActivitiesCmd,
    },
    Comments {
        #[command(subcommand)]
        cmd: CommentsCmd,
    },
    Links {
        #[command(subcommand)]
        cmd: LinksCmd,
    },
    Relations {
        #[command(subcommand)]
        cmd: RelationsCmd,
    },
    /// Files attached to a work item
    Attachments {
        #[command(subcommand)]
        cmd: AttachmentsCmd,
    },
}

#[derive(Subcommand)]
pub enum ActivitiesCmd {
    List {
        #[command(flatten)]
        target: WorkItemTarget,
        #[command(flatten)]
        page: PageArgs,
    },
    Get {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Resource UUID"
        )]
        id: String,
    },
}

#[derive(Subcommand)]
pub enum CommentsCmd {
    List {
        #[command(flatten)]
        target: WorkItemTarget,
        #[command(flatten)]
        page: PageArgs,
    },
    Get {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            alias = "comment-id",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Resource UUID"
        )]
        id: String,
    },
    Create {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        comment_html: Option<String>,
    },
    Update {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            alias = "comment-id",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Resource UUID"
        )]
        id: String,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        comment_html: Option<String>,
    },
    Delete {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            alias = "comment-id",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Resource UUID"
        )]
        id: String,
    },
}

#[derive(Subcommand)]
pub enum LinksCmd {
    List {
        #[command(flatten)]
        target: WorkItemTarget,
        #[command(flatten)]
        page: PageArgs,
    },
    Get {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            alias = "link-id",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Resource UUID"
        )]
        id: String,
    },
    Create {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        url: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        title: Option<String>,
    },
    Update {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            alias = "link-id",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Resource UUID"
        )]
        id: String,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        url: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        title: Option<String>,
    },
    Delete {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            alias = "link-id",
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Resource UUID"
        )]
        id: String,
    },
}

#[derive(Subcommand)]
pub enum RelationsCmd {
    List {
        #[command(flatten)]
        target: WorkItemTarget,
    },
    Create {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
        #[arg(long, value_hint = ValueHint::Other)]
        relation_type: Option<String>,
        /// Issue UUIDs, comma-separated
        #[arg(
            long,
            value_delimiter = ',',
            value_name = "ISSUE_UUID",
            value_hint = ValueHint::Other
        )]
        issues: Option<Vec<String>>,
    },
}

// Target work item for `update`, `delete`, `comments`, `links`, `relations`, `activities`.
// `--project` is optional when `--work-item` is a composite id (`ENG-123`). Plain comment on
// purpose: a doc comment here would become every flattening subcommand's `about` text.
#[derive(clap::Args, Clone, Debug, Default)]
pub struct WorkItemTarget {
    #[arg(
        long,
        value_name = "PROJECT",
        value_hint = ValueHint::Other,
        help = "Project identifier (e.g. ENG) or UUID; optional when ITEM is a PREFIX-N id"
    )]
    pub project: Option<String>,
    #[arg(
        long,
        value_name = "ITEM",
        value_hint = ValueHint::Other,
        help = "Composite id (PROJ-123) or work item UUID (UUID needs --project)"
    )]
    pub work_item: Option<String>,
    /// Positional fallback: agents write `comments list ENG-123` (emits warning)
    #[arg(value_name = "WORK_ITEM_POSITIONAL", hide = true)]
    pub work_item_positional: Option<String>,
}

#[derive(clap::Args, Clone, Default)]
pub struct PageArgs {
    #[arg(long, value_name = "N")]
    pub page_size: Option<u32>,
    #[arg(long, value_name = "CURSOR", value_hint = ValueHint::Other)]
    pub cursor: Option<String>,
    #[arg(long)]
    pub all: bool,
}

impl PageArgs {
    fn params(&self) -> PageParams {
        PageParams {
            per_page: self.page_size,
            cursor: self.cursor.clone(),
            fetch_all: self.all,
        }
    }
}

pub fn run(cli: &Cli, json_mode: bool, api: &mut PlaneApi) -> Result<(), CoreError> {
    let refresh = cli.refresh;
    let strict_catalog = cli.strict_catalog;
    let catalog = api.fetch_projects_catalog(refresh)?;

    match &cli.command {
        Commands::Config { .. } => unreachable!("`config` is handled in main"),
        Commands::SelfManage { .. } => unreachable!("`self` is handled in main"),
        Commands::Auth { cmd } => match cmd {
            AuthCmd::Whoami => {
                let v = api.whoami()?;
                output::emit_success(json_mode, v, None, false)?;
            }
        },
        Commands::Projects { cmd } => match cmd {
            ProjectsCmd::List { page } => {
                let (v, nc, tr) = api.list_projects_public(&page.params())?;
                api.save_projects_to_cache(&v)?;
                output::emit_success(json_mode, v, nc, tr)?;
            }
            ProjectsCmd::Get { id } => {
                let pid = resolve_project_id(id, &catalog)?;
                let v = api.get_project(&pid)?;
                output::emit_success(json_mode, v, None, false)?;
            }
            ProjectsCmd::Context { id } => {
                crate::projects_context_cmd::run(cli, json_mode, id, api)?;
            }
            ProjectsCmd::Create {
                json,
                name,
                description,
                identifier,
                project_lead,
            } => {
                let body = body::project_create_flags(
                    json.as_deref(),
                    name.clone(),
                    description.clone(),
                    identifier.clone(),
                    project_lead.clone(),
                )?;
                let v = api.create_project(&body)?;
                output::emit_success(json_mode, v, None, false)?;
            }
            ProjectsCmd::Update {
                id,
                json,
                name,
                description,
                identifier,
                project_lead,
            } => {
                let pid = resolve_project_id(id, &catalog)?;
                let body = body::project_create_flags(
                    json.as_deref(),
                    name.clone(),
                    description.clone(),
                    identifier.clone(),
                    project_lead.clone(),
                )?;
                let v = api.update_project(&pid, &body)?;
                output::emit_success(json_mode, v, None, false)?;
            }
            ProjectsCmd::Delete { id } => projects::delete(json_mode, api, &catalog, id)?,
            ProjectsCmd::Archive { id } => projects::archive(json_mode, api, &catalog, id)?,
            ProjectsCmd::Unarchive { id } => projects::unarchive(json_mode, api, &catalog, id)?,
            ProjectsCmd::Summary { id } => projects::summary(json_mode, api, &catalog, id)?,
            ProjectsCmd::Features(args) => features::run(args, json_mode, api, &catalog)?,
            ProjectsCmd::Members { cmd } => members::run_project(cmd, json_mode, api, &catalog)?,
        },
        Commands::WorkItems { cmd } => match cmd {
            WorkItemsCmd::List {
                project,
                project_positional,
                extra_positional,
                state,
                assignee,
                page,
            } => {
                let mut warnings: Vec<String> = vec![];
                let project_str = resolve_project_arg(
                    project.as_deref(),
                    project_positional.as_deref(),
                    &mut warnings,
                )?;
                if !extra_positional.is_empty() {
                    warnings.push(format!(
                        "unexpected positional arg(s) {:?} ignored; pass filters with --state or --assignee",
                        extra_positional
                    ));
                }
                let pid = helpers::resolve_project_auto_refresh(
                    api,
                    &project_str,
                    &catalog,
                    refresh,
                    &mut warnings,
                )?;
                // A non-UUID `--assignee` is a member name or email; an unknown one is an
                // error, never a silent empty list.
                let assignee = resolve_assignee_filters(api, assignee)?;
                let (v, nc, tr) = api.list_work_items(&pid, &page.params())?;
                let (v, filter_warnings) =
                    apply_work_item_filters(v, state.as_deref(), &assignee, api, &pid, refresh)?;
                warnings.extend(filter_warnings);
                output::emit_success_with_warnings(json_mode, v, nc, tr, warnings)?;
            }
            WorkItemsCmd::Get {
                work_item,
                project,
                positional,
            } => {
                let mut warnings: Vec<String> = vec![];
                let (effective_work_item, effective_project) = resolve_get_args(
                    work_item.as_deref(),
                    project.as_deref(),
                    positional,
                    &mut warnings,
                )?;
                let v = helpers::get_work_item_checked(
                    api,
                    &catalog,
                    refresh,
                    effective_project,
                    &effective_work_item,
                    &mut warnings,
                )?;
                output::emit_success_with_warnings(json_mode, v, None, false, warnings)?;
            }
            WorkItemsCmd::Create {
                project,
                json,
                name,
                description,
                priority,
                state,
                assignee,
                policy,
            } => {
                let pid = resolve_project_id(project, &catalog)?;
                let assignees = if assignee.is_empty() {
                    None
                } else {
                    Some(assignee.clone())
                };
                let body = body::work_item_flags(
                    json.as_deref(),
                    name.clone(),
                    description.clone(),
                    priority.clone(),
                    state.clone(),
                    assignees,
                )?;
                let body = helpers::normalize_work_item_if_strict(
                    api,
                    pid,
                    body,
                    strict_catalog,
                    refresh,
                )?;
                policy_gate::run_create(api, &catalog, refresh, json_mode, pid, body, policy)?;
            }
            WorkItemsCmd::Update {
                target,
                json,
                name,
                description,
                priority,
                state,
                assignee,
                policy,
            } => {
                let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                let assignees = if assignee.is_empty() {
                    None
                } else {
                    Some(assignee.clone())
                };
                let body = body::work_item_flags(
                    json.as_deref(),
                    name.clone(),
                    description.clone(),
                    priority.clone(),
                    state.clone(),
                    assignees,
                )?;
                let body = helpers::normalize_work_item_if_strict(
                    api,
                    pid,
                    body,
                    strict_catalog,
                    refresh,
                )?;
                policy_gate::run_update(api, &catalog, refresh, json_mode, pid, iid, body, policy)?;
            }
            WorkItemsCmd::Delete { target, dry_run } => {
                let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                policy_gate::run_delete(api, &catalog, json_mode, pid, iid, *dry_run)?;
            }
            WorkItemsCmd::Search {
                query,
                project,
                limit,
            } => {
                let opt_pid = match project {
                    Some(p) => Some(resolve_project_id(p, &catalog)?),
                    None => None,
                };
                let v = api.search_work_items(query, opt_pid.as_ref(), *limit)?;
                output::emit_success(json_mode, v, None, false)?;
            }
            WorkItemsCmd::Activities { cmd } => match cmd {
                ActivitiesCmd::List { target, page } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let (v, nc, tr) = api.list_activities(&pid, &iid, &page.params())?;
                    output::emit_success(json_mode, v, nc, tr)?;
                }
                ActivitiesCmd::Get { target, id } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let pk = Uuid::parse_str(id).map_err(|_| {
                        CoreError::Usage(format!("invalid activity id UUID: {id:?}"))
                    })?;
                    let v = api.get_activity(&pid, &iid, &pk)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
            },
            WorkItemsCmd::Comments { cmd } => match cmd {
                CommentsCmd::List { target, page } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let (v, nc, tr) = api.list_comments(&pid, &iid, &page.params())?;
                    output::emit_success(json_mode, v, nc, tr)?;
                }
                CommentsCmd::Get { target, id } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let pk = Uuid::parse_str(id).map_err(|_| {
                        CoreError::Usage(format!("invalid comment id UUID: {id:?}"))
                    })?;
                    let v = api.get_comment(&pid, &iid, &pk)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
                CommentsCmd::Create {
                    target,
                    json,
                    comment_html,
                } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let body = body::comment_flags(json.as_deref(), comment_html.clone())?;
                    let v = api.create_comment(&pid, &iid, &body)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
                CommentsCmd::Update {
                    target,
                    id,
                    json,
                    comment_html,
                } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let pk = Uuid::parse_str(id).map_err(|_| {
                        CoreError::Usage(format!("invalid comment id UUID: {id:?}"))
                    })?;
                    let body = body::comment_flags(json.as_deref(), comment_html.clone())?;
                    let v = api.update_comment(&pid, &iid, &pk, &body)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
                CommentsCmd::Delete { target, id } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let pk = Uuid::parse_str(id).map_err(|_| {
                        CoreError::Usage(format!("invalid comment id UUID: {id:?}"))
                    })?;
                    policy_gate::gate_delete(
                        api,
                        &catalog,
                        &pid,
                        "comment",
                        "work-items comments delete",
                    )?;
                    api.delete_comment(&pid, &iid, &pk)?;
                    output::emit_empty_success(json_mode)?;
                }
            },
            WorkItemsCmd::Links { cmd } => match cmd {
                LinksCmd::List { target, page } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let (v, nc, tr) = api.list_links(&pid, &iid, &page.params())?;
                    output::emit_success(json_mode, v, nc, tr)?;
                }
                LinksCmd::Get { target, id } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let pk = Uuid::parse_str(id)
                        .map_err(|_| CoreError::Usage(format!("invalid link id UUID: {id:?}")))?;
                    let v = api.get_link(&pid, &iid, &pk)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
                LinksCmd::Create {
                    target,
                    json,
                    url,
                    title,
                } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let body = body::link_flags(json.as_deref(), url.clone(), title.clone())?;
                    let v = api.create_link(&pid, &iid, &body)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
                LinksCmd::Update {
                    target,
                    id,
                    json,
                    url,
                    title,
                } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let pk = Uuid::parse_str(id)
                        .map_err(|_| CoreError::Usage(format!("invalid link id UUID: {id:?}")))?;
                    let body = body::link_flags(json.as_deref(), url.clone(), title.clone())?;
                    let v = api.update_link(&pid, &iid, &pk, &body)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
                LinksCmd::Delete { target, id } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let pk = Uuid::parse_str(id)
                        .map_err(|_| CoreError::Usage(format!("invalid link id UUID: {id:?}")))?;
                    policy_gate::gate_delete(
                        api,
                        &catalog,
                        &pid,
                        "link",
                        "work-items links delete",
                    )?;
                    api.delete_link(&pid, &iid, &pk)?;
                    output::emit_empty_success(json_mode)?;
                }
            },
            WorkItemsCmd::Relations { cmd } => match cmd {
                RelationsCmd::List { target } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let v = api.list_relations(&pid, &iid)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
                RelationsCmd::Create {
                    target,
                    json,
                    relation_type,
                    issues,
                } => {
                    let (pid, iid) = helpers::resolve_target(api, &catalog, refresh, target)?;
                    let mut patch = serde_json::Map::new();
                    if let Some(rt) = relation_type {
                        patch.insert("relation_type".into(), json!(rt));
                    }
                    if let Some(iss) = issues {
                        let uuids: Result<Vec<Uuid>, _> =
                            iss.iter().map(|s| Uuid::parse_str(s.trim())).collect();
                        let uuids = uuids
                            .map_err(|_| CoreError::Usage("invalid UUID in --issues".into()))?;
                        patch.insert("issues".into(), json!(uuids));
                    }
                    let base = load_json_source(json.as_deref())?.unwrap_or(json!({}));
                    let body = merge_objects(base, Value::Object(patch));
                    let v = api.create_relations(&pid, &iid, &body)?;
                    output::emit_success(json_mode, v, None, false)?;
                }
            },
            WorkItemsCmd::Attachments { cmd } => {
                attachments::run(cmd, json_mode, api, &catalog, refresh)?
            }
        },
        Commands::Modules { cmd } => modules::run(cmd, json_mode, api, &catalog, refresh)?,
        Commands::Inbox(args) => inbox::run(args, json_mode, api, &catalog, refresh)?,
        Commands::Cycles { cmd } => cycles::run(cmd, json_mode, api, &catalog, refresh)?,
        Commands::Labels { cmd } => labels::run(cmd, json_mode, api, &catalog)?,
        Commands::States { cmd } => states::run(cmd, json_mode, api, &catalog)?,
        Commands::Intake { cmd } => intake::run(cmd, json_mode, api, &catalog, refresh)?,
        Commands::Members { cmd } => members::run_workspace(cmd, json_mode, api)?,
        Commands::Invitations { cmd } => members::run_invitations(cmd, json_mode, api)?,
        Commands::Tui => {
            crate::tui::run_tui(api)?;
        }
        Commands::Complete { cmd } => {
            crate::complete_cmd::run(cli, json_mode, cmd, api)?;
        }
        Commands::Completions { .. } => {}
        Commands::StateAlias { .. } => {
            return Err(CoreError::Usage(
                "unknown command 'state'; use: plane-cli states list --project <PROJECT> --json\n\
                 The command is plural: `states`, not `state`."
                    .into(),
            ));
        }
        cmd @ (Commands::WorkItemAlias { .. }
        | Commands::IssueAlias { .. }
        | Commands::TicketAlias { .. }) => {
            let guidance = aliases::alias_guidance(cmd).unwrap_or("unknown command");
            return Err(CoreError::Usage(guidance.into()));
        }
        Commands::ProjectAlias { .. } => {
            return Err(CoreError::Usage(
                "unknown command 'project'; use: plane-cli projects <list|get|create|update|delete> --json\n\
                 The command is plural: `projects`, not `project`."
                    .into(),
            ));
        }
    }

    Ok(())
}

/// Resolve project identifier from --project flag or positional fallback. Emits warning if positional was used.
fn resolve_project_arg<'a>(
    flag: Option<&'a str>,
    positional: Option<&'a str>,
    warnings: &mut Vec<String>,
) -> Result<String, CoreError> {
    match (flag, positional) {
        (Some(p), _) => Ok(p.to_string()),
        (None, Some(p)) => {
            warnings.push(format!(
                "project {p:?} passed as positional arg; use --project {p} to suppress this warning"
            ));
            Ok(p.to_string())
        }
        (None, None) => Err(CoreError::Usage(
            "--project <PROJECT> is required (project identifier, e.g. ENG, or UUID)".into(),
        )),
    }
}

/// Resolve (work_item, project) from flag + optional positional args. Emits warning when positionals used.
fn resolve_get_args<'a>(
    work_item_flag: Option<&'a str>,
    project_flag: Option<&'a str>,
    positional: &'a [String],
    warnings: &mut Vec<String>,
) -> Result<(String, Option<&'a str>), CoreError> {
    if let Some(wi) = work_item_flag {
        return Ok((wi.to_string(), project_flag));
    }
    match positional {
        [proj, wi] => {
            warnings.push(format!(
                "positional args [{proj:?}, {wi:?}] interpreted as --project {proj} --work-item {wi}; use explicit flags to suppress this warning"
            ));
            Ok((wi.clone(), Some(proj.as_str())))
        }
        [wi] => {
            warnings.push(format!(
                "positional arg {wi:?} interpreted as --work-item; use --work-item flag to suppress this warning"
            ));
            Ok((wi.clone(), project_flag))
        }
        [] => Err(CoreError::Usage(
            "--work-item <ITEM> is required (composite id like PROJ-123 or UUID)".into(),
        )),
        _ => Err(CoreError::Usage(format!(
            "too many positional args {:?}; use --project and --work-item flags",
            positional
        ))),
    }
}

/// Filter work item list results by state and/or assignee. Returns (filtered_value, warnings).
fn apply_work_item_filters(
    v: serde_json::Value,
    state: Option<&str>,
    assignees: &[String],
    api: &mut plane_core::PlaneApi,
    project_id: &uuid::Uuid,
    refresh: bool,
) -> Result<(serde_json::Value, Vec<String>), CoreError> {
    use serde_json::Value;

    if state.is_none() && assignees.is_empty() {
        return Ok((v, vec![]));
    }

    let mut filter_warnings: Vec<String> = vec![];

    // Resolve state name → UUID if needed
    let state_uuid: Option<String> = if let Some(s) = state {
        if uuid::Uuid::parse_str(s).is_ok() {
            Some(s.to_string())
        } else {
            // Try resolving name via states catalog
            let states = api.fetch_states_catalog(project_id, refresh)?;
            let matched: Vec<_> = states
                .iter()
                .filter(|st| st.name.eq_ignore_ascii_case(s))
                .collect();
            match matched.len() {
                0 => {
                    let examples: Vec<_> =
                        states.iter().take(5).map(|st| st.name.as_str()).collect();
                    return Err(CoreError::Resolve(format!(
                        "unknown state {:?} for --state filter; available: {}",
                        s,
                        examples.join(", ")
                    )));
                }
                1 => Some(matched[0].id.to_string()),
                _ => {
                    return Err(CoreError::Resolve(format!(
                        "ambiguous state name {:?}: {} matches; use UUID instead",
                        s,
                        matched.len()
                    )));
                }
            }
        }
    } else {
        None
    };

    filter_warnings.push(
        "--state/--assignee filtering is client-side; result count may be lower than expected \
         if the page was truncated before filtering — use --all to fetch all pages first"
            .into(),
    );

    let filter_items = |items: Vec<Value>| -> Vec<Value> {
        items
            .into_iter()
            .filter(|item| work_item_matches(item, state_uuid.as_deref(), assignees))
            .collect()
    };

    let filtered = match &v {
        Value::Object(obj) if obj.contains_key("results") => {
            let results = obj
                .get("results")
                .and_then(|r| r.as_array())
                .cloned()
                .unwrap_or_default();
            let filtered_results = filter_items(results);
            let mut out = obj.clone();
            out.insert("results".into(), Value::Array(filtered_results.clone()));
            out.insert("count".into(), Value::Number(filtered_results.len().into()));
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(filter_items(arr.clone())),
        other => other.clone(),
    };

    Ok((filtered, filter_warnings))
}

/// `--assignee` values as user UUIDs: UUIDs pass through; anything else is looked up as a
/// workspace member display name or email (`validation_error` when unknown or ambiguous).
fn resolve_assignee_filters(
    api: &plane_core::PlaneApi,
    raw: &[String],
) -> Result<Vec<String>, CoreError> {
    if raw.iter().all(|r| Uuid::parse_str(r.trim()).is_ok()) {
        return Ok(raw.iter().map(|r| r.trim().to_string()).collect());
    }
    let members = api.list_workspace_members()?;
    let rows = members
        .as_array()
        .cloned()
        .or_else(|| members.get("results").and_then(|r| r.as_array()).cloned())
        .map(Value::Array)
        .unwrap_or(Value::Array(vec![]));
    raw.iter()
        .map(|r| plane_core::resolve_member_ref(&rows, r).map(|u| u.to_string()))
        .collect()
}

/// A related id on a listed work item: CE 1.4.2 returns a bare UUID string (`"state": "<uuid>"`,
/// `"assignees": ["<uuid>"]`) unless the field is expanded into an object with `id`.
fn related_id(v: &serde_json::Value) -> Option<&str> {
    v.as_str()
        .or_else(|| v.get("id").and_then(|id| id.as_str()))
}

/// `work-items list --state/--assignee` filter for one item (ids compared case-insensitively).
fn work_item_matches(item: &serde_json::Value, state: Option<&str>, assignees: &[String]) -> bool {
    if let Some(su) = state {
        let item_state = item
            .get("state")
            .and_then(related_id)
            .or_else(|| item.get("state_id").and_then(|id| id.as_str()))
            .unwrap_or("");
        if !item_state.eq_ignore_ascii_case(su) {
            return false;
        }
    }
    if !assignees.is_empty() {
        let assigned: Vec<&str> = item
            .get("assignees")
            .and_then(|a| a.as_array())
            .map(|arr| arr.iter().filter_map(related_id).collect())
            .unwrap_or_default();
        if !assignees
            .iter()
            .any(|au| assigned.iter().any(|a| a.eq_ignore_ascii_case(au.trim())))
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod list_filter_tests {
    use super::work_item_matches;
    use serde_json::json;

    const ME: &str = "7b871681-a7ab-4470-b642-88305803b29b";
    const TODO: &str = "8e235dc1-8aeb-4e27-8967-cbf6f2784d81";

    #[test]
    fn assignee_filter_matches_plain_uuid_strings_and_objects() {
        let plain = json!({"assignees": [ME], "state": TODO});
        let expanded = json!({"assignees": [{"id": ME}], "state": {"id": TODO}});
        let other = json!({"assignees": ["someone-else"], "state": TODO});
        let me = vec![ME.to_string()];
        assert!(work_item_matches(&plain, None, &me));
        assert!(work_item_matches(&expanded, None, &me));
        assert!(!work_item_matches(&other, None, &me));
        assert!(work_item_matches(&plain, None, &[ME.to_uppercase()]));
    }

    #[test]
    fn assignee_names_resolve_through_members_and_unknown_names_fail() {
        let _g = crate::plane_env_files::lock_env();
        let mut server = mockito::Server::new();
        let members = server
            .mock("GET", "/api/v1/workspaces/ws/members/")
            .with_body(format!(
                r#"[{{"id":"{ME}","display_name":"alice","email":"alice@example.com"}}]"#
            ))
            .expect(2)
            .create();
        let m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        // UUIDs need no lookup.
        let ids = super::resolve_assignee_filters(&m.api, &[ME.to_string()]).unwrap();
        assert_eq!(ids, vec![ME.to_string()]);
        let ids = super::resolve_assignee_filters(&m.api, &["Alice".to_string()]).unwrap();
        assert_eq!(ids, vec![ME.to_string()]);
        let e = super::resolve_assignee_filters(&m.api, &["nobody".to_string()]).unwrap_err();
        assert_eq!(e.error_code(), "validation_error");
        assert_eq!(e.exit_kind().code(), 2);
        members.assert();
    }

    #[test]
    fn state_filter_matches_plain_uuid_strings_and_objects() {
        let plain = json!({"state": TODO});
        let expanded = json!({"state": {"id": TODO}});
        let legacy = json!({"state_id": TODO});
        for item in [&plain, &expanded, &legacy] {
            assert!(work_item_matches(item, Some(TODO), &[]));
            assert!(!work_item_matches(item, Some(ME), &[]));
        }
    }
}
