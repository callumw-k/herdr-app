# Rail sizing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use the execution skill named at the plan handoff (superpowers:subagent-driven-development or superpowers:executing-plans) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A 52px collapsed sidebar whose rows, markers, icons and click targets are all bigger.

**Architecture:** Three default mods draw the rail when the `sidebar` node is under 160px: `spaces` (top list), `agents` (bottom list and "»"), `workspace-chrome` (brand row and footer). Each changes only its rail branch. Rail rows stop using a fixed 32px width and fill the list, which gets a 4px side gutter and 4px between rows.

**Tech Stack:** JS mods on gpui-kit / gpui-base element builders, tests in `bun test`.

**Spec:** `docs/superpowers/specs/2026-11-10-rail-sizing-design.md`

## Global Constraints

- Rail width 52px on every platform (was 76 on macOS, 44 elsewhere).
- Rail rows are the list width minus a 4px gutter each side (44 at 52px), never a fixed width.
- 4px between rail rows; rail row corner radius 8.
- The expanded sidebar (`narrow()` false) renders exactly as before. Every change sits behind the existing `collapsed` checks.
- The footer block stays 69px in `defaults/layouts/main.json`; no layout file changes.
- No code comments unless deleting one would let a reader get something wrong.

## Review Focus

- A rail saved at 76px or dragged to any width under 160px: rows fill it with the 4px gutter, nothing stays a centred 32px box. Checked by eye on macOS (no renderer on the Linux box).
- Two-digit numbers ("12") with the 16px "!" badge fit inside a 44px row without clipping. Checked by eye.
- The macOS window buttons overhang the tab strip at 52px and the tabs mod still clears them (`defaults/mods/tabs/main.js:88` already computes this from the sidebar width; nothing to change, confirm by eye).
- The two `toggleSidebar` copies (agents, workspace-chrome) both fold to 52. Pinned by the Task 1 test.
- Footer contents fit the 69px block: top padding 4 + 30 + 30 + bottom padding 4 = 68.

---

### Task 1: Fold the rail to 52px everywhere

**Files:**
- Modify: `defaults/mods/agents/main.js:20-26` (`toggleSidebar`)
- Modify: `defaults/mods/workspace-chrome/main.js:18-24` (`toggleSidebar`)
- Modify: `defaults/AGENTS.md:482-484`
- Test: `scripts/agents.test.ts`

**Interfaces:**
- Consumes: none
- Produces: none

**Depends on:** none

- [ ] **Step 1: Write the failing test** in `scripts/agents.test.ts`

`test("both rail toggles fold to 52 on every platform and unfold to the saved width")`. For each of `defaults/mods/agents/main.js` and `defaults/mods/workspace-chrome/main.js`, strip imports as the file's other tests do, then build `toggleSidebar` with `new Function(...)` injecting `layout`, `window`, `shared`, `setShared`, `run` (and the other names each module references at top level: `View` for both). For `platform` in `["macos", "linux", "windows"]`:
  - With `layout()` returning `{ id: "sidebar", size: { px: 268 } }`, calling `toggleSidebar()` records `["layout.resize", { id: "sidebar", px: 52 }]` and `setShared("sidebar.width", 268)`.
  - With `layout()` returning px 52 and `shared("sidebar.width")` returning 300, it records `["layout.resize", { id: "sidebar", px: 300 }]`.

- [ ] **Step 2: Run it and see it fail**

Run: `bun test scripts/agents.test.ts`
Expected: FAIL, macOS folds to 76 and Linux to 44.

- [ ] **Step 3: Change both `toggleSidebar` copies** to fold to `52` with no platform branch. Drop `window` from a mod's `herdr` import only if this made it unused (it stays used in `workspace-chrome`).

- [ ] **Step 4: Update `defaults/AGENTS.md:482-484`** to: "Narrower than 160px, the sidebar mods draw a compact rail; folding resizes `sidebar` to 52. On macOS the window buttons overhang the tab strip, and the tabs leave room for them when the sidebar is narrower."

- [ ] **Step 5: Run the tests**

Run: `bun test scripts/`
Expected: all pass, `docs.test.ts` included.

- [ ] **Step 6: Commit**

```bash
git add defaults/mods/agents/main.js defaults/mods/workspace-chrome/main.js defaults/AGENTS.md scripts/agents.test.ts
git commit -m "fix: fold the sidebar to a 52px rail on every platform"
```

### Task 2: Spaces rail at the new sizes

**Files:**
- Modify: `defaults/mods/spaces/main.js` (`RAIL_CELL`/`railCell` at 29-30, `grouped` 298-363, `compact` 447-462, `render` 478-486)

**Interfaces:**
- Consumes: none
- Produces: none

**Depends on:** none

