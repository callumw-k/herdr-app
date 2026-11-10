# Herdr

Herdr is a desktop app for terminals and coding agents. Keep work in spaces, see which agents need you, and leave terminals running when you close the window.

This is a fork of [herdrdev/herdr-app](https://github.com/herdrdev/herdr-app). It adds tab arrangements, floating terminals, spaces that follow your folders and nested tabs, covered in [What this fork adds](#what-this-fork-adds).

## Install this fork

The fork has no release builds, so build it from source. You need Rust 1.95.0, Zig 0.16.0 and [just](https://github.com/casey/just), plus the Linux development packages listed in [CI](.github/workflows/ci.yml).

```sh
git clone --recurse-submodules https://github.com/callumw-k/herdr-app.git
cd herdr-app
```

On Linux and macOS, if Herdr is already installed (from the [upstream installers](#install-upstream-herdr) below, say), swap the fork's build into it:

```sh
scripts/install-local
```

It builds a release, replaces the `herdr-app` on your PATH and leaves the rest of the install as it is. On macOS it re-signs `Herdr.app` afterwards. If it warns that a downloaded update is waiting, that update replaces the fork's build the next time you quit, so run the script again after reopening Herdr.

For a fresh install, use the platform installer instead:

```sh
just install
```

It builds a release and adds Herdr to your desktop (`~/Applications/Herdr.app` on macOS). It doesn't install [herdr-core](https://github.com/herdrdev/herdr-core), but the app offers to install a core when it can't find one.

Quit and reopen Herdr to use the new build. Builds from source don't update themselves, so pull and run the same command again to update.

## What this fork adds

On macOS the Alt keys below are Option, which acts as Alt by default on US and ABC keyboard layouts.

### Tab arrangements

A tab can lay its panes out as even columns, even rows, a grid, or stacked (the focused pane full size, the others folded to title bars). An arranged tab lays itself out again whenever a pane opens or closes. Dragging a divider holds until the next change, and splitting by hand turns the arrangement off for that tab.

| Key | Action |
| --- | --- |
| Alt+] / Alt+[ | Next or previous arrangement (`tab.arrange`) |
| Alt+N | Add a pane in the tab's arrangement (`pane.add`) |

Alt+] and Alt+[ no longer switch tabs. Ctrl+Tab and Ctrl+PageUp/PageDown still do.

### Floating terminals

A space can keep terminals floating over all of its tabs, in a box in the middle of the tab area. They stay put when you switch tabs, are arranged like a tab, and close with the space, never with a tab.

| Key | Action |
| --- | --- |
| Alt+F | Open, focus or hide the floats (`float.toggle`) |
| Alt+Shift+F | Open another float (`float.new`) |

### Spaces that follow your folders

Pin a space to a folder and any shell that `cd`s into that folder, or anywhere below it, moves to that space as a new tab once it's back at its prompt. `cd x && make` moves once `make` ends, and a program working in another folder (`git -C`, `make -C`) moves nothing. The deepest pin wins.

Folders listed under `repos` in settings each own one space. A shell that `cd`s into one joins the space already open there, which gets pinned, and the focused shell starts a new space when there isn't one. A pin shows next to a space in the sidebar when it's pinned or in a declared repo.

| Key | Action |
| --- | --- |
| Alt+. | Pin the shown space to its folder, or unpin it (`space.pin`) |
| Alt+Shift+. | Add or remove the shown space's folder from `repos` (`space.declare`) |

A shell only moves between spaces on its own machine, and floats never move.

### Nested tabs

`herdr-app do tab.new` run in a terminal opens the new tab under that terminal's tab. It reads `parent › child` in the tab bar, the picker and the window title, and opens next to its siblings. In the agents list, an agent in a child tab sits under the parent tab's agent. When a parent closes, its children move up a level.

```sh
herdr-app do tab.new --parent <tab id>   # nest under another tab
herdr-app do tab.new --parent ''         # top-level tab
```

### Smaller changes

- The window title shows the space and tab.
- When this machine's layout first loads with no spaces, Herdr opens one at `~`.

### Settings

Four new keys in `~/.config/herdr-app/settings.json`:

```json
{
  "arrangement": "grid",
  "float_width": 0.9,
  "float_height": 0.9,
  "repos": [
    { "path": "~/code/herdr" },
    { "path": "~/code/projects", "children": true },
    { "path": "/srv/api", "machine": "vps" }
  ]
}
```

- `arrangement`: how new tabs lay out their panes: `vertical`, `horizontal`, `grid` (default), `stacked`, or `none` to split by hand. Existing tabs keep theirs.
- `float_width`, `float_height`: the floats' box. Up to `1` is a share of the tab area (default `0.9`), above `1` is pixels at zoom 1.
- `repos`: folders that each own a space. With `children`, every folder directly inside it is a repo, but not the folder itself. `machine` names another machine, and its paths are written in full, since `~` is this computer's home. The deepest entry wins, so a repo inside a projects folder beats the projects folder.

If you've edited your `keys.json`, add the new keys yourself:

```json
"alt-]": "tab.arrange:next",
"alt-[": "tab.arrange:prev",
"alt-n": "pane.add",
"alt-f": "float.toggle",
"alt-shift-f": "float.new",
"alt-.": "space.pin",
"alt-shift-.": "space.declare"
```

### The agent guide

Herdr writes [defaults/AGENTS.md](defaults/AGENTS.md) to `~/.config/herdr-app/AGENTS.md` on every start, and that's how agents working in your Herdr learn what it can do. The fork's version covers everything above: the commands (`tab.arrange`, `pane.add`, `float.new`, `float.toggle`, `float.show`, `space.pin`, `space.declare`, and `tab.new` with `parent`), the new settings, and the `arrangement`, `floats`, `path`, `repo` and `parent` fields mods read from `spaces()`. `herdr-app --skill` also explains nested tabs. The [command reference](docs/commands.md) lists each command's arguments.

## Install upstream Herdr

These install upstream Herdr from herdr.dev, without the fork's features. Use them to get herdr-core and the desktop app in place before running `scripts/install-local`.

### Linux and macOS

```sh
curl -fsSL https://releases.herdr.dev/app/install.sh | sh
```

Linux needs a desktop session and a working Vulkan driver. macOS builds are available for Apple Silicon; Intel Mac users can [build from source](#build-from-source). Linux builds are x86-64 and ARM64.

The installer adds Herdr to your apps and puts the command in `~/.local/bin`.
Add that folder to your PATH if the installer asks.
On macOS the app lives in `~/Applications/Herdr.app`.
For drag-and-drop installation on Apple Silicon, download [Herdr.dmg](https://releases.herdr.dev/app/Herdr.dmg) and drag Herdr to Applications.

On macOS (Apple Silicon), you can also install with [Homebrew](https://brew.sh/):

```sh
brew install --cask herdr-app
```

### Windows

Run in PowerShell on x64 Windows with curl and tar installed (Windows 10 1803 or newer):

```powershell
irm https://releases.herdr.dev/app/install.ps1 | iex
```

Herdr appears in the Start menu. The installer adds `%LOCALAPPDATA%\Programs\herdr-app` to your user PATH. Open a new terminal to pick it up.

The shell and PowerShell installers download and run code. Read the [shell installer](distribution/install.sh) or [PowerShell installer](distribution/install.ps1) first if you prefer.
They also install [herdr-core](https://github.com/herdrdev/herdr-core) if it is missing.
The app can help install or update a core it cannot use.

## Find your work

A **space** holds work on one machine, usually in one repo. Each space has **tabs**. Split a tab into **panes** to put terminals or mod panels beside each other.

An unnamed space follows its first terminal's folder. Name a space or tab when you want a label that stays.
New tabs start in the focused terminal's folder.

The agents sidebar brings agents from your spaces into one list, including workers started by another agent.
Agents that report status show as:

- **Working:** still busy.
- **Blocked:** waiting for you.
- **Done:** finished while you were looking elsewhere.
- **Idle:** not working, or a finished agent you have looked at.

Click an agent to see it. A worker without a pane opens in **peek**, over your tab. Keep it as a tab if you want it beside your other work.
Ctrl-click a live terminal address in terminal text to peek at it; use Cmd-click on macOS.
An address such as `t7` means a terminal on the same machine as the terminal showing it. `vps/t7` names one on another machine.

The window arranges terminals. The core runs them.
Closing the window leaves them running; closing a pane or tab ends its terminals.
After a reboot or core crash, kept panes get fresh shells in their last folders, not their old programs.

## Work on another machine

Add an SSH machine from Settings → Machines or the sidebar's Add machine action.
Pick a host from your SSH config or type how SSH reaches it.
Spaces on that machine run their terminals there, not on your computer.

Each machine keeps its spaces, tabs and splits in its core. Connected screens share that structure and the live terminals, but keep their own focus and zoom.
Remote work keeps running when your local window closes or your laptop sleeps, as long as the remote machine and core stay up.

## Make it yours

Open the command palette to find an action by name, or open Settings to change fonts, colors, sounds and other preferences.
The built-in layouts include Herdr, Terminals and Herdr 98.
A **layout** arranges the window. **Mods** are small JavaScript programs that draw its panels, menus and dialogs.

Duplicate a shipped layout or mod before changing its structure or code. Your copies stay yours; Herdr updates the shipped ones on start.
Save a personal layout, mod or key file and the window reloads it.
A broken edit keeps the last good version and reports the error.

Ctrl+Shift+P shows every command and its key (Cmd+Shift+P on macOS). Change any key in `keys.json`; see [the keys section](defaults/AGENTS.md#keys-keysjson) of the mod guide for the format, including JSON objects for named, typed command arguments.

[Layouts and mods](docs/mods.md) · [Commands](docs/commands.md)

## Updates and privacy

Installed releases check for updates in the background and download a new app beside the running one. It takes over when you quit and reopen, or choose the update action. Your terminals keep running in the core.
Settings → Updates shows what is available. What's New opens after an update; you can open it again from the command palette.
Builds from source do not update themselves.

Usage telemetry is on by default in releases. Turn it off in Settings → Privacy.
Mods are trusted code, not sandboxed extensions. Read [security and privacy](docs/security.md) before installing someone else's mod.

## For agents

```sh
herdr-app --skill
```

This prints the agent guide for driving the window. The herdr-core skill points agents here.
Use herdr-core for running and coordinating programs; use the app to change what you see.

```sh
herdr-app ls
herdr-app status
herdr-app commands
```

The first two inspect the open window. The command list also works without one.
See the [command reference](docs/commands.md) for arguments.

## Build from source

For bugs, mod API requests and code contributions, see [Contributing](CONTRIBUTING.md).

Clone this repo with its submodules. Use Rust 1.95.0 and Zig 0.16.0, the versions used in [CI](.github/workflows/ci.yml).
Linux needs the development packages listed in that workflow.

```sh
git clone --recurse-submodules https://github.com/callumw-k/herdr-app.git
cd herdr-app
just build
just test
just install
```

Install [just](https://github.com/casey/just) to use these recipes.
The platform installer builds a release and adds the app to your desktop.
On Linux it links to this checkout, so keep the checkout in place.
Quit and reopen Herdr to use the new build.

Documentation checks also need [Bun](https://bun.sh): `just docs-check` checks generated pages and CLI examples; `just docs-generate` refreshes the reference pages.

To compare drawing changes on Windows, Linux or macOS, run the same fixed workloads against each build:

```sh
python scripts/bench.py --bin <app executable> --set spinners --seconds 20
```

The runner requires `hc` on PATH and uses the Python executable running the script. It prints a unique artifact folder with the run settings, counters and results, then stops its private app, terminals and core. Every set uses Python fixture programs; the default set’s TUI workload replaces historical `btop` measurements. `--only spinners` measures just the quiet native spinner scenario. `--set titles` runs eight agents whose titles spin after a prefix (`π ⠹ task`) on the main layout. Keep the window visible and the display awake, and compare the same window activity and display settings; inactive windows may draw at a lower rate. Windows records visibility, foreground activity and display Hz at each measurement boundary; other platforms mark these observations unavailable. Visibility does not establish occlusion. Draws are window render calls. Render-to-paint elapsed time stops before scene finalization, backend drawing, uploads and presentation; it is separate from OS process CPU time. Neither measures physical presentation or the FPS HUD’s derived MAX FPS.

`python scripts/bench_compare.py ghostty --root <artifact folder>` reuses a run’s fake agent for the macOS comparison. Benchmark checks: `python -m unittest discover -s scripts -p test_bench.py`.
