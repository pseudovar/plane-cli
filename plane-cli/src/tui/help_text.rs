//! Static help copy for `?` overlays (hybrid: contextual then global).

use crate::tui::app::Screen;

pub fn context(screen: Screen) -> &'static str {
    match screen {
        Screen::Projects => {
            "Projects (this screen)\n\
             j / ↓  next project · k / ↑  previous\n\
             Enter  open work items for project\n\
             m  modules for project\n\
             r  refresh project list · s  env wizard · i  skill installer\n\
             ?  this help · ? again  all keys · q  quit"
        }
        Screen::WorkItems => {
            "Work items (this screen)\n\
             j / ↓  next row · k / ↑  previous · Enter  full detail + comments\n\
             n / p  next/prev page (browse) · m  modules · /  search · v  state distribution strip\n\
             Shift+G  activity sparkline (focused row, best-effort)\n\
             Shift+S / Shift+P / Shift+A  state · priority · assignee · e title · w description · Shift+D delete\n\
             Esc  clear search results (after a search) · b  back to projects\n\
             ?  this help · ? again  all keys · q  quit\n\
             Search: type query · Ctrl-G  toggle workspace vs project scope"
        }
        Screen::Modules => {
            "Modules (this screen)\n\
             j / k  move · Enter  issues in module · b  back\n\
             ?  this help · ? again  all keys · q  quit"
        }
        Screen::ModuleIssues => {
            "Module issues (this screen)\n\
             j / k  move · Enter  detail · n / p  pages · b  back to modules\n\
             Shift+S / Shift+P / Shift+A  state · priority · assignee · e title · w description · Shift+D delete\n\
             ?  this help · ? again  all keys · q  quit"
        }
        Screen::Detail => {
            "Work item detail\n\
             [ / ] or PgUp/PgDn  scroll comments · c  new comment (type · Enter send · Esc cancel)\n\
             Shift+S / Shift+P / Shift+A  state · priority · assignee · e title · w description · Shift+D delete\n\
             b  back to list · ?  help · q  quit"
        }
    }
}

pub const GLOBAL: &str = "\
plane-cli TUI — all keys\n\
\n\
Global\n\
  q       quit\n\
  ?       screen help (press again for this list)\n\
  a       when help is open: jump to this global list\n\
  Esc     close help overlay; cancel comment/search draft; clear search results (work items)\n\
  r       refresh projects (project list)\n\
  s       run API env setup wizard (leaves TUI briefly)\n\
  i       run agent skill installer wizard\n\
\n\
Projects\n\
  j k     move selection\n\
  Enter   open work items\n\
  m       modules\n\
\n\
Work items\n\
  j k     move · Enter detail\n\
  n p     next/prev page (normal browse; search is one page)\n\
  m       modules (same project)\n\
  v       toggle state distribution strip (loaded pages only)\n\
  Shift+G activity sparkline for focused item (best-effort label)\n\
  Shift+S / Shift+P / Shift+A  edit state / priority / assignee (modal)\n\
  e w     edit title / description (modal)\n\
  Shift+D delete (y/n confirm)\n\
  /       start search (Enter run · Esc cancel draft)\n\
  Ctrl-G  while searching: toggle project-only vs whole workspace\n\
  b       back to projects\n\
\n\
Modules / module issues\n\
  As on screen footers; b goes back. On module issues: same mutation keys as work items (Shift+S/P/A, e, w, Shift+D).\n\
\n\
Detail\n\
  c       comment · [ ] scroll comments · b back\n\
  Same Shift+S/P/A · e/w · Shift+D mutations as on work items when detail is open\n\
";
