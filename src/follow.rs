//! Spaces that follow directories: a space pinned to a folder takes the
//! shells that change into it, and each repo declared in settings.json is
//! held by one space: the space already in it, pinned there, else a new one
//! the first time the focused shell goes into it. Paths belong to their
//! machine: another machine's are compared as written.

use crate::model::{Model, Space};
use crate::settings::Repo;
use std::collections::HashMap;

/// Where a shell that just changed folder goes.
#[derive(Debug, PartialEq)]
pub enum Follow {
    /// Into this space, as a new tab.
    Join(String),
    /// Its own space already sits in this declared repo: pin it there.
    Pin(String),
    /// Another space already sits in this declared repo: pin it there, and
    /// go into it as a new tab.
    Adopt(String, String),
    /// Into a new space pinned to this declared repo.
    Found(String),
}

/// A folder as written in settings.json: `~` is this computer's home.
pub fn expand(path: &str, home: &str) -> String {
    let path = path.trim();
    match path.strip_prefix('~') {
        Some("") => home.to_owned(),
        Some(rest) if rest.starts_with(['/', '\\']) => format!("{}{rest}", home.trim_end_matches(['/', '\\'])),
        _ => path.to_owned(),
    }
}

/// A full path on any machine: `/srv`, `C:\src`, `\\server\share`.
pub fn absolute(path: &str) -> bool {
    path.starts_with(['/', '\\']) || path.as_bytes().get(1) == Some(&b':')
}

fn parts(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|s| !s.is_empty() && *s != ".").collect()
}

/// Symlinks resolved, for a folder on this computer that exists.
fn real(path: &str, local: bool) -> String {
    if !local {
        return path.to_owned();
    }
    std::fs::canonicalize(path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_owned())
}

/// How deep `pinned` is, when `cwd` is it or inside it. Whole names only:
/// `/ws` doesn't hold `/ws-worktrees`.
pub fn claims(pinned: &str, cwd: &str, local: bool) -> Option<usize> {
    let (pinned, cwd) = (real(pinned, local), real(cwd, local));
    let (p, c) = (parts(&pinned), parts(&cwd));
    (c.len() >= p.len() && c[..p.len()] == p[..]).then_some(p.len())
}

/// The same folder, symlinks resolved.
pub fn same(a: &str, b: &str, local: bool) -> bool {
    parts(&real(a, local)) == parts(&real(b, local))
}

fn machine_of(r: &Repo) -> Option<&str> {
    r.machine.as_deref().filter(|m| !m.is_empty())
}

/// The declared repo `cwd` is in, on `machine` (None: this computer). The
/// deepest entry wins; a `children` entry stands for each folder directly
/// inside it, never itself.
pub fn declared(cwd: &str, repos: &[Repo], machine: Option<&str>, home: &str) -> Option<String> {
    let local = machine.is_none();
    let mut best: Option<(String, usize)> = None;
    for r in repos.iter().filter(|r| machine_of(r) == machine) {
        let path = if local { expand(&r.path, home) } else { r.path.trim().to_owned() };
        if path.is_empty() {
            continue;
        }
        let Some(depth) = claims(&path, cwd, local) else { continue };
        let found = if r.children {
            let inside = real(cwd, local);
            let Some(child) = parts(&inside).get(depth).map(|s| s.to_string()) else { continue };
            let base = real(&path, local);
            let sep = if base.contains('\\') && !base.contains('/') { '\\' } else { '/' };
            (format!("{}{sep}{child}", base.trim_end_matches(['/', '\\'])), depth + 1)
        } else {
            (path, depth)
        };
        if best.as_ref().is_none_or(|(_, d)| found.1 > *d) {
            best = Some(found);
        }
    }
    best.map(|(path, _)| path)
}

