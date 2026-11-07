//! The app's shared world, the same for every layout: spaces, each with
//! tabs (named pieces of work), each tab holding its terminals and
//! how they're split. Plus which pane has focus and agent attention (done vs
//! seen) with Herdr's rollup. Pure data; the window drives it.

use crate::arrangement::{Arrangement, arrange};
use crate::layout::{Axis, Content, Node};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

/// A named piece of work in a space: its terminals and how they're split.
#[derive(Clone, Debug)]
pub struct Tab {
    pub id: String,
    pub name: String,
    /// The arrangement: a row/column tree of terminal blocks. Its id is the tab's.
    pub root: Node,
    pub focused: Option<String>,
    /// Panes focused before, most recent last: closing the focused pane goes back.
    pub before: Vec<String>,
    pub zoomed: Option<String>,
    /// Put away (ms since the epoch): out of the lists, still running. Comes
    /// back when it's shown or one of its agents needs the person.
    pub settled: Option<u64>,
    /// None: split by hand. Some: rebuilt evenly from the pane order whenever
    /// a pane comes or goes.
    pub arrangement: Option<Arrangement>,
    /// The tab it was made from (a tab of the same space): it reads
    /// `parent › child` and opens beside its siblings.
    pub parent: Option<String>,
}

/// A place to work, plus the folder its new terminals start in.
#[derive(Clone, Debug)]
pub struct Space {
    pub id: String,
    /// The name someone gave it; empty: named after where its first
    /// terminal is (Herdr's way, `Model::space_name`).
    pub name: String,
    pub folder: String,
    /// The machine its terminals run on (`hc machine ls`); None: this one.
    pub machine: Option<String>,
    /// The folder it's pinned to, on its machine: a shell that changes into
    /// it (or below) moves here (follow.rs).
    pub path: Option<String>,
    pub tabs: Vec<Tab>,
    /// The selected tab.
    pub active: Option<String>,
    /// Terminals floating over every tab of the space, arranged like a tab.
    pub floats: Option<Tab>,
    /// This screen shows the floats.
    pub floats_shown: bool,
    /// The floats hold the keyboard (only while shown).
    pub floats_focused: bool,
}

/// A closed space, so opening it again doesn't ask for its name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recent {
    pub name: String,
    pub folder: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Alert {
    /// Needs the person (blocked or error).
    Blocked,
    /// Finished while the person wasn't looking.
    Done,
}

/// What core and the user's attention say about one terminal.
#[derive(Clone, Debug, Default)]
pub struct Agent {
    /// The program's 7501 word (working, idle, done, blocked, error), if it reports.
    pub word: Option<String>,
    /// The agent's name from its 7501 report (`app`), e.g. "claude-code".
    pub app: Option<String>,
    /// Whether the person has seen it since it last stopped working.
    pub seen: bool,
    /// Core's turn counter: goes up each time it stops working.
    pub turn: u64,
    /// The last turn a person looked at on any client (`herdr.seen`, docs/labels.md).
    /// None: no client shares it yet, so `seen` decides.
    pub seen_turn: Option<u64>,
    /// Bumped on every state change (for "most recent" ordering).
    pub seq: u64,
    /// When the state last changed, ms since the epoch (0: not since the app started).
    pub since: u64,
}

#[derive(Default)]
pub struct Model {
    pub spaces: Vec<Space>,
    /// The selected space.
    pub active: Option<String>,
    pub recent: Vec<Recent>,
    /// Each pane's live folder, for its display name.
    pub pane_folders: BTreeMap<String, String>,
    pub agents: HashMap<String, Agent>,
    seq: u64,
    /// Where each terminal works now: its repo (worktrees included) and branch.
    pub folder_info: HashMap<String, FolderInfo>,
    /// How new tabs are arranged (None: split by hand).
    pub default_arrangement: Option<Arrangement>,
    /// The tab area's width over its height, for grids (0: not measured yet).
    pub aspect: f32,
    /// Terminals whose panes went with a space that closed on its own; the
    /// window ends them.
    pub doomed: Vec<String>,
}

/// Herdr's attention priority: blocked > done > working > idle > nothing.
pub fn priority(status: Option<&str>) -> u8 {
    match status {
        Some("blocked") => 4,
        Some("done") => 3,
        Some("working") => 2,
        Some("idle") => 1,
        _ => 0,
    }
}

impl Tab {
    /// A tab with one terminal pane.
    pub fn new(id: &str, name: &str, pane_id: &str, address: &str) -> Tab {
        Tab::with_pane(id, name, pane(pane_id, address))
    }

    /// A tab with one pane (a terminal or a mod).
    pub fn with_pane(id: &str, name: &str, pane: Node) -> Tab {
        Tab {
            id: id.to_owned(),
            name: name.to_owned(),
            focused: Some(pane.id.clone()),
            before: Vec::new(),
            root: Node::container(id, Axis::Row, vec![pane]),
            zoomed: None,
            settled: None,
            arrangement: None,
            parent: None,
        }
    }

    /// Focuses a pane and remembers the one it leaves. A zoomed tab shows the
    /// pane it focuses, never a hidden one.
    pub fn focus(&mut self, pane: &str) {
        if self.zoomed.is_some() {
            self.zoomed = Some(pane.to_owned());
        }
        if let Some(old) = self.focused.take().filter(|o| o != pane) {
            self.before.retain(|p| *p != old && p != pane);
            self.before.push(old);
            if self.before.len() > 32 {
                self.before.remove(0);
            }
        }
        self.focused = Some(pane.to_owned());
    }

    /// The focused pane, else the first one.
    pub fn focused_pane(&self) -> Option<String> {
        self.focused.clone().filter(|p| self.root.find(p).is_some()).or_else(|| self.root.panes().first().map(|n| n.id.clone()))
    }

    /// What's drawn: only the zoomed pane when one is zoomed.
    pub fn shown(&self) -> &Node {
        self.zoomed.as_deref().and_then(|z| self.root.find(z)).unwrap_or(&self.root)
    }
}

impl Space {
    pub fn tab(&self, id: &str) -> Option<&Tab> {
        self.tabs.iter().find(|c| c.id == id)
    }

    pub fn shown_floats(&self) -> Option<&Tab> {
        self.floats.as_ref().filter(|_| self.floats_shown)
    }

    /// The tab holding the keyboard: the floats while they have it, else the selected tab.
    pub fn focused_tab(&self) -> Option<&Tab> {
        self.shown_floats().filter(|_| self.floats_focused).or_else(|| self.active_tab())
    }

    /// Where a new child of `parent` goes: after its last descendant.
    fn after_family(&self, parent: &str) -> Option<usize> {
        let mut family = HashSet::from([parent]);
        while let Some(t) = self.tabs.iter().find(|t| !family.contains(t.id.as_str()) && t.parent.as_deref().is_some_and(|p| family.contains(p))) {
            family.insert(&t.id);
        }
        self.tabs.iter().rposition(|t| family.contains(t.id.as_str())).map(|i| i + 1)
    }

    /// Parents that don't hold (a tab gone from the space, itself, a loop) go:
    /// those tabs are top-level.
    pub fn tidy_parents(&mut self) {
        for i in 0..self.tabs.len() {
            let me = self.tabs[i].id.clone();
            let mut seen = HashSet::new();
            let mut up = self.tabs[i].parent.clone();
            while let Some(p) = up {
                if p == me || self.tab(&p).is_none() {
                    self.tabs[i].parent = None;
                    break;
                }
                if !seen.insert(p.clone()) {
                    break;
                }
                up = self.tab(&p).and_then(|t| t.parent.clone());
            }
        }
    }

    /// The selected tab, else the first.
    pub fn active_tab(&self) -> Option<&Tab> {
        self.active.as_deref().and_then(|a| self.tab(a)).or(self.tabs.first())
    }

    fn select(&mut self, ctx: &str) {
        if self.tab(ctx).is_some() {
            self.active = Some(ctx.to_owned());
        }
    }
}

impl Model {
    pub fn fresh_id(&mut self, prefix: &str) -> String {
        use std::hash::{BuildHasher, Hasher};
        let random = || std::collections::hash_map::RandomState::new().build_hasher().finish();
        format!("{prefix}{:016x}{:016x}", random(), random())
    }

    pub fn fresh_id_on(&mut self, prefix: &str, machine: Option<&str>) -> String {
        crate::mirror::qualify(&self.fresh_id(prefix), machine)
    }

    /// Machine blocks have one order on every screen; order inside each stays shared.
    pub fn order_spaces(&mut self) {
        self.spaces.sort_by(|a, b| a.machine.cmp(&b.machine));
    }