No unit test: the change is sizes in the rail branch, and the existing harness proxies every builder call, so a test would only restate the values. Verified by `bun test scripts/spaces.test.ts` staying green and by eye.

- [ ] **Step 1: Make rail cells fill the list.** Remove `RAIL_CELL` and change `railCell` so the cell is `w_full()` (callers stop setting `.w(RAIL_CELL)`). In `render`, the list gets `.px(collapsed ? 4 : 8)` and `.when(collapsed, el => el.gap(4))`.

- [ ] **Step 2: `compact`** row: `h(34)`, `rounded(8)`, `text_size(15)`; number slot `w(18)`; status slot `w(16)`; dot `size(9)` when `glyph(w.status) === "●"`, else `size(4)`.

- [ ] **Step 3: Header "+" (`space-new`)** in the rail: `size(36)`, `rounded(8)`, icon `size(16)`. Expanded keeps 26 and 14.

- [ ] **Step 4: Rail-only sizes in `grouped`:** machine head `h(30)` (was 26) and `rounded(8)`, its icon `size(16)` (expanded stays 13), badge `size(7)` with `rounded(4)`; empty "new space there" cell `h(34)`, `rounded(8)`, outlined box `size(22)` with `rounded(6)`, plus icon `size(12)`; `loadingRow` `h(34)` in the rail. Drop the head's `mt(4)` in the rail: the list's gap spaces it now.

- [ ] **Step 5: Run the tests**

Run: `bun test scripts/spaces.test.ts`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add defaults/mods/spaces/main.js
git commit -m "fix: bigger space rows and buttons in the sidebar rail"
```

### Task 3: Agents rail at the new sizes

**Files:**
- Modify: `defaults/mods/agents/main.js` (`status` 202-211, collapsed `row` branch 307-321, list and "»" in `render` 452-475)

**Interfaces:**
- Consumes: none
- Produces: none

**Depends on:** Task 1 (same file)

- [ ] **Step 1: Give `status` a rail size.** Add a third parameter `big = false` to `status(t, a, compact = false, big = false)`. When `big`: spinner `size(14)`, "!" badge `size(16)` with `text_size(12)` and `rounded(5)`, ring and dot `size(9)` with `rounded(5)`. Only the collapsed row passes `true`; every other caller is unchanged.

- [ ] **Step 2: Collapsed row:** drop the `railCell`-style wrapper; the row is `w_full()`, `h(34)`, `rounded(8)`, `gap(2)`, `text_size(15)`; number slot `w(18)`; status slot `w(16)` and calls `this.status(t, a, true, true)`.

- [ ] **Step 3: List in the rail:** `.px(collapsed ? 4 : 8)` and `.when(collapsed, el => el.gap(4).pt(6))`.

- [ ] **Step 4: "»" button:** the `collapse` element becomes `w(44)`, `h(30)`, `rounded(8)`, centred content, `text_size(15)`, `hover` sets `bg(t.hover)` and `text_color(t.foreground)`. Its wrapper row drops the fixed `h(28)` for `py(4)` and stays centred.

- [ ] **Step 5: Run the tests**

Run: `bun test scripts/agents.test.ts`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add defaults/mods/agents/main.js
git commit -m "fix: bigger agent rows and expand button in the sidebar rail"
```

### Task 4: Footer and brand row in the rail

**Files:**
- Modify: `defaults/mods/workspace-chrome/main.js:26-54` (`button`, footer `row`, brand row)

**Interfaces:**
- Consumes: none
- Produces: none

**Depends on:** Task 1 (same file)

- [ ] **Step 1: Footer rows in the rail:** `w_full()` (replacing `w(32)`), `h(30)`, `rounded(8)`, icon `size(18)` (expanded stays 14 and `h(28)`). The footer column in the rail uses `px(4)`, `pt(4)`, `pb(4)` and stretches its children instead of `items_center()`. The update-ready dot stays `absolute()`, positioned `top(5)` and `right(10)` so it sits on the larger icon's corner.

- [ ] **Step 2: Linux brand-row toggle in the rail:** `button` takes an optional `size = 26`; the rail's `sidebar-toggle` passes `36` with icon `size(16)`. The brand row in the rail uses `pl(8)`, `pr(8)` and centres its child.

- [ ] **Step 3: Run the full suite**

Run: `bun test scripts/`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add defaults/mods/workspace-chrome/main.js
git commit -m "fix: bigger footer and toggle buttons in the sidebar rail"
```

### Task 5: Check on macOS

**Depends on:** Task 1, Task 2, Task 3, Task 4

- [ ] **Step 1:** With Callum's go-ahead, push `worktree-rail-sizing` to `origin`.
- [ ] **Step 2:** Callum checks the branch out on the Mac, runs `scripts/install-local`, reopens Herdr, folds the sidebar, and checks each Review Focus line by eye. Fixes found here go back to the task that owns the file.
