# UI Tokens

Every design value lives here. Components use CSS variables. No raw values in components. The interactive prototype, `design/sentinel-prototype.html`, uses these exact names and is the visual reference.

## Design Direction

**Graphite workbench.** A quiet, neutral surface like the coding tools people already use (Claude Code, Codex, Antigravity), so the layout is familiar on day one: chats on the left, one stream in the middle, panels that slide in on the right.

Color is rare and carries meaning. One amber accent marks the main action, focus, and the active tab. Status colors mark task state. Everything else is grey.

**Why it avoids generic defaults:** no gradients, no glass, no stock illustrations, no emoji icons. Status uses shape, label, and color together. Users can change theme, accent, tint, fonts, size, density, and radius in Settings.

## Colors

### Dark Theme

| Token | Value | Use |
|---|---|---|
| `--bg` | `#191919` | Stream and panel background |
| `--sidebar` | `#141414` | Chat sidebar |
| `--surface` | `#212121` | Cards, composer, popovers |
| `--surface2` | `#2B2B2B` | Hover, selected row, user message |
| `--border` | `#303030` | Dividers, outlines |
| `--text` | `#ECECEA` | Primary text |
| `--muted` | `#9B9A96` | Secondary text |
| `--accent` | `#E8963A` | Main action, focus, active tab |
| `--accent-fg` | `#1A1006` | Text on accent |
| `--ok` | `#4CC38A` | Verified, healthy |
| `--warn` | `#E5B93C` | Partial, near cap, note |
| `--danger` | `#EF6A6E` | Blocked, violation, kill |
| `--info` | `#6BA8FF` | Running |
| `--lock` | `#B994FF` | Reserved files and waiting |

### Light Theme

| Token | Value |
|---|---|
| `--bg` | `#FAFAF9` |
| `--sidebar` | `#F2F1EF` |
| `--surface` | `#FFFFFF` |
| `--surface2` | `#EDECE9` |
| `--border` | `#E0DED9` |
| `--text` | `#1D1D1B` |
| `--muted` | `#6B6A66` |
| `--accent` | `#B35F09` |
| `--accent-fg` | `#FFFFFF` |
| `--ok` | `#1E7F55` |
| `--warn` | `#8A6A00` |
| `--danger` | `#B3262B` |
| `--info` | `#1F5FBF` |
| `--lock` | `#6B3FC4` |

### Surface Tints (Settings)

| Tint | Dark `bg / sidebar / surface / surface2 / border` | Light `bg / sidebar / surface / surface2 / border` |
|---|---|---|
| Neutral | default values above | default values above |
| Warm | `#1B1917 / #161412 / #242120 / #2E2A28 / #34302D` | `#FBF9F6 / #F3EFE9 / #FFFFFF / #ECE6DD / #E2DAD0` |
| Cool | `#16191D / #12151A / #1E2228 / #272C34 / #2C323B` | `#F7F9FB / #EEF2F6 / #FFFFFF / #E5EBF1 / #D9E1E9` |

### Accent Options (Settings)

Six swatches: `#E8963A` (amber), `#5B8DEF` (blue), `#3DAA7D` (green), `#9B7BEA` (violet), `#E5668B` (rose), `#A8A8A4` (grey). A custom color picker is also offered. The text on the accent (`--accent-fg`) is chosen automatically by luminance: dark text on light accents, white on dark ones. The default accent differs per theme to hold contrast.

## Contrast Rules

- Body text on its surface: at least 4.5:1.
- Large text and UI borders that carry meaning: at least 3:1.
- Status is never color alone. Every status has a label and a shape.
- Settings, Appearance shows a live readout for text, muted text, accent on background, and button text, and marks each pass or fail.
- `cargo xtask contrast-check` checks every text and surface pair in both themes. It is the authority. The hex values above are the starting set.

## Typography

| Setting | Default | Choices |
|---|---|---|
| Interface font `--font-ui` | Inter | System, Inter, IBM Plex Sans, DM Sans, Source Serif 4, Atkinson Hyperlegible |
| Code font `--font-mono` | JetBrains Mono | System mono, JetBrains Mono, IBM Plex Mono, Fira Code, Source Code Pro |