    pub fn move_space(&mut self, id: &str, index: usize) -> bool {
        self.order_spaces();
        let Some(from) = self.spaces.iter().position(|s| s.id == id) else { return false };
        let machine = self.spaces[from].machine.clone();
        let first = self.spaces.iter().position(|s| s.machine == machine).unwrap();
        let last = self.spaces.iter().rposition(|s| s.machine == machine).unwrap();
        let to = index.saturating_sub(1).clamp(first, last);
        let space = self.spaces.remove(from);
        self.spaces.insert(to, space);
        true
    }

    pub fn active_index(&self) -> Option<usize> {
        let id = self.active.as_deref()?;
        self.spaces.iter().position(|p| p.id == id).or((!self.spaces.is_empty()).then_some(0))
    }

    pub fn active_space(&self) -> Option<&Space> {
        self.active_index().map(|i| &self.spaces[i])
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_space()?.active_tab()
    }

    pub fn space(&self, id: &str) -> Option<&Space> {
        self.spaces.iter().find(|p| p.id == id)
    }

    pub fn space_mut(&mut self, id: &str) -> Option<&mut Space> {
        self.spaces.iter_mut().find(|p| p.id == id)
    }

    /// Every tab, floats included.
    pub fn tabs(&self) -> impl Iterator<Item = &Tab> {
        self.spaces.iter().flat_map(|p| p.tabs.iter().chain(p.floats.iter()))
    }

    fn tabs_mut(&mut self) -> impl Iterator<Item = &mut Tab> {
        self.spaces.iter_mut().flat_map(|p| p.tabs.iter_mut().chain(p.floats.iter_mut()))
    }

    pub fn is_floats(&self, tab: &str) -> bool {
        self.spaces.iter().any(|p| p.floats.as_ref().is_some_and(|f| f.id == tab))
    }

    pub fn tab(&self, id: &str) -> Option<&Tab> {
        self.tabs().find(|c| c.id == id)
    }

    pub fn tab_mut(&mut self, id: &str) -> Option<&mut Tab> {
        self.tabs_mut().find(|c| c.id == id)
    }

    /// The space holding a tab or a node inside one.
    pub fn space_of(&self, node: &str) -> Option<&Space> {
        self.spaces.iter().find(|p| p.tabs.iter().chain(p.floats.iter()).any(|c| c.id == node || c.root.find(node).is_some()))
    }

    /// The tab holding a node (pane, container).
    pub fn tab_of(&self, node: &str) -> Option<&Tab> {
        self.tabs().find(|c| c.root.find(node).is_some())
    }

    fn tab_of_mut(&mut self, node: &str) -> Option<&mut Tab> {
        self.tabs_mut().find(|c| c.root.find(node).is_some())
    }

    /// Adds a space at the end and selects it.
    /// What a space is called: its name, else its first terminal's checkout
    /// (a worktree's own folder; outside git its folder), as Herdr names workspaces.
    pub fn space_name(&self, p: &Space, home: &str) -> String {
        if !p.name.is_empty() {
            return p.name.clone();
        }
        if let Some(path) = &p.path {
            return folder_name(path, home);
        }
        let first = p.tabs.iter().flat_map(|c| c.root.terminals()).next();
        let folder = first
            .and_then(|n| n.terminal().and_then(|a| self.folder_info.get(a)).map(|f| f.top.clone()).or_else(|| self.pane_folders.get(&n.id).cloned()))
            .unwrap_or_else(|| p.folder.clone());
        folder_name(&folder, home)
    }

    /// What a tab is called: its name, else its place in its space (1, 2, ...).
    pub fn tab_name(p: &Space, c: &Tab) -> String {
        if !c.name.is_empty() {
            return c.name.clone();
        }
        (p.tabs.iter().position(|x| x.id == c.id).unwrap_or(0) + 1).to_string()
    }

    /// The tab a new tab goes under: `parent` (a tab id; "": none), else the
    /// tab of the terminal `caller` (the one that ran `herdr-app do`) when
    /// it's an ordinary tab in `space` (the space the new tab goes to, if given).
    pub fn new_tab_parent(&self, parent: Option<&str>, caller: Option<&str>, space: Option<&str>) -> Result<Option<String>, String> {
        let in_space = |tab: &str| space.is_none_or(|s| self.space_of(tab).is_some_and(|p| p.id == s));
        match parent {
            Some("") => Ok(None),
            Some(id) if self.tab(id).is_none() || self.is_floats(id) => Err(format!("no tab {id:?} to be the parent")),
            Some(id) if !in_space(id) => Err(format!("tab {id:?} isn't in space {:?}", space.unwrap_or_default())),
            Some(id) => Ok(Some(id.to_owned())),
            None => Ok(caller
                .and_then(|t| self.pane_of(t))
                .and_then(|p| self.tab_of(&p))
                .map(|t| t.id.clone())
                .filter(|t| !self.is_floats(t) && in_space(t))),
        }
    }

    /// Makes a tab a child of `parent`, a tab of its space that isn't its own
    /// descendant: it moves after the parent's family.
    pub fn nest(&mut self, tab: &str, parent: &str) -> bool {
        let Some(p) = self.spaces.iter_mut().find(|p| p.tab(tab).is_some()) else { return false };
        let mut up = Some(parent.to_owned());
        while let Some(id) = up {
            if id == tab {
                return false;
            }
            up = p.tab(&id).and_then(|t| t.parent.clone());
        }
        if p.tab(parent).is_none() {
            return false;
        }
        let mut t = p.tabs.remove(p.tabs.iter().position(|t| t.id == tab).unwrap());
        t.parent = Some(parent.to_owned());
        let at = p.after_family(parent).unwrap_or(p.tabs.len());
        p.tabs.insert(at, t);
        true
    }

    /// What a tab reads as where its family shows: `parent › child`.
    pub fn tab_label(p: &Space, c: &Tab) -> String {
        match c.parent.as_deref().and_then(|id| p.tab(id)) {
            Some(parent) => format!("{} › {}", Self::tab_name(p, parent), Self::tab_name(p, c)),
            None => Self::tab_name(p, c),
        }
    }

    pub fn add_space(&mut self, id: &str, name: &str, folder: &str, mut ctx: Tab) {
        ctx.arrangement = ctx.arrangement.or(self.default_arrangement);
        let active = Some(ctx.id.clone());
        self.spaces.push(Space {
            id: id.to_owned(),
            name: name.to_owned(),
            folder: folder.to_owned(),
            machine: None,
            path: None,
            tabs: vec![ctx],
            active,
            floats: None,
            floats_shown: false,
            floats_focused: false,
        });
        self.recent.retain(|r| r.name != name);
        self.active = Some(id.to_owned());
    }

    /// Adds a tab after the space's selected one and selects it (not the space).
    pub fn add_tab(&mut self, space: &str, mut ctx: Tab) -> bool {
        ctx.arrangement = ctx.arrangement.or(self.default_arrangement);
        let Some(p) = self.space_mut(space) else { return false };
        let at = p.active.as_deref().and_then(|a| p.tabs.iter().position(|c| c.id == a)).map_or(p.tabs.len(), |i| i + 1);
        let id = ctx.id.clone();
        p.tabs.insert(at, ctx);
        p.active = Some(id);
        true
    }

    /// Splits `target` (a pane) with a new terminal pane and focuses it.
    pub fn split(&mut self, target: &str, axis: Axis, new_pane: &str, address: &str) -> bool {
        self.split_with(target, axis, pane(new_pane, address))
    }

    /// Splits a pane with any block: a terminal, or a mod (a viewer beside the shell).
    pub fn split_with(&mut self, target: &str, axis: Axis, block: Node) -> bool {
        let machine = self.space_of(target).and_then(|s| s.machine.clone());
        let container = self.fresh_id_on("s", machine.as_deref());
        let new_pane = block.id.clone();
        let Some(c) = self.tab_of_mut(target) else { return false };
        if !c.root.split(target, axis, block, container) {
            return false;
        }
        c.arrangement = None;
        c.zoomed = None;
        c.focus(&new_pane);
        true
    }

    /// Adds a pane after `target` the way its tab is laid out: in its
    /// arrangement, else split to the right. Focuses it.
    pub fn add_pane(&mut self, target: &str, block: Node) -> bool {
        let Some(tab) = self.tab_of(target) else { return false };
        if tab.arrangement.is_none() {
            return self.split_with(target, Axis::Row, block);
        }
        let mut panes: Vec<Node> = tab.root.panes().into_iter().cloned().collect();
        let Some(at) = panes.iter().position(|p| p.id == target) else { return false };
        let new_pane = block.id.clone();
        panes.insert(at + 1, block);
        let id = tab.id.clone();
        self.rebuild(&id, panes);
        let c = self.tab_mut(&id).unwrap();
        c.zoomed = None;
        c.focus(&new_pane);
        true
    }

