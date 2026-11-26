use ratatui::style::{Color, Modifier, Style};

/// Terminal styling; respects `NO_COLOR` (non-empty env disables ANSI colors).
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub selected: Style,
    pub normal: Style,
    pub status: Style,
    pub warn: Style,
    pub accent: Style,
    pub muted: Style,
    pub state_group_backlog: Style,
    pub state_group_started: Style,
    pub state_group_done: Style,
    pub state_group_cancelled: Style,
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

impl Theme {
    pub fn from_env() -> Self {
        if no_color() {
            return Self::monochrome();
        }
        Self {
            selected: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            normal: Style::default(),
            status: Style::default().fg(Color::Cyan),
            warn: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            accent: Style::default().fg(Color::Magenta),
            muted: Style::default().fg(Color::DarkGray),
            state_group_backlog: Style::default().fg(Color::Blue),
            state_group_started: Style::default().fg(Color::Cyan),
            state_group_done: Style::default().fg(Color::Green),
            state_group_cancelled: Style::default().fg(Color::DarkGray),
        }
    }

    fn monochrome() -> Self {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        Self {
            selected: bold,
            normal: Style::default(),
            status: Style::default(),
            warn: bold,
            accent: Style::default(),
            muted: Style::default(),
            state_group_backlog: Style::default(),
            state_group_started: Style::default(),
            state_group_done: Style::default(),
            state_group_cancelled: Style::default(),
        }
    }

    /// Style for workflow state text (P+R: group hints when color is on).
    pub fn state_style(&self, group: Option<&str>) -> Style {
        if no_color() {
            return Style::default();
        }
        let g = group.unwrap_or("").to_ascii_lowercase();
        if g.contains("backlog") || g.contains("unstarted") || g.contains("todo") {
            self.state_group_backlog
        } else if g.contains("started") || g.contains("progress") || g.contains("doing") {
            self.state_group_started
        } else if g.contains("done") || g.contains("complete") || g.contains("closed") {
            self.state_group_done
        } else if g.contains("cancel") {
            self.state_group_cancelled
        } else {
            self.normal
        }
    }
}