Fonts are bundled with the app in the product, with no external requests. The prototype loads them from Google Fonts. Each family needs a fallback stack. License checks are tracked in `library-docs.md`.

**Size.** `--fs` is the base size, 14 px by default, adjustable from 12 to 18 px. All sizes are in `em`, so the whole interface scales with it.

| Scale | Size | Use |
|---|---|---|
| Label | 0.82em | Badges, chips, hints |
| Secondary | 0.9em | Table text, descriptions |
| Body | 1em | Stream, controls |
| Title | 1.05em | Modal titles |
| Page title | 1.5em | Empty state, setup steps |
| Figure | 2.1em | Cost figure |

Weights: 400 body, 500 labels, 600 titles. Numbers use tabular figures. Agent output, paths, and diffs use the code font. The stream width `--mw` defaults to 760 px (adjustable 560 to 980) to keep lines readable.

## Spacing and Density

Spacing is in `em`. `--d` is a density multiplier: 0.8 compact, 1 default, 1.25 spacious. It scales the gap between stream items and card padding.

## Radius, Borders, Elevation

| Token | Value |
|---|---|
| `--r` | 8 px default, adjustable 2 to 16 |
| Cards | 1.4 times `--r` |
| Composer | 2 times `--r` |
| Pills and badges | 999 px |
| Borders | 1 px `--border` |
| Popover, modal shadow | `0 10px 36px rgba(0,0,0,.16)` |

Cards use a border, not a shadow.

## Focus

2 px outline in `--accent`, 2 px offset. Always visible on keyboard focus. Never removed.

## Motion

| Use | Duration |
|---|---|
| Hover, toggle | 150 ms |
| Popover, new stream item | 150 to 250 ms |
| Panel and sidebar slide | 220 to 250 ms |

Motion explains change: a panel sliding in, a card appearing. No decorative loops. `prefers-reduced-motion` turns motion off.

## Layout Sizes

| Item | Size |
|---|---|
| Sidebar | 264 px |
| Right panel | 440 px. Expanded: `min(780px, 62%)` |
| Top bar | 3.5em (3.3em on phone) |
| Tab bar (phone) | About 3.4em |
| Icon button | 2.2em. On phone 3em (at least 44 px) |
| Stream width | `--mw`, 760 px default |
| Modal | `min(1000px, 100%)` wide, `min(720px, 100%)` tall |

## Breakpoints

| Name | Width | Layout |
|---|---|---|
| `phone` | under 620 px | One column. Bottom tabs. Sidebar is a drawer. Panels are bottom sheets |
| `tablet` | 620 to 1179 px | One column beside a docked, collapsible sidebar. Panels slide over the stream |
| `desktop` | 1180 px and up | Sidebar, stream, panels side by side |

Layout follows the width of the app, not the device type. The sidebar is open by default at tablet width and above.

## Toasts

Surface background, text color, and a 1 px border. No colored side bar. The kind (ok, warn, danger, info) is shown only by the icon color: `--ok`, `--warn`, `--danger`, `--info`. Same shadow as popovers. Centered above the composer (above the tab bar on phone), at most 30em wide, visible about 3 seconds, `role="status"`. Toast text must pass the contrast rules in both themes.

## Layers

| Layer | Value |
|---|---|
| Base | 0 |
| Scrim | 35 |
| Sidebar and panels (slide-over) | 40 |
| Phone tab bar | 45 |
| Popover | 50 |
| Modal, settings, guide | 60 |
| Setup wizard | 70 |
| Command palette | 85 |
| Tour | 90 |
| Toast | 95 |

## Icons

One outline set drawn as inline SVG, 1.7 px stroke, sized in `em` so icons scale with text. No emoji. Icons pair with a text label except in the top bar, where each has a tooltip and an accessible name, and in the phone tab bar, where the label sits under the icon. Names are listed in `ui-registry.md`.