    /// Lays a tab out in an arrangement (None: keep the shape, split by hand from now on).
    pub fn arrange(&mut self, tab: &str, arrangement: Option<Arrangement>) -> bool {
        let Some(c) = self.tab_mut(tab) else { return false };
        c.arrangement = arrangement;
        self.reflow(tab);
        true
    }

    /// Rebuilds an arranged tab from its panes in order; a hand-split one stays.
    pub fn reflow(&mut self, tab: &str) {
        let Some(panes) = self.tab(tab).map(|c| c.root.panes().into_iter().cloned().collect::<Vec<_>>()) else { return };
        self.rebuild(tab, panes);
    }

    fn rebuild(&mut self, tab: &str, panes: Vec<Node>) {
        let aspect = if self.aspect > 0.0 { self.aspect } else { 1.6 };
        let Some(c) = self.tab_mut(tab) else { return };
        let Some(kind) = c.arrangement else { return };
        if panes.is_empty() {
            return;
        }
        c.root = arrange(kind, &c.root.id, panes, aspect);
    }

    /// Takes a terminal pane out of its tab for another space: never a float,
    /// never into its own space. A tab or space it leaves empty closes.
    fn take_pane(&mut self, pane: &str, into: &str) -> Option<Node> {
        let tab = self.tab_of(pane)?;
        if self.is_floats(&tab.id) || self.space_of(pane)?.id == into {
            return None;
        }
        let mut node = tab.root.find(pane).filter(|n| n.terminal().is_some())?.clone();
        node.size = None;
        self.remove_pane(pane);
        Some(node)
    }

    /// Moves a terminal pane to a new tab `tab` in `space` (follow.rs). With
    /// `focus` it's shown and focused; without, what each space shows stays.
    pub fn move_to_space(&mut self, pane: &str, space: &str, tab: &str, focus: bool) -> bool {
        let Some(keep) = self.space(space).map(|p| p.active.clone()) else { return false };
        let Some(node) = self.take_pane(pane, space) else { return false };
        self.add_tab(space, Tab::with_pane(tab, "", node));
        if focus {
            return self.focus_pane(pane);
        }
        self.space_mut(space).unwrap().active = keep;
        true
    }

    /// Moves a terminal pane into a new space on its machine, pinned to `path`.
    pub fn move_to_new_space(&mut self, pane: &str, space: &str, tab: &str, folder: &str, path: Option<&str>, focus: bool) -> bool {
        if self.space(space).is_some() {
            return false;
        }
        let machine = self.space_of(pane).and_then(|p| p.machine.clone());
        let Some(node) = self.take_pane(pane, space) else { return false };
        let shown = self.active.clone();
        self.add_space(space, "", folder, Tab::with_pane(tab, "", node));
        let p = self.space_mut(space).unwrap();
        p.machine = machine;
        p.path = path.map(str::to_owned);
        if focus {
            return self.focus_pane(pane);
        }
        self.active = shown.filter(|s| self.space(s).is_some()).or(Some(space.to_owned()));
        true
    }

    /// Adds a pane to a space's floats, making them when it has none, and
    /// shows and focuses them. `layer`: the id new floats take.
    pub fn add_float(&mut self, space: &str, layer: &str, block: Node) -> bool {
        let Some(p) = self.space_mut(space) else { return false };
        let target = p.floats.as_ref().and_then(|f| f.focused_pane());
        p.floats_shown = true;
        p.floats_focused = true;
        let Some(target) = target else {
            let mut floats = Tab::with_pane(layer, "", block);
            floats.arrangement = Some(Arrangement::Stacked);
            p.floats = Some(floats);
            self.active = Some(space.to_owned());
            return true;
        };
        self.active = Some(space.to_owned());
        self.add_pane(&target, block)
    }

    /// Rearranges existing panes in one tab without changing their terminal addresses.
    /// No edge swaps their places; an edge places the source beside the target.
    pub fn move_pane(&mut self, source: &str, target: &str, edge: Option<crate::layout::Edge>) -> bool {
        if source == target {
            return false;
        }
        let Some(tab) = self.tab_of(source) else { return false };
        if !tab.root.find(source).is_some_and(Node::is_pane) || !tab.root.find(target).is_some_and(Node::is_pane) {
            return false;
        }
        let mut root = tab.root.clone();
        if let Some(edge) = edge {
            use crate::layout::Edge;
            let mut pane = root.remove(source).unwrap();
            pane.size = None;
            let axis = if matches!(edge, Edge::Left | Edge::Right) { Axis::Row } else { Axis::Column };
            let machine = self.space_of(source).and_then(|s| s.machine.clone());
            let container = self.fresh_id_on("s", machine.as_deref());
            if !root.split(target, axis, pane, container) {
                return false;
            }
            if matches!(edge, Edge::Left | Edge::Top) {
                root.swap(source, target);
            }
        } else if !root.swap(source, target) {
            return false;
        }
        let tab = self.tab_of_mut(source).unwrap();
        tab.root = root;
        if edge.is_some() {
            tab.arrangement = None;
        }
        tab.focus(source);
        true
    }

    pub fn swap_panes(&mut self, pane: &str, target: &str) -> Result<(), String> {
        let is_pane = |m: &Model, id: &str| m.tab_of(id).and_then(|c| c.root.find(id)).is_some_and(Node::is_pane);
        if !is_pane(self, pane) {
            return Err(format!("no pane {pane:?}"));
        }
        if !is_pane(self, target) {
            return Err(format!("no target pane {target:?}"));
        }
        if pane == target {
            return Err("pane.swap target is the pane itself".into());
        }
        let Some(c) = self.tab_of_mut(pane) else { return Err(format!("no pane {pane:?}")) };
        if c.root.find(target).is_none() {
            return Err(format!("pane.swap target {target:?} is in another tab"));
        }
        c.root.swap(pane, target);
        Ok(())
    }

    /// Removes a pane. Returns what else closed: the tab, and the space if it was its last.
    /// The last float closing takes the floats away, never the space.
    pub fn remove_pane(&mut self, pane_id: &str) -> (bool, bool) {
        let Some(c) = self.tab_of_mut(pane_id) else { return (false, false) };
        c.root.remove(pane_id);
        if c.zoomed.as_deref() == Some(pane_id) {
            c.zoomed = None;
        }
        if !c.root.panes().is_empty() {
            c.before.retain(|p| p != pane_id);
            if c.focused.as_deref() == Some(pane_id) {
                // Back to the pane focused before it that's still here, else the first.
                let here: Vec<String> = c.root.panes().iter().map(|n| n.id.clone()).collect();
                c.before.retain(|p| here.contains(p));
                c.focused = c.before.pop().or_else(|| here.first().cloned());
            }
            let id = c.id.clone();
            self.reflow(&id);
            return (false, false);
        }
        let id = c.id.clone();
        if self.is_floats(&id) {
            self.remove_tab(&id);
            return (false, false);
        }
        let (_, space_closed) = self.remove_tab(&id);
        (true, space_closed)
    }

    /// Removes a tab. Returns its terminal addresses and whether its space closed.
    /// A space's floats go alone; its last tab takes the space and its floats.
    pub fn remove_tab(&mut self, ctx: &str) -> (Vec<String>, bool) {
        if let Some(p) = self.spaces.iter_mut().find(|p| p.floats.as_ref().is_some_and(|f| f.id == ctx)) {
            let floats = p.floats.take().unwrap();
            p.floats_shown = false;
            p.floats_focused = false;
            return (addresses(&floats.root), false);
        }
        let Some(pi) = self.spaces.iter().position(|p| p.tab(ctx).is_some()) else { return (vec![], false) };
        let p = &mut self.spaces[pi];
        let ci = p.tabs.iter().position(|c| c.id == ctx).unwrap();
        let removed = p.tabs.remove(ci);
        for child in p.tabs.iter_mut().filter(|t| t.parent.as_deref() == Some(ctx)) {
            child.parent = removed.parent.clone();
        }
        if p.active.as_deref() == Some(ctx) {
            // The tab that slides into the slot, or the last one.
            p.active = p.tabs.get(ci).or(p.tabs.last()).map(|c| c.id.clone());
        }
        let closed = p.tabs.is_empty();
        if closed {
            let id = p.id.clone();
            let floats = self.remove_space(&id);
            self.doomed.extend(floats);
        }
        (addresses(&removed.root), closed)
    }