/// Where a shell in `from` that just changed to `cwd` goes. The deepest
/// pinned space on its machine that holds `cwd` takes it, unless its own
/// space's pin is as deep: two spaces pinned to one folder never trade a shell
/// back and forth. Otherwise a declared repo is held by the unpinned space
/// already in it (its first terminal's folder, from `firsts`, which leaves
/// out the shell itself and so lacks `from` when the shell is alone): its own
/// space before another, a lone shell's space only when no other is. Only the
/// focused shell (`focused`) starts a new space for a repo.
pub fn decide(model: &Model, from: &str, cwd: &str, focused: bool, firsts: &HashMap<String, String>, repos: &[Repo], home: &str) -> Option<Follow> {
    let source = model.space(from)?;
    let machine = source.machine.as_deref();
    let local = machine.is_none();
    let depth = |p: &Space| p.path.as_deref().and_then(|path| claims(path, cwd, local));
    let own = depth(source);
    let mut best: Option<(&Space, usize)> = None;
    for p in model.spaces.iter().filter(|p| p.id != from && p.machine.as_deref() == machine) {
        if let Some(d) = depth(p)
            && best.is_none_or(|(_, b)| d > b)
        {
            best = Some((p, d));
        }
    }
    if let Some((p, d)) = best {
        return own.is_none_or(|o| o < d).then(|| Follow::Join(p.id.clone()));
    }
    if own.is_some() {
        return None;
    }
    let repo = declared(cwd, repos, machine, home)?;
    let unpinned_in_repo = |p: &Space| p.path.is_none() && firsts.get(&p.id).is_some_and(|f| claims(&repo, f, local).is_some());
    if unpinned_in_repo(source) {
        return Some(Follow::Pin(repo));
    }
    if let Some(p) = model.spaces.iter().find(|p| p.id != from && p.machine.as_deref() == machine && unpinned_in_repo(p)) {
        return Some(Follow::Adopt(p.id.clone(), repo));
    }
    if source.path.is_none() && !firsts.contains_key(from) {
        return Some(Follow::Pin(repo));
    }
    focused.then_some(Follow::Found(repo))
}

/// A space's pin after the pin key at `cwd`: pinned there, or cleared when
/// it already was.
pub fn toggled_pin(current: Option<&str>, cwd: &str, local: bool) -> Option<String> {
    (!current.is_some_and(|p| same(p, cwd, local))).then(|| cwd.to_owned())
}

