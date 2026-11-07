# Spaces that follow directories, and nested tabs

Two features ported from Cerdr's behaviour (its README, `src/workspace/pinned_path.rs`,
`src/app/creation.rs`, `src/workspace/tab_family.rs`). Each lands as its own commit.

## 1. Spaces that follow directories

### Data

- `Space.path: Option<String>`: the directory the space is pinned to, on the space's
  machine. Shared: `SharedSpace.path`, `serde(default)`, skipped when `None`, so a stock
  herdr-app reads the document. A stock client writing the document drops the field,
  as it drops `repos` when it rewrites `settings.json`. `spaces()` gains `path`.
- An unnamed pinned space is called after its pinned directory (it otherwise follows its
  first terminal's repo, which reads `src` once that terminal wanders into a subfolder).
- `settings.json` gains `repos`: `[{ "path": "~/code/herdr" }, { "path": "~/code/projects",
  "children": true }, { "path": "/srv/api", "machine": "vps" }]`. `children` and `machine`
  are optional (false, this computer). `~` expands to this computer's home; an entry for
  another machine must be absolute (its home isn't known here), which `problems()` reports.

### Matching (pure functions in `src/follow.rs`, unit tested)

- `claims(pinned, cwd)`: component-wise prefix, so `/ws` doesn't claim `/ws-worktrees`.
  Local paths are canonicalised when they exist (symlinked checkouts); remote paths compare
  lexically. Depth = component count.
- `declared(cwd, repos)`: the deepest entry that claims `cwd`. A `children` entry claims
  `path/<child>` at depth+1 and never its own folder.
- `decide(...)` for a pane whose cwd changed, after Cerdr's `reclaim_pane_after_cwd_change`:
  1. The deepest pinned space on the pane's machine, other than its own, that claims the
     cwd. The pane's own space competes on depth: it moves only when its own pin is absent
     or shallower. Two spaces pinned to one folder never trade a pane back and forth.
  2. Otherwise only the focused pane may go further, and only when its own space's pin
     doesn't claim the cwd. The deepest declared repo for the cwd on that machine:
     - its own space already sits in that repo (its first terminal's folder is inside):
       pin it in place rather than making a twin;
     - else a new space pinned to the repo, the pane moved into it.

### When it runs

- Each tick, every shell at its prompt (the shell in front, not working) records its
  process and folder. A new folder with the same process is a `cd` and is decided; a new
  process (a restart, a lost shell coming back) or the first sighting after a start only
  records, so nothing moves on its own. A program running in another folder (`git -C`,
  `make -C`) never counts; `cd x && make` follows once make ends.
- Only panes in ordinary tabs follow (never floats), and only into spaces on their own
  machine.
- A moved pane goes to a new tab in the destination. It takes focus and the destination is
  shown only when it is still the focused pane of the shown space when the move lands;
  otherwise nothing shown changes. A source tab left empty closes. A shell whose leaving
  would close its space stays when that space has floats, so a `cd` never ends them.
- The declared-repo check uses the space's first terminal other than the moving one; a
  shell alone in its space pins the space in place instead of founding a twin.
- Moving relabels the terminal (`herdr.space`, `herdr.tab`) first, through the same
  background path `terminal.keep` / `terminal.park` use (`start_terminal_change`, a new
  `TerminalChange::Move`), and changes the model only when the labels are written. If the
  destination space closed meanwhile, it is made again under the labelled id. A move that
  fails or no longer applies writes the labels back to where the pane is.

### Commands and keys

- `space.pin [id] [path]`: no `path` toggles the space's pin against its focused
  terminal's folder (a second press there clears it); a full path sets it (`~` is this computer's home); `""` clears.
  Keys: `alt-.`.
- `space.declare [id]`: declares or undeclares the space's directory (its pin, else its
  focused terminal's folder) in `settings.json` `repos`, for the space's machine. Declaring
  an unpinned space also pins it, so the space owns its repo straight away instead of a
  later `cd` making a second one. Keys: `alt->` (and `alt-shift-.`).
- Keys on macOS match. The pre-change defaults go in `src/keys/shipped/` so untouched
  keys files upgrade.

## 2. Nested tabs

### Data

- `Tab.parent: Option<String>`, a tab id in the same space. Shared: `SharedTab.parent`,
  `serde(default)`, skipped when `None`. A document whose parent names a missing tab, the
  tab itself, a float or a cycle loads that tab as top-level.
- `spaces()` tabs gain `parent` (id or null); `herdr.d.ts` documents it.

### Behaviour

- `tab.new` takes `parent`: a tab id, or `""` for a top-level tab (null means omitted, as for every command). Without it, a
  `herdr-app do tab.new` run inside a terminal makes the new tab a child of that terminal's
  tab: the client sends `$HC_TERMINAL_ID` with the request as `caller`, and the window
  resolves it to its pane's tab. A caller in a float, or a `space` other than the caller's,
  gives a top-level tab. With a parent and no `space`, the tab goes in the parent's space.
- A child opens after its parent's last descendant (next to its siblings).
- Closing a tab hands its children to its own parent (the grandparent), or top level.
- Label: `parent › child`, the parent's own name (named, else its number) then the
  child's. Used by the window title (new: `<space> / <tab label>`, `Herdr` with nothing
  shown), the `where` label `hc ls` shows, and notification bodies.

### Shipped mods

- `tabs` and `picker`: a child reads `parent › child`.
- `agents`: an agent in a child tab nests under the first agent of its nearest ancestor tab
  that has one, drawn indented beneath it. Pinned agents keep their own row.
- `spaces`: unchanged. The sidebar lists spaces, not tabs; the nesting shows in the tab
  bar, the window title and the agents list.

## Guide and docs

`defaults/AGENTS.md` (commands, `repos`, `path`, `parent`), `herdr.d.ts`, `src/SKILL.md`
where it lists commands, and `docs/commands.md` regenerated.