    /// Removes a space (remembered in `recent`). Returns its terminal addresses.
    pub fn remove_space(&mut self, id: &str) -> Vec<String> {
        let Some(pi) = self.spaces.iter().position(|p| p.id == id) else { return vec![] };
        let was_active = self.active_index() == Some(pi);
        let p = self.spaces.remove(pi);
        self.recent.retain(|r| r.name != p.name);
        self.recent.insert(0, Recent { name: p.name.clone(), folder: p.folder.clone() });
        self.recent.truncate(20);
        if was_active {
            self.active = self.spaces.get(pi).or(self.spaces.last()).map(|p| p.id.clone());
        }
        p.tabs.iter().chain(p.floats.iter()).flat_map(|c| addresses(&c.root)).collect()
    }

    /// The focused pane of the selected space: its focused float while the
    /// floats hold the keyboard, else its selected tab's.
    pub fn focused_pane(&self) -> Option<String> {
        self.active_space()?.focused_tab()?.focused_pane()
    }

    /// Shows or hides a space's floats; hidden, they let go of the keyboard.
    pub fn show_floats(&mut self, space: &str, shown: bool, focused: bool) -> bool {
        let Some(p) = self.space_mut(space).filter(|p| p.floats.is_some()) else { return false };
        p.floats_shown = shown;
        p.floats_focused = shown && focused;
        true
    }

    /// Selects a tab and its space. Shown again, a settled tab is back.
    pub fn select_tab(&mut self, ctx: &str) -> bool {
        let Some(p) = self.spaces.iter_mut().find(|p| p.tab(ctx).is_some()) else { return false };
        p.select(ctx);
        self.active = Some(p.id.clone());
        self.unsettle(ctx);
        true
    }

    /// Puts a tab away. The shown one hands over to the next unsettled
    /// tab of its space, if there is one.
    pub fn settle(&mut self, ctx: &str, now: u64) -> bool {
        let Some(c) = self.tab_mut(ctx) else { return false };
        c.settled = Some(now);
        let Some(p) = self.spaces.iter_mut().find(|p| p.tab(ctx).is_some()) else { return true };
        if p.active_tab().is_some_and(|c| c.id == ctx) {
            let i = p.tabs.iter().position(|c| c.id == ctx).unwrap_or(0);
            let n = p.tabs.len();
            if let Some(next) = (1..n).map(|k| &p.tabs[(i + k) % n]).find(|c| c.settled.is_none()) {
                p.active = Some(next.id.clone());
            }
        }
        true
    }

    pub fn unsettle(&mut self, ctx: &str) -> bool {
        self.tab_mut(ctx).is_some_and(|c| c.settled.take().is_some())
    }

    /// Focuses a pane anywhere: selects its space and tab. A float brings its
    /// floats up with the keyboard; a tab's pane takes the keyboard from them.
    pub fn focus_pane(&mut self, pane_id: &str) -> bool {
        let Some(c) = self.tab_of_mut(pane_id) else { return false };
        c.focus(pane_id);
        let ctx = c.id.clone();
        if let Some(p) = self.spaces.iter_mut().find(|p| p.floats.as_ref().is_some_and(|f| f.id == ctx)) {
            p.floats_shown = true;
            p.floats_focused = true;
            self.active = Some(p.id.clone());
            return true;
        }
        if let Some(p) = self.spaces.iter_mut().find(|p| p.tab(&ctx).is_some()) {
            p.floats_focused = false;
        }
        self.select_tab(&ctx)
    }

    /// The address shown in a pane block.
    pub fn address_of(&self, pane_id: &str) -> Option<String> {
        self.tabs().find_map(|c| c.root.find(pane_id)).and_then(|n| n.terminal()).map(str::to_owned)
    }

    pub fn pane_of(&self, address: &str) -> Option<String> {
        self.tabs().flat_map(|c| c.root.terminals()).find(|n| n.terminal() == Some(address)).map(|n| n.id.clone())
    }

    /// Every terminal in every tab, in space → tab → pane order.
    pub fn all_addresses(&self) -> Vec<String> {
        self.tabs().flat_map(|c| addresses(&c.root)).collect()
    }

    /// Applies a status word from core (Herdr's seen rules).
    pub fn set_word(&mut self, address: &str, word: Option<String>, app: Option<String>, looking: bool) -> bool {
        let a = self.agents.entry(address.to_owned()).or_insert_with(|| Agent { seen: true, ..Default::default() });
        a.app = app.or(a.app.take());
        if a.word == word {
            return false;
        }
        let was_busy = matches!(a.word.as_deref(), Some("working" | "blocked"));
        match word.as_deref() {
            Some("working" | "blocked") => a.seen = true,
            // A finish you didn't watch is "done" until you look at it.
            Some(_) if was_busy => a.seen = looking,
            _ => {}
        }
        a.word = word;
        self.seq += 1;
        a.seq = self.seq;
        a.since = now_ms();
        true
    }

    /// Herdr's attention rule for a change of 7501 word: needing you always
    /// alerts; finishing alerts only if the person wasn't looking.
    pub fn alert(prev: Option<&str>, new: Option<&str>, looking: bool) -> Option<Alert> {
        let blocked = |w: Option<&str>| matches!(w, Some("blocked" | "error"));
        let busy = |w: Option<&str>| matches!(w, Some("working" | "blocked" | "error"));
        if blocked(new) && !blocked(prev) {
            Some(Alert::Blocked)
        } else if busy(prev) && new.is_some() && !busy(new) && !looking {
            Some(Alert::Done)
        } else {
            None
        }
    }

    /// `alert` for one terminal: only ours ring. A sub-agent reports to its
    /// parent; a terminal in none of the spaces (another machine's, another
    /// app's) isn't the person's business here.
    pub fn alert_for(&self, addr: &str, sub_agent: bool, prev: Option<&str>, new: Option<&str>, looking: bool) -> Option<Alert> {
        if sub_agent || self.pane_of(addr).is_none() {
            return None;
        }
        Self::alert(prev, new, looking)
    }

    /// Marks what the person is looking at as seen.
    pub fn mark_seen(&mut self, addresses: &[String]) -> bool {
        let mut changed = false;
        for addr in addresses {
            if let Some(a) = self.agents.get_mut(addr)
                && !a.seen
            {
                a.seen = true;
                changed = true;
            }
        }
        changed
    }

    /// Core's turn and the shared seen turn for an agent; true when either changed.
    pub fn set_turn(&mut self, address: &str, turn: u64, seen_turn: Option<u64>) -> bool {
        let Some(a) = self.agents.get_mut(address) else { return false };
        let changed = (a.turn, a.seen_turn) != (turn, seen_turn);
        (a.turn, a.seen_turn) = (turn, seen_turn);
        changed
    }

    /// What the UI shows: working, blocked, done (finished, unseen), idle, or None (not an agent).
    pub fn status(&self, address: &str) -> Option<&'static str> {
        let a = self.agents.get(address)?;
        Some(match a.word.as_deref()? {
            "working" => "working",
            "blocked" | "error" => "blocked",
            _ if a.seen_turn.map_or(!a.seen, |s| a.turn > s) => "done",
            _ => "idle",
        })
    }

    /// When something in a tab last changed state, ms (None: nothing since the app started).
    pub fn changed_at(&self, c: &Tab) -> Option<u64> {
        addresses(&c.root).iter().filter_map(|a| self.agents.get(a)).map(|a| a.since).filter(|t| *t > 0).max()
    }

    /// Since when an agent in a tab has been working, ms (the earliest, if several).
    pub fn working_since(&self, c: &Tab) -> Option<u64> {
        addresses(&c.root)
            .iter()
            .filter_map(|a| self.agents.get(a))
            .filter(|a| a.word.as_deref() == Some("working") && a.since > 0)
            .map(|a| a.since)
            .min()
    }

    /// The most urgent status among some terminals.
    pub fn rollup<'a>(&self, addresses: impl IntoIterator<Item = &'a String>) -> Option<&'static str> {
        addresses.into_iter().filter_map(|a| self.status(a)).max_by_key(|s| priority(Some(s)))
    }


}

pub fn pane(id: &str, address: &str) -> Node {
    Node::block(id, Content::Terminal { terminal: address.to_owned() })
}