/// Declares `path` on `machine`, or undeclares it when an entry (not a
/// projects folder) already names it. Returns whether it's declared now.
pub fn toggle_repo(repos: &mut Vec<Repo>, path: &str, machine: Option<&str>, home: &str) -> bool {
    let local = machine.is_none();
    let before = repos.len();
    repos.retain(|r| {
        let written = if local { expand(&r.path, home) } else { r.path.trim().to_owned() };
        r.children || machine_of(r) != machine || !same(&written, path, local)
    });
    if repos.len() != before {
        return false;
    }
    repos.push(Repo { path: path.to_owned(), children: false, machine: machine.map(str::to_owned) });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Tab;

    fn repo(path: &str) -> Repo {
        Repo { path: path.into(), children: false, machine: None }
    }

    fn projects(path: &str) -> Repo {
        Repo { path: path.into(), children: true, machine: None }
    }

    fn firsts(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(p, f)| (p.to_string(), f.to_string())).collect()
    }

    fn spaces(pins: &[(&str, Option<&str>, Option<&str>)]) -> Model {
        let mut m = Model::default();
        for (i, (id, pin, machine)) in pins.iter().enumerate() {
            m.add_space(id, "", "/", Tab::new(&format!("t{i}"), "", &format!("p{i}"), &format!("a{i}")));
            let p = m.space_mut(id).unwrap();
            p.path = pin.map(str::to_owned);
            p.machine = machine.map(str::to_owned);
        }
        m
    }

    #[test]
    fn a_pin_holds_its_folder_and_below_but_not_a_sibling_sharing_a_prefix() {
        assert_eq!(claims("/ws", "/ws", false), Some(1));
        assert_eq!(claims("/ws", "/ws/src/app", false), Some(1));
        assert_eq!(claims("/ws/", "/ws/src", false), Some(1));
        assert_eq!(claims("/ws", "/ws-worktrees/a", false), None);
        assert_eq!(claims("/ws/src", "/ws", false), None);
    }

    #[test]
    fn a_symlinked_checkout_matches_its_real_folder_here() {
        let base = std::env::temp_dir().join(format!("herdr-follow-{}", std::process::id()));
        let (real_dir, link) = (base.join("real"), base.join("link"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(real_dir.join("src")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real_dir, &link).unwrap();
        let (link, inside) = (link.to_string_lossy().into_owned(), real_dir.join("src").to_string_lossy().into_owned());
        assert!(claims(&link, &inside, true).is_some());
        assert_eq!(claims(&link, &inside, false), None, "another machine's paths are compared as written");
        assert_eq!(toggled_pin(Some(&link), &real_dir.to_string_lossy(), true), None);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn only_full_paths_count_as_pins() {
        assert!(absolute("/srv/api") && absolute("C:\\src") && absolute("\\\\host\\share"));
        assert!(!absolute(".") && !absolute("../api") && !absolute("code") && !absolute("~/code") && !absolute(""));
    }

    #[test]
    fn home_expands_only_a_leading_tilde() {
        assert_eq!(expand("~/code/herdr", "/home/u"), "/home/u/code/herdr");
        assert_eq!(expand("~", "/home/u/"), "/home/u/");
        assert_eq!(expand("/srv/~x", "/home/u"), "/srv/~x");
        assert_eq!(expand("~other/x", "/home/u"), "~other/x");
    }

    #[test]
    fn the_deepest_declaration_wins_and_a_projects_folder_stands_for_each_child() {
        let repos = [projects("/projects"), repo("/projects/budget/wt"), repo("/repos/herdr")];
        assert_eq!(declared("/projects/budget/src", &repos, None, "/h").as_deref(), Some("/projects/budget"));
        assert_eq!(declared("/projects/budget/wt/src", &repos, None, "/h").as_deref(), Some("/projects/budget/wt"));
        assert_eq!(declared("/projects", &repos, None, "/h"), None, "a projects folder isn't a repo itself");
        assert_eq!(declared("/repos/herdr-old", &repos, None, "/h"), None);
        assert_eq!(declared("/repos/herdr/src", &repos, None, "/h").as_deref(), Some("/repos/herdr"));
        assert_eq!(declared("/projects/budget/src", &[repo("/"), projects("/projects")], None, "/h").as_deref(), Some("/projects/budget"));
    }

    #[test]
    fn declarations_belong_to_their_machine() {
        let repos = [Repo { path: "/srv/api".into(), children: false, machine: Some("vps".into()) }, repo("~/code")];
        assert_eq!(declared("/srv/api/src", &repos, None, "/h"), None);
        assert_eq!(declared("/srv/api/src", &repos, Some("vps"), "/h").as_deref(), Some("/srv/api"));
        assert_eq!(declared("/h/code/x", &repos, None, "/h").as_deref(), Some("/h/code"));
        assert_eq!(declared("/h/code/x", &repos, Some("vps"), "/h"), None);
    }

    #[test]
    fn a_shell_joins_the_deepest_pinned_space_on_its_own_machine() {
        let m = spaces(&[("here", None, None), ("repo", Some("/r"), None), ("wt", Some("/r/wt"), None), ("far", Some("/r/wt"), Some("vps"))]);
        assert_eq!(decide(&m, "here", "/r/wt/src", false, &firsts(&[("here", "/")]), &[], "/h"), Some(Follow::Join("wt".into())));
        assert_eq!(decide(&m, "here", "/r/src", false, &firsts(&[("here", "/")]), &[], "/h"), Some(Follow::Join("repo".into())));
        assert_eq!(decide(&m, "here", "/elsewhere", true, &firsts(&[("here", "/")]), &[], "/h"), None);
    }

    #[test]
    fn a_shell_stays_when_its_own_pin_is_as_deep_so_twins_never_trade_it() {
        let m = spaces(&[("a", Some("/r"), None), ("b", Some("/r"), None), ("wt", Some("/r/wt"), None)]);
        assert_eq!(decide(&m, "a", "/r/src", true, &firsts(&[("a", "/r")]), &[repo("/r")], "/h"), None);
        assert_eq!(decide(&m, "b", "/r", true, &firsts(&[("b", "/r")]), &[], "/h"), None);
        assert_eq!(decide(&m, "a", "/r/wt", true, &firsts(&[("a", "/r")]), &[], "/h"), Some(Follow::Join("wt".into())), "a deeper pin still wins");
    }

    #[test]
    fn a_shell_on_another_machine_never_goes_to_a_space_here() {
        let m = spaces(&[("far", None, Some("vps")), ("near", Some("/r"), None)]);
        assert_eq!(decide(&m, "far", "/r/src", true, &firsts(&[("far", "/")]), &[repo("/r")], "/h"), None);
    }

    #[test]
    fn only_the_focused_shell_founds_a_space_for_a_declared_repo() {
        let m = spaces(&[("here", None, None)]);
        let repos = [repo("/r")];
        assert_eq!(decide(&m, "here", "/r/src", false, &firsts(&[("here", "/other")]), &repos, "/h"), None);
        assert_eq!(decide(&m, "here", "/r/src", true, &firsts(&[("here", "/other")]), &repos, "/h"), Some(Follow::Found("/r".into())));
    }

    #[test]
    fn a_space_already_in_the_declared_repo_is_pinned_rather_than_twinned() {
        let m = spaces(&[("here", None, None)]);
        assert_eq!(decide(&m, "here", "/r/src", true, &firsts(&[("here", "/r/docs")]), &[repo("/r")], "/h"), Some(Follow::Pin("/r".into())));
        let pinned_elsewhere_inside = spaces(&[("here", Some("/r"), None)]);
        assert_eq!(decide(&pinned_elsewhere_inside, "here", "/r/src", true, &firsts(&[("here", "/r")]), &[repo("/r")], "/h"), None, "its own pin holds it");
    }

    #[test]
    fn a_declared_repo_already_open_elsewhere_takes_the_shell_instead_of_a_twin() {
        let m = spaces(&[("scratch", None, None), ("app", None, None), ("pinned", Some("/q"), None), ("far", None, Some("vps"))]);
        let repos = [repo("/r")];
        let open = firsts(&[("scratch", "/notes"), ("app", "/r/src"), ("pinned", "/r"), ("far", "/r")]);
        assert_eq!(decide(&m, "scratch", "/r/docs", false, &open, &repos, "/h"), Some(Follow::Adopt("app".into(), "/r".into())), "any shell, focused or not");
        let none_open = firsts(&[("scratch", "/notes"), ("pinned", "/r"), ("far", "/r")]);
        assert_eq!(decide(&m, "scratch", "/r/docs", true, &none_open, &repos, "/h"), Some(Follow::Found("/r".into())), "a space pinned elsewhere or on another machine isn't the repo's");
        assert_eq!(decide(&m, "scratch", "/r/docs", false, &none_open, &repos, "/h"), None);
    }

    #[test]
    fn a_lone_shell_joins_the_repos_space_before_pinning_its_own() {
        let m = spaces(&[("mine", None, None), ("app", None, None)]);
        let repos = [repo("/r")];
        assert_eq!(decide(&m, "mine", "/r/src", true, &firsts(&[("app", "/r")]), &repos, "/h"), Some(Follow::Adopt("app".into(), "/r".into())));
        assert_eq!(decide(&m, "mine", "/r/src", false, &firsts(&[("app", "/elsewhere")]), &repos, "/h"), Some(Follow::Pin("/r".into())));
        assert_eq!(decide(&m, "mine", "/r/src", true, &firsts(&[("mine", "/r"), ("app", "/r")]), &repos, "/h"), Some(Follow::Pin("/r".into())), "its own space's other shells keep it");
    }

    #[test]
    fn the_pin_key_pins_then_clears_at_the_same_folder() {
        assert_eq!(toggled_pin(None, "/ws", false).as_deref(), Some("/ws"));
        assert_eq!(toggled_pin(Some("/ws/"), "/ws", false), None);
        assert_eq!(toggled_pin(Some("/ws"), "/other", false).as_deref(), Some("/other"));
    }

    #[test]
    fn declaring_toggles_only_a_plain_entry_on_the_same_machine() {
        let mut repos = vec![projects("/p"), Repo { path: "/r".into(), children: false, machine: Some("vps".into()) }, repo("~/r")];
        assert!(!toggle_repo(&mut repos, "/h/r", None, "/h"));
        assert_eq!(repos.len(), 2);
        assert!(toggle_repo(&mut repos, "/p", None, "/h"), "a projects folder entry is not the repo itself");
        assert_eq!(repos.last(), Some(&repo("/p")));
        assert!(toggle_repo(&mut repos, "/r", None, "/h"), "vps's /r is another folder");
        assert!(!toggle_repo(&mut repos, "/r", Some("vps"), "/h"));
        assert_eq!(repos, vec![projects("/p"), repo("/p"), repo("/r")]);
    }
}