Settings uses a cog. The theme switch uses a sun (in dark) or a moon (in light). The two must never look alike, because they sit side by side in the sidebar footer. The theme switch says what it will do: "Switch to light theme".

## Charts

- Cost over time: line, with the cap as a dashed line in `--danger`.
- Per-agent cost: horizontal bars in each agent's color, with an "est" label when the tool does not report cost.
- Every chart has a table or text alternative.

## Status Mapping

| Status | Token | Shape | Label |
|---|---|---|---|
| Ready, Planned | `--muted` | hollow circle | Ready |
| Waiting | `--lock` | square | Waiting |
| Running | `--info` | triangle | Running |
| Verifying | `--warn` | diamond | Verifying |
| VERIFIED | `--ok` | filled circle | Verified |
| BLOCKED | `--danger` | octagon | Blocked |
| CONTRADICTION | `--danger` | split circle | Contradiction |
| SKIPPED | `--muted` | dash | Skipped |
| PARTIAL | `--warn` | half circle | Partial |

## Lesson and Note Mapping

| State | Token | Shape | Label |
|---|---|---|---|
| Candidate | `--muted` | dotted ring | Candidate |
| Proven | `--ok` | ring with check | Proven |
| Rejected | `--danger` | ring with bar | Rejected |
| Retired | `--muted` | dashed ring | Retired |
| Quality note | `--warn` | hexagon | Note |

Rings are used for lessons so they never read as task statuses.

Brain note types use these tokens in lists and in the graph: Convention `--info`, Decision `--accent`, Fix `--ok`, File `--muted`, Task `--lock`, Run `--warn`, Lesson `--ok`, Glossary `--muted`, How-to `--info`. Strength bars use `--ok`.

## Settings-Driven Tokens

These can be changed in Settings, Appearance. They are stored on each device, so the phone and the desktop can differ.

| Setting | Token | Range |
|---|---|---|
| Theme | `data-theme` | System, light, dark |
| Accent | `--accent`, `--accent-fg` | Six swatches or custom |
| Surface tint | `--bg`, `--sidebar`, `--surface`, `--surface2`, `--border` | Neutral, warm, cool |
| Interface font | `--font-ui` | Six choices |
| Code font | `--font-mono` | Five choices |
| Text size | `--fs` | 12 to 18 px |
| Stream width | `--mw` | 560 to 980 px |
| Density | `--d` | 0.8, 1, 1.25 |
| Corner radius | `--r` | 2 to 16 px |
| Color tokens | `--bg`, `--sidebar`, `--surface`, `--text`, `--muted`, `--accent`, `--ok`, `--warn`, `--danger`, `--info` | Any color, kept per theme |

Reset restores every value above. "Copy as CSS" exports the current tokens.

## Themes as CSS Variables

```css
:root {
  --bg: #FAFAF9; --sidebar: #F2F1EF; --surface: #FFFFFF; --surface2: #EDECE9;
  --border: #E0DED9; --text: #1D1D1B; --muted: #6B6A66;
  --accent: #B35F09; --accent-fg: #FFFFFF;
  --ok: #1E7F55; --warn: #8A6A00; --danger: #B3262B; --info: #1F5FBF; --lock: #6B3FC4;
  --font-ui: Inter, system-ui, -apple-system, "Segoe UI", sans-serif;
  --font-mono: "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace;
  --fs: 14px; --d: 1; --r: 8px; --mw: 760px;
  color-scheme: light;
}

:root[data-theme="dark"] {
  --bg: #191919; --sidebar: #141414; --surface: #212121; --surface2: #2B2B2B;
  --border: #303030; --text: #ECECEA; --muted: #9B9A96;
  --accent: #E8963A; --accent-fg: #1A1006;
  --ok: #4CC38A; --warn: #E5B93C; --danger: #EF6A6E; --info: #6BA8FF; --lock: #B994FF;
  color-scheme: dark;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { /* same values as [data-theme="dark"] */ }
}
```

The theme follows the system by default and can be set in Settings. When the user changes a value in Settings, the app writes it as an inline custom property on the root, which overrides the stylesheet.