/// A folder as a name: its last part, `~` for the home folder (`/` and `\` both).
pub fn folder_name(path: &str, home: &str) -> String {
    let full = path;
    let path = path.trim_end_matches(['/', '\\']);
    let home = home.trim_end_matches(['/', '\\']);
    let same = if cfg!(windows) { path.eq_ignore_ascii_case(home) } else { path == home };
    if full.is_empty() || same {
        return "~".into();
    }
    if path.is_empty() {
        return full[..1].to_owned();
    }
    path.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(path).to_owned()
}

/// Terminal addresses in a subtree, in tree order.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn addresses(node: &Node) -> Vec<String> {
    node.terminals().iter().filter_map(|n| n.terminal().map(str::to_owned)).collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderInfo {
    /// The checkout's top folder (a worktree's own), else the folder itself.
    pub top: String,
    pub branch: Option<String>,
}

/// A space as mods see it. `name` is what to call it; `named`: someone gave it
/// (else it's Herdr's automatic name, which a mod may show its own way).
pub fn space_json(m: &Model, p: &Space, index: usize, branch: Option<&str>, home: &str) -> Value {
    let active = p.active_tab().map(|c| c.id.clone());
    let tabs: Vec<Value> = p
        .tabs
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "id": c.id,
                "index": i + 1,
                "name": Model::tab_name(p, c),
                "named": !c.name.is_empty(),
                "active": Some(&c.id) == active.as_ref(),
                "status": m.rollup(&addresses(&c.root)),
                "panes": c.root.panes().len(),
                "arrangement": c.arrangement.map(Arrangement::name),
                "parent": c.parent,
                "zoomed": c.zoomed.is_some(),
                "settled": c.settled.is_some(),
                "settled_at": c.settled,
                "changed_at": m.changed_at(c),
                "working_since": m.working_since(c),
            })
        })
        .collect();
    let all: Vec<String> = p.tabs.iter().chain(p.floats.iter()).flat_map(|c| addresses(&c.root)).collect();
    json!({
        "id": p.id,
        "index": index + 1,
        "name": m.space_name(p, home),
        "named": !p.name.is_empty(),
        "folder": p.folder,
        "path": p.path,
        "machine": p.machine,
        "branch": branch,
        "active": m.active.as_deref() == Some(&p.id),
        "status": m.rollup(&all),
        "tabs": tabs,
        "floats": p.floats.as_ref().map(|f| json!({
            "id": f.id,
            "panes": f.root.panes().len(),
            "arrangement": f.arrangement.map(Arrangement::name),
            "status": m.rollup(&addresses(&f.root)),
            "shown": p.floats_shown,
            "focused": p.floats_shown && p.floats_focused,
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_name_as_in_herdr_on_every_system() {
        assert_eq!(folder_name("/home/can", "/home/can"), "~");
        assert_eq!(folder_name("/home/can/", "/home/can"), "~");
        assert_eq!(folder_name("/Users/can/Projects/herdr", "/Users/can"), "herdr");
        assert_eq!(folder_name("/", "/home/can"), "/");
        assert_eq!(folder_name("", "/home/can"), "~");
        assert_eq!(folder_name(r"C:\Users\can\src\api", r"C:\Users\can"), "api");
        assert_eq!(folder_name(r"\\?\C:\Users\can\src\api\", r"C:\Users\can"), "api");
        assert_eq!(folder_name("C:/Users/can/src/web", r"C:\Users\can"), "web");
    }

    #[test]
    fn unnamed_spaces_follow_their_first_terminal_and_tabs_count() {
        let mut m = Model::default();
        m.add_space("p1", "", "/home/me", Tab::new("c1", "", "x1", "t1"));
        m.add_tab("p1", Tab::new("c2", "review", "x2", "t2"));
        m.add_tab("p1", Tab::new("c3", "", "x3", "t3"));
        let p = m.space("p1").unwrap().clone();
        assert_eq!(m.space_name(&p, "/home/me"), "~");
        m.pane_folders.insert("x1".into(), "/src/notes".into());
        assert_eq!(m.space_name(&p, "/home/me"), "notes");
        m.folder_info.insert("t1".into(), FolderInfo { top: "/src/herdr".into(), branch: Some("main".into()) });
        assert_eq!(m.space_name(&p, "/home/me"), "herdr");
        let names: Vec<String> = p.tabs.iter().map(|c| Model::tab_name(&p, c)).collect();
        assert_eq!(names.iter().map(String::as_str).collect::<Vec<_>>(), ["1", "review", "3"]);
        m.space_mut("p1").unwrap().name = "api".into();
        assert_eq!(m.space_name(m.space("p1").unwrap(), "/home/me"), "api");
    }

    #[test]
    fn alerts_follow_herdr() {
        let alert = Model::alert;
        assert_eq!(alert(Some("working"), Some("blocked"), true), Some(Alert::Blocked));
        assert_eq!(alert(Some("blocked"), Some("error"), false), None);
        assert_eq!(alert(Some("working"), Some("idle"), false), Some(Alert::Done));
        assert_eq!(alert(Some("blocked"), Some("done"), false), Some(Alert::Done));
        assert_eq!(alert(Some("working"), Some("idle"), true), None);
        assert_eq!(alert(None, Some("idle"), false), None);
        assert_eq!(alert(Some("working"), None, false), None);
    }

    #[test]
    fn only_our_own_agents_alert() {
        let m = model_with_one();
        // In a space: it rings.
        assert_eq!(m.alert_for("t1", false, Some("working"), Some("done"), false), Some(Alert::Done));
        // Another machine's, or another app's, in none of the spaces: quiet.
        assert_eq!(m.alert_for("vps/t7", false, Some("working"), Some("done"), false), None);
        assert_eq!(m.alert_for("t9", false, Some("working"), Some("blocked"), false), None);
        // A sub-agent reports to its parent, not to the person.
        assert_eq!(m.alert_for("t1", true, Some("working"), Some("blocked"), false), None);
    }

    fn model_with_one() -> Model {
        let mut m = Model::default();
        m.add_space("p1", "api", "/src/api", Tab::new("c1", "fix login", "x1", "t1"));
        m
    }

    #[test]
    fn a_followed_pane_moves_to_a_new_tab_there_with_focus_only_when_asked() {
        let mut m = model_with_one();
        m.split("x1", Axis::Row, "x2", "t2");
        m.add_space("p2", "web", "/src/web", Tab::new("c3", "deploy", "x3", "t3"));
        m.add_tab("p2", Tab::new("c4", "", "x4", "t4"));
        m.select_tab("c3");
        m.focus_pane("x1");
        assert!(m.move_to_space("x2", "p2", "c5", false));
        assert_eq!(pane_order(&m, "c1"), ["x1"]);
        assert_eq!(pane_order(&m, "c5"), ["x2"]);
        assert_eq!(m.space("p2").unwrap().tabs.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["c3", "c5", "c4"]);
        assert_eq!(m.space("p2").unwrap().active.as_deref(), Some("c3"), "a background move changes nothing shown");
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
        assert!(m.move_to_space("x1", "p2", "c6", true));
        assert!(m.space("p1").is_none(), "its last pane leaving closes the space");
        assert_eq!((m.active.as_deref(), m.focused_pane().as_deref()), (Some("p2"), Some("x1")));
        assert_eq!(m.address_of("x1").as_deref(), Some("t1"));
    }

    #[test]
    fn floats_and_moves_into_their_own_space_stay_put() {
        let mut m = model_with_one();
        m.add_float("p1", "f1", pane("y1", "u1"));
        m.add_space("p2", "web", "/src/web", Tab::new("c3", "deploy", "x3", "t3"));
        assert!(!m.move_to_space("y1", "p2", "c9", true));
        assert!(!m.move_to_space("x1", "p1", "c9", true));
        assert!(!m.move_to_space("x1", "missing", "c9", true));
        assert_eq!(m.space_of("y1").map(|p| p.id.as_str()), Some("p1"));
        assert_eq!(m.space_of("x1").map(|p| p.id.as_str()), Some("p1"));
    }

    #[test]
    fn a_pane_founding_a_space_takes_its_machine_and_pin() {
        let mut m = Model::default();
        m.add_space("vps/p1", "", "/srv", Tab::new("vps/c1", "", "vps/x1", "vps/t1"));
        m.space_mut("vps/p1").unwrap().machine = Some("vps".into());
        m.split("vps/x1", Axis::Row, "vps/x2", "vps/t2");
        assert!(m.move_to_new_space("vps/x2", "vps/p2", "vps/c2", "/srv/api/src", Some("/srv/api"), true));
        let p = m.space("vps/p2").unwrap();
        assert_eq!((p.machine.as_deref(), p.path.as_deref(), p.folder.as_str()), (Some("vps"), Some("/srv/api"), "/srv/api/src"));
        assert_eq!(m.space_name(p, "/home/u"), "api");
        assert_eq!(m.focused_pane().as_deref(), Some("vps/x2"));
        assert!(!m.move_to_new_space("vps/x1", "vps/p2", "vps/c3", "/", None, true), "never over a space that exists");
    }

    fn add_child(m: &mut Model, id: &str, parent: &str) -> bool {
        m.add_tab("p1", Tab::new(id, "", &format!("x-{id}"), &format!("t-{id}")));
        m.nest(id, parent)
    }

    fn tab_order(m: &Model, space: &str) -> Vec<String> {
        m.space(space).unwrap().tabs.iter().map(|t| t.id.clone()).collect()
    }

    #[test]
    fn a_child_tab_opens_after_its_parents_family() {
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "docs", "x2", "t2"));
        add_child(&mut m, "a", "c1");
        add_child(&mut m, "g", "a");
        m.select_tab("c2");
        add_child(&mut m, "b", "c1");
        assert_eq!(tab_order(&m, "p1"), ["c1", "a", "g", "b", "c2"]);
        assert_eq!(m.active_tab().map(|t| t.id.as_str()), Some("b"));
        assert!(!add_child(&mut m, "lost", "gone"));
        assert_eq!(m.tab("lost").unwrap().parent, None, "a parent that isn't in the space: top-level");
        assert!(!m.nest("c1", "g"), "never under its own descendant");
        assert!(!m.nest("c1", "c1"));
        assert_eq!(m.tab("c1").unwrap().parent, None);
    }

    #[test]
    fn closing_a_parent_hands_its_children_to_the_grandparent() {
        let mut m = model_with_one();
        add_child(&mut m, "a", "c1");
        add_child(&mut m, "g1", "a");
        add_child(&mut m, "g2", "a");
        m.remove_tab("a");
        assert_eq!(m.tab("g1").unwrap().parent.as_deref(), Some("c1"));
        assert_eq!(m.tab("g2").unwrap().parent.as_deref(), Some("c1"));
        m.remove_tab("c1");
        assert_eq!((m.tab("g1").unwrap().parent.clone(), m.tab("g2").unwrap().parent.clone()), (None, None));
    }

    #[test]
    fn a_child_reads_parent_then_child() {
        let mut m = model_with_one();
        add_child(&mut m, "a", "c1");
        add_child(&mut m, "b", "a");
        let p = m.space("p1").unwrap();
        assert_eq!(Model::tab_label(p, p.tab("a").unwrap()), "fix login › 2");
        assert_eq!(Model::tab_label(p, p.tab("b").unwrap()), "2 › 3", "only the parent, not the whole line");
        assert_eq!(Model::tab_label(p, p.tab("c1").unwrap()), "fix login");
        assert_eq!(space_json(&m, p, 0, None, "/")["tabs"][1]["parent"], "c1");
        assert!(space_json(&m, p, 0, None, "/")["tabs"][0]["parent"].is_null());
    }

    #[test]
    fn parents_that_dont_hold_are_dropped() {
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "", "x2", "t2"));
        m.add_tab("p1", Tab::new("c3", "", "x3", "t3"));
        m.add_tab("p1", Tab::new("c4", "", "x4", "t4"));
        let p = m.space_mut("p1").unwrap();
        for (tab, parent) in [("c1", "c1"), ("c2", "c3"), ("c3", "c2"), ("c4", "missing")] {
            p.tabs.iter_mut().find(|t| t.id == tab).unwrap().parent = Some(parent.into());
        }
        p.tidy_parents();
        let parents: Vec<Option<&str>> = p.tabs.iter().map(|t| t.parent.as_deref()).collect();
        assert_eq!(parents, [None, None, Some("c2"), None], "a loop is broken once, the rest of it kept");
    }

    #[test]
    fn a_new_tabs_parent_is_the_named_one_else_the_callers_tab() {
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "", "x2", "t2"));
        m.add_float("p1", "f1", pane("y1", "u1"));
        m.add_space("p2", "web", "/src/web", Tab::new("c3", "", "x3", "t3"));
        assert_eq!(m.new_tab_parent(None, Some("t2"), None), Ok(Some("c2".into())));
        assert_eq!(m.new_tab_parent(None, Some("t2"), Some("p2")), Ok(None), "a tab for another space is top-level");
        assert_eq!(m.new_tab_parent(None, Some("u1"), None), Ok(None), "floats are no parent");
        assert_eq!(m.new_tab_parent(None, Some("t9"), None), Ok(None), "a terminal without a pane");
        assert_eq!(m.new_tab_parent(None, None, None), Ok(None));
        assert_eq!(m.new_tab_parent(Some(""), Some("t2"), None), Ok(None), "an empty parent: top-level, even from a pane");
        assert_eq!(m.new_tab_parent(Some("c3"), Some("t2"), None), Ok(Some("c3".into())));
        assert!(m.new_tab_parent(Some("c3"), None, Some("p1")).is_err());
        assert!(m.new_tab_parent(Some("f1"), None, None).is_err());
        assert!(m.new_tab_parent(Some("nope"), None, None).is_err());
    }

    #[test]
    fn settling_the_shown_tab_hands_over_and_showing_it_brings_it_back() {
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "docs", "x2", "t2"));
        m.select_tab("c1");
        assert!(m.settle("c1", 5));
        assert_eq!(m.active_tab().map(|c| c.id.as_str()), Some("c2"));
        assert_eq!(m.tab("c1").unwrap().settled, Some(5));
        // The last unsettled one stays shown, settled.
        assert!(m.settle("c2", 6));
        assert_eq!(m.active_tab().map(|c| c.id.as_str()), Some("c2"));
        m.select_tab("c1");
        assert_eq!(m.tab("c1").unwrap().settled, None);
    }

    #[test]
    fn closing_the_last_pane_closes_tab_then_space_and_remembers_it() {
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "docs", "x2", "t2"));
        m.add_space("p2", "web", "/src/web", Tab::new("c3", "deploy", "x3", "t3"));
        assert_eq!(m.active.as_deref(), Some("p2"));
        assert_eq!(m.remove_pane("x3"), (true, true));
        assert_eq!(m.active.as_deref(), Some("p1"));
        assert_eq!(m.recent, [Recent { name: "web".into(), folder: "/src/web".into() }]);
        // A tab closes with its last pane; the space stays while it has another.
        assert_eq!(m.remove_pane("x2"), (true, false));
        assert_eq!(m.active_tab().map(|c| c.id.as_str()), Some("c1"));
        // Opening it again takes it off the recent list.
        m.add_space("p4", "web", "/src/web", Tab::new("c4", "x", "x4", "t4"));
        assert!(m.recent.is_empty());
    }

    #[test]
    fn a_mod_is_a_pane_beside_terminals() {
        let mut m = model_with_one();
        let viewer = Node::block("x2", Content::Mod { name: "markdown".into(), props: json!({ "file": "README.md" }) });
        assert!(m.split_with("x1", Axis::Row, viewer));
        let c = m.tab("c1").unwrap();
        assert_eq!(c.root.panes().len(), 2);
        assert_eq!(c.focused.as_deref(), Some("x2"));
        // It has no terminal: nothing to restart or kill, and the tab survives
        // its terminal pane closing.
        assert_eq!(m.address_of("x2"), None);
        assert_eq!(m.remove_pane("x1"), (false, false));
        assert_eq!(m.tab("c1").unwrap().root.panes().len(), 1);
    }

    #[test]
    fn pane_moves_preserve_terminals_and_focus_for_swap_and_every_edge() {
        use crate::layout::Edge;
        for edge in [None, Some(Edge::Left), Some(Edge::Right), Some(Edge::Top), Some(Edge::Bottom)] {
            let mut m = model_with_one();
            m.split("x1", Axis::Row, "x2", "t2");
            m.split("x2", Axis::Column, "x3", "t3");
            assert!(m.move_pane("x1", "x3", edge));
            assert_eq!(m.focused_pane().as_deref(), Some("x1"));
            for (pane, address) in [("x1", "t1"), ("x2", "t2"), ("x3", "t3")] {
                assert_eq!(m.address_of(pane).as_deref(), Some(address));
            }
            let root = &m.active_tab().unwrap().root;
            assert_eq!(root.panes().len(), 3);
            if let Some(edge) = edge {
                let parent = root.parent_of("x1").unwrap();
                let expected_axis = if matches!(edge, Edge::Left | Edge::Right) { Axis::Row } else { Axis::Column };
                assert_eq!(parent.axis(), Some(expected_axis));
                let ids: Vec<_> = parent.children().iter().map(|n| n.id.as_str()).collect();
                let source = ids.iter().position(|id| *id == "x1").unwrap();
                let target = ids.iter().position(|id| *id == "x3").unwrap();
                assert_eq!(source < target, matches!(edge, Edge::Left | Edge::Top));
                assert_eq!(source.abs_diff(target), 1);
            }
        }
    }

    #[test]
    fn cross_axis_pane_moves_split_equally_not_with_the_old_slot_size() {
        use crate::layout::{Edge, Size};
        for old_size in [Some(Size::Weight { weight: 0.5 }), Some(Size::Px { px: 200. })] {
            let mut m = model_with_one();
            m.split("x1", Axis::Row, "x2", "t2");
            m.tab_of_mut("x2").unwrap().root.find_mut("x2").unwrap().size = old_size;
            assert!(m.move_pane("x2", "x1", Some(Edge::Bottom)));
            let parent = m.active_tab().unwrap().root.parent_of("x2").unwrap();
            assert_eq!(parent.children()[0].size, None);
            assert_eq!(parent.children()[1].size, None);
        }
    }

    #[test]
    fn invalid_pane_moves_leave_the_model_alone() {
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "docs", "x2", "t2"));
        let before = format!("{:?}", m.spaces);
        for (source, target) in [("x1", "x1"), ("x1", "missing"), ("missing", "x1"), ("x1", "x2"), ("c1", "x1")] {
            assert!(!m.move_pane(source, target, None));
            assert_eq!(format!("{:?}", m.spaces), before);
        }
    }

    #[test]
    fn new_tabs_go_after_the_selected_one() {
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "b", "x2", "t2"));
        m.select_tab("c1");
        m.add_tab("p1", Tab::new("c3", "c", "x3", "t3"));
        let order: Vec<&str> = m.spaces[0].tabs.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(order, ["c1", "c3", "c2"]);
        assert_eq!(m.active_tab().map(|c| c.id.as_str()), Some("c3"));
    }

    fn three_panes_and_another_tab() -> Model {
        let mut m = model_with_one();
        m.split("x1", Axis::Row, "x2", "t2");
        m.split("x2", Axis::Row, "x3", "t3");
        m.add_tab("p1", Tab::new("c2", "docs", "x4", "t4"));
        m.select_tab("c1");
        m.focus_pane("x1");
        m
    }

    fn pane_order(m: &Model, tab: &str) -> Vec<String> {
        m.tab(tab).unwrap().root.panes().iter().map(|n| n.id.clone()).collect()
    }

    #[test]
    fn swapping_panes_by_id_trades_places_and_keeps_focus_on_the_same_id() {
        let mut m = three_panes_and_another_tab();
        assert_eq!(m.swap_panes("x1", "x3"), Ok(()));
        assert_eq!(pane_order(&m, "c1"), ["x3", "x2", "x1"]);
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
        assert_eq!(m.address_of("x1").as_deref(), Some("t1"));
        assert_eq!(m.swap_panes("x2", "x3"), Ok(()));
        assert_eq!(pane_order(&m, "c1"), ["x2", "x3", "x1"]);
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
    }

    #[test]
    fn swapping_panes_by_id_refuses_each_bad_pair_without_changes() {
        let mut m = three_panes_and_another_tab();
        let cases = [
            ("nope", "x2", "no pane \"nope\""),
            ("x1", "nope", "no target pane \"nope\""),
            ("x1", "x1", "pane.swap target is the pane itself"),
            ("x1", "x4", "pane.swap target \"x4\" is in another tab"),
        ];
        for (pane, target, error) in cases {
            assert_eq!(m.swap_panes(pane, target), Err(error.to_owned()), "{pane} {target}");
            assert_eq!(pane_order(&m, "c1"), ["x1", "x2", "x3"], "{pane} {target}");
            assert_eq!(pane_order(&m, "c2"), ["x4"], "{pane} {target}");
            assert_eq!(m.focused_pane().as_deref(), Some("x1"), "{pane} {target}");
        }
    }

    #[test]
    fn swapping_refuses_a_container_as_pane_or_target() {
        let mut m = three_panes_and_another_tab();
        let container = m.tab("c1").unwrap().root.id.clone();
        assert_eq!(m.swap_panes(&container, "x1"), Err(format!("no pane {container:?}")));
        assert_eq!(m.swap_panes("x1", &container), Err(format!("no target pane {container:?}")));
        assert_eq!(pane_order(&m, "c1"), ["x1", "x2", "x3"]);
    }

    #[test]
    fn splits_focus_the_new_pane_and_closing_it_moves_focus() {
        let mut m = model_with_one();
        assert!(m.split("x1", Axis::Row, "x9", "t9"));
        assert_eq!(m.focused_pane().as_deref(), Some("x9"));
        assert_eq!(m.remove_pane("x9"), (false, false));
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
        // The arrangement keeps the tab's id with one pane left.
        assert_eq!(m.active_tab().unwrap().root.id, "c1");
    }

    #[test]
    fn closing_the_focused_pane_goes_back_to_the_one_before() {
        let mut m = model_with_one();
        m.split("x1", Axis::Row, "x2", "t2");
        m.split("x2", Axis::Row, "x3", "t3");
        // From x2 to x3; closing x3 lands on x2, not on the first pane.
        m.remove_pane("x3");
        assert_eq!(m.focused_pane().as_deref(), Some("x2"));
        // Clicked around: x1, then x2; close x2 and it's x1.
        m.split("x2", Axis::Row, "x4", "t4");
        m.focus_pane("x1");
        m.focus_pane("x4");
        m.remove_pane("x4");
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
        // The one before is gone too: the next one back that's still there.
        m.split("x2", Axis::Row, "x5", "t5");
        m.focus_pane("x2");
        m.remove_pane("x5");
        m.remove_pane("x2");
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
    }

    #[test]
    fn finishes_you_did_not_watch_are_done_until_seen() {
        let mut m = model_with_one();
        m.set_word("t1", Some("working".into()), Some("claude".into()), false);
        assert_eq!(m.status("t1"), Some("working"));
        m.set_word("t1", Some("idle".into()), None, false);
        assert_eq!(m.status("t1"), Some("done"));
        assert_eq!(m.rollup(&m.all_addresses()), Some("done"));
        m.mark_seen(&["t1".into()]);
        assert_eq!(m.status("t1"), Some("idle"));
        // Watching it finish: straight to idle.
        m.set_word("t1", Some("working".into()), None, true);
        m.set_word("t1", Some("done".into()), None, true);
        assert_eq!(m.status("t1"), Some("idle"));
        m.set_word("t1", Some("blocked".into()), None, true);
        assert_eq!(m.rollup(&m.all_addresses()), Some("blocked"));
    }

    #[test]
    fn a_shared_seen_turn_decides_over_this_screen() {
        let mut m = model_with_one();
        m.set_word("t1", Some("working".into()), None, true);
        m.set_word("t1", Some("idle".into()), None, true);
        assert_eq!(m.status("t1"), Some("idle"));
        // Another client hasn't seen turn 3 (marked unread): done, though we watched.
        assert!(m.set_turn("t1", 3, Some(2)));
        assert_eq!(m.status("t1"), Some("done"));
        assert!(!m.set_turn("t1", 3, Some(2)));
        // Seen elsewhere: idle, though we didn't watch.
        m.set_word("t1", Some("working".into()), None, false);
        m.set_word("t1", Some("idle".into()), None, false);
        m.set_turn("t1", 4, Some(4));
        assert_eq!(m.status("t1"), Some("idle"));
        // Needing you or working beats seen.
        m.set_word("t1", Some("blocked".into()), None, false);
        assert_eq!(m.status("t1"), Some("blocked"));
        // No label: this screen's own rule.
        m.set_word("t1", Some("idle".into()), None, false);
        m.set_turn("t1", 5, None);
        assert_eq!(m.status("t1"), Some("done"));
    }

    #[test]
    fn focusing_another_pane_in_a_zoomed_tab_moves_the_zoom() {
        let mut m = model_with_one();
        assert!(m.split("x1", Axis::Row, "x2", "t2"));
        m.tab_mut("c1").unwrap().zoomed = Some("x2".into());
        assert!(m.focus_pane("x1"));
        assert_eq!(m.tab("c1").unwrap().zoomed.as_deref(), Some("x1"));
        // A tab nobody zoomed stays unzoomed when focus moves.
        m.tab_mut("c1").unwrap().zoomed = None;
        assert!(m.focus_pane("x2"));
        assert_eq!(m.tab("c1").unwrap().zoomed, None);
    }

    fn arranged(kind: Arrangement) -> Model {
        let mut m = Model { default_arrangement: Some(kind), aspect: 1.6, ..Default::default() };
        m.add_space("p1", "api", "/src/api", Tab::new("c1", "", "x1", "t1"));
        m
    }

    fn weights(n: &Node) -> Vec<Option<crate::layout::Size>> {
        n.panes().iter().map(|p| p.size).collect()
    }

    #[test]
    fn new_tabs_take_the_default_arrangement() {
        let m = arranged(Arrangement::Grid);
        assert_eq!(m.tab("c1").unwrap().arrangement, Some(Arrangement::Grid));
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "", "x2", "t2"));
        assert_eq!(m.tab("c2").unwrap().arrangement, None);
    }

    #[test]
    fn adding_to_an_arranged_tab_goes_after_the_target_evenly_and_focuses_it() {
        let mut m = arranged(Arrangement::Vertical);
        m.add_pane("x1", pane("x2", "t2"));
        m.add_pane("x2", pane("x3", "t3"));
        m.add_pane("x1", pane("x4", "t4"));
        assert_eq!(pane_order(&m, "c1"), ["x1", "x4", "x2", "x3"]);
        assert_eq!(m.focused_pane().as_deref(), Some("x4"));
        let root = &m.tab("c1").unwrap().root;
        assert_eq!((root.id.as_str(), root.axis(), root.children().len()), ("c1", Some(Axis::Row), 4));
    }

    #[test]
    fn adding_to_a_hand_split_tab_splits_right() {
        let mut m = model_with_one();
        m.split("x1", Axis::Column, "x2", "t2");
        m.add_pane("x2", pane("x3", "t3"));
        assert_eq!(m.tab("c1").unwrap().root.parent_of("x3").unwrap().axis(), Some(Axis::Row));
        assert_eq!(m.tab("c1").unwrap().arrangement, None);
    }

    #[test]
    fn closing_a_pane_evens_an_arranged_tab_out_again() {
        let mut m = arranged(Arrangement::Horizontal);
        m.add_pane("x1", pane("x2", "t2"));
        m.add_pane("x2", pane("x3", "t3"));
        m.tab_mut("c1").unwrap().root.find_mut("x1").unwrap().size = Some(crate::layout::Size::Weight { weight: 3.0 });
        m.remove_pane("x2");
        let root = &m.tab("c1").unwrap().root;
        assert_eq!(weights(root), [None, None]);
        assert_eq!(pane_order(&m, "c1"), ["x1", "x3"]);
    }

    #[test]
    fn splitting_or_dropping_on_an_edge_makes_a_tab_hand_split_but_swaps_keep_its_arrangement() {
        let mut m = arranged(Arrangement::Grid);
        m.add_pane("x1", pane("x2", "t2"));
        m.add_pane("x2", pane("x3", "t3"));
        assert!(m.swap_panes("x1", "x3").is_ok());
        assert_eq!(m.tab("c1").unwrap().arrangement, Some(Arrangement::Grid));
        assert!(m.move_pane("x1", "x2", Some(crate::layout::Edge::Bottom)));
        assert_eq!(m.tab("c1").unwrap().arrangement, None);
        let mut m = arranged(Arrangement::Grid);
        m.split("x1", Axis::Column, "x2", "t2");
        assert_eq!(m.tab("c1").unwrap().arrangement, None);
    }

    #[test]
    fn a_full_cycle_of_arrangements_brings_the_tab_back() {
        let mut m = arranged(Arrangement::Vertical);
        for (id, addr) in [("x2", "t2"), ("x3", "t3"), ("x4", "t4"), ("x5", "t5")] {
            let last = m.tab("c1").unwrap().root.panes().last().unwrap().id.clone();
            m.add_pane(&last, pane(id, addr));
        }
        let start = m.tab("c1").unwrap().root.clone();
        let mut kind = Arrangement::Vertical;
        for _ in 0..4 {
            kind = kind.next();
            assert!(m.arrange("c1", Some(kind)));
            assert_eq!(pane_order(&m, "c1"), ["x1", "x2", "x3", "x4", "x5"]);
        }
        assert_eq!(m.tab("c1").unwrap().root, start);
        assert!(m.arrange("c1", None));
        assert_eq!(m.tab("c1").unwrap().root, start);
    }

    #[test]
    fn floats_open_over_the_space_shown_and_focused_and_stacked() {
        let mut m = model_with_one();
        assert!(m.add_float("p1", "f1", pane("y1", "u1")));
        let p = m.space("p1").unwrap();
        assert!(p.floats_shown && p.floats_focused);
        assert_eq!(p.floats.as_ref().unwrap().arrangement, Some(Arrangement::Stacked));
        assert_eq!(p.tabs.len(), 1);
        assert_eq!(m.focused_pane().as_deref(), Some("y1"));
        assert!(m.add_float("p1", "unused", pane("y2", "u2")));
        assert_eq!(pane_order(&m, "f1"), ["y1", "y2"]);
        assert_eq!(m.focused_pane().as_deref(), Some("y2"));
        assert_eq!((m.pane_of("u2").as_deref(), m.tab_of("y2").map(|t| t.id.as_str())), (Some("y2"), Some("f1")));
        assert!(m.is_floats("f1") && !m.is_floats("c1"));
        assert_eq!(m.all_addresses(), ["t1", "u1", "u2"]);
    }

    #[test]
    fn focus_moves_between_a_tab_and_its_floats_and_hiding_lets_go() {
        let mut m = model_with_one();
        m.add_float("p1", "f1", pane("y1", "u1"));
        assert!(m.focus_pane("x1"));
        let p = m.space("p1").unwrap();
        assert!(p.floats_shown && !p.floats_focused);
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
        assert!(m.focus_pane("y1"));
        assert_eq!(m.focused_pane().as_deref(), Some("y1"));
        assert!(m.show_floats("p1", false, true));
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
        assert!(!m.space("p1").unwrap().floats_focused);
        assert!(!m.show_floats("missing", true, true));
    }

    #[test]
    fn floats_stay_over_every_tab_of_their_space() {
        let mut m = model_with_one();
        m.add_float("p1", "f1", pane("y1", "u1"));
        m.add_tab("p1", Tab::new("c2", "", "x2", "t2"));
        assert!(m.space("p1").unwrap().floats_shown);
        assert_eq!(m.focused_pane().as_deref(), Some("y1"));
        m.remove_tab("c2");
        assert!(m.space("p1").unwrap().floats.is_some());
    }

    #[test]
    fn the_last_float_closing_takes_the_floats_but_never_the_space() {
        let mut m = model_with_one();
        m.add_float("p1", "f1", pane("y1", "u1"));
        assert_eq!(m.remove_pane("y1"), (false, false));
        let p = m.space("p1").unwrap();
        assert!(p.floats.is_none() && !p.floats_shown && !p.floats_focused);
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
    }

    #[test]
    fn a_space_closing_with_its_last_tab_ends_its_floats_too() {
        let mut m = model_with_one();
        m.add_float("p1", "f1", pane("y1", "u1"));
        assert_eq!(m.remove_pane("x1"), (true, true));
        assert_eq!(m.doomed, ["u1"]);
        let mut m = model_with_one();
        m.add_float("p1", "f1", pane("y1", "u1"));
        assert_eq!(m.remove_space("p1"), ["t1", "u1"]);
    }

    #[test]
    fn removing_the_floats_as_a_tab_ends_only_them() {
        let mut m = model_with_one();
        m.add_float("p1", "f1", pane("y1", "u1"));
        m.add_float("p1", "f1", pane("y2", "u2"));
        assert_eq!(m.remove_tab("f1"), (vec!["u1".to_owned(), "u2".to_owned()], false));
        assert!(m.space("p1").unwrap().floats.is_none());
        assert!(m.tab("c1").is_some());
    }

    #[test]
    fn focus_follows_panes_across_tabs_and_spaces() {
        let mut m = model_with_one();
        m.add_tab("p1", Tab::new("c2", "b", "x2", "t2"));
        assert_eq!(m.focused_pane().as_deref(), Some("x2"));
        m.add_space("p2", "web", "/w", Tab::new("c3", "c", "x3", "t3"));
        assert!(m.focus_pane("x1"));
        assert_eq!(m.active.as_deref(), Some("p1"));
        assert_eq!(m.focused_pane().as_deref(), Some("x1"));
        assert_eq!(m.active_tab().map(|c| addresses(c.shown())).unwrap_or_default(), ["t1"]);
    }
}
