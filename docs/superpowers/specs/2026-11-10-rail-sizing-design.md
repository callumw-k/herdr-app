# Collapsed sidebar: narrower, bigger, easier to hit

## Intent

The collapsed sidebar (the rail) reads as sparse: a 76px strip on macOS with a 32px cell in the
middle, 13px numbers and 3–7px status dots, and rows whose highlights touch. Callum wants
everything in it bigger, the rail itself narrower, and every click target larger. The layout
(spaces above, agents below, footer at the bottom, the 50/50 split) stays as it is.

## Goals

- Rail width drops from 76px to 52px on macOS. Linux goes from 44px to 52px so both match.
- Rows fill the rail instead of sitting as a fixed box in the middle.
- Numbers, status markers and icons get bigger.
- Every clickable thing in the rail gets a larger hit area.

## Non-goals

- The expanded sidebar is untouched.
- No change to what the rail shows (numbers and status, machine headers, "+", the footer).
- No change to the spaces/agents split or the layout file's structure.

## Approaches

- **A. 52px rail, traffic lights overhang into the tab strip (picked).** The app already handles a
  sidebar narrower than the macOS window buttons: the tabs mod pads itself to clear them
  (`defaults/mods/tabs/main.js:88`) and the divider starts below the toolbar
  (`src/window.rs:46`). It's the only option that makes the rail both narrower and bigger.
- B. Keep the traffic lights inside the rail. The floor is about 70px, so the width barely changes.
- C. Stack the number above its status to reach about 40px. Rows get taller and stop reading as one
  line.

## Design

### Width

`toggleSidebar` in both `agents` and `workspace-chrome` resizes the rail to 52 on every platform
(it's duplicated in each mod today, and stays that way). `defaults/AGENTS.md` changes its "44 (76
on macOS…)" sentence to say 52, and notes that on macOS the window buttons overhang the tab strip.

A rail someone saved at 76 or dragged to another width below 160px still works: rows size to the
rail with a 4px gutter each side instead of a fixed width.

### Rows (spaces and agents)

| | Now | New |
|---|---|---|
| Row box | 32 × 26, centred | rail width − 8 (44 at 52px) × 34 |
| Space between rows | 0 | 4px |
| Corner radius | 6 | 8 |
| Number | 13px, in a 14px slot | 15px, in an 18px slot |
| Status slot | 14px | 16px |
| Space dot (needs you / done) | 7px | 9px |
| Space dot (quiet) | 3px | 4px |
| Agent ring / dot | 7px | 9px |
| Agent spinner | 12px | 14px |
| Agent "!" badge | 14px, 10.5px text | 16px, 12px text |

The whole row stays the click target, as it is today.

### Spaces header and machine rows

- The "+" (new space) button: 26 → 36px square, icon 14 → 16px.
- Machine header (monitor/server icon): row width as above, 26 → 30px tall, icon 13 → 16px, the
  connection badge 6 → 7px.
- "New space there" on an empty machine: row width as above, 34px tall, its outlined box 18 → 22px.
- The "…" loading row: 34px tall.

### Agents' "»" (expand)

The "»" becomes a 44 × 30 button with a hover background, 15px text, instead of bare text in a
28px strip.

### Footer (settings, add machine)

The footer block is fixed at 69px in `defaults/layouts/main.json`, which leaves room for two 30px
rows. In the rail each becomes a full-width 44 × 30 button with an 18px icon (from 32 × 28, 14px).
The update-ready dot moves with the icon.

## Left to the builder

- Exact vertical padding inside the spaces and agents lists, so the first row doesn't crowd the
  section edge.
- Whether the brand row on Linux (the sidebar toggle) also grows its button to 36px. Match the
  spaces "+" if it does.

## Checking it

There's no rendering on the Linux box (no Weston), so Callum checks it on macOS after
`scripts/install-local`. The mod tests (`bun test scripts/`) must still pass.
