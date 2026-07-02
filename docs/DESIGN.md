# Grafiki — Design System (source of truth)

> Derived from the user's reference screenshots in `Inspiration/` (Granola dark,
> Wispr Flow light, Linear dark), Jul 1 2026. Pairs with `docs/UX_REDESIGN.md`
> (screens & flows). This file defines how everything LOOKS and FEELS.

---

## 1. What the three references share (the DNA we adopt)

1. **Sheet-on-frame layout.** A soft gray window frame with the content area as
   a large rounded "sheet" (Wispr). Sidebar lives on the frame, content on the
   sheet. Instant premium feel, zero chrome.
2. **One accent, neutrals do the work.** Wispr: orange keycap. Granola:
   olive-yellow highlights. Linear: purple-gray. Everything else is warm
   neutral. Color = meaning, never decoration.
3. **Serif display for emotional moments, grotesk for UI.** Granola's "Coming
   up" / "Hi Vishal, ask anything"; Wispr's banner headlines ("Flow spells the
   way *you* do"). Body/controls stay sans.
4. **Rows, not cards, for lists.** Hairline-divided rows with a time gutter
   (Wispr transcript, Granola timeline, Linear issues). Cards are reserved for
   stats and promos.
5. **Date-grouped timelines.** "Today / Yesterday / Mon, Jun 29" section
   headers; times right-aligned and de-emphasized (Granola, Wispr).
6. **Page header pattern.** Title left, ONE primary action right (dark pill
   button), underline tabs below, content under that (Wispr Dictionary/
   Snippets, Linear project).
7. **Floating ask bar.** Granola's bottom-center rounded "Ask anything" pill
   with mic + quick actions. The signature ambient-AI affordance.
8. **kbd/keycap chips as first-class UI.** Wispr's orange `fn` key inline in a
   headline, `⌥Opt 1` chips on cards; Linear's keyboard-first everything.
9. **Suggestion banners.** Tinted single-row banner with inline action
   (Granola: "✦ Add teammates from these meetings → [Add to folder]"). This is
   our Review-suggestion pattern.
10. **Settings = modal sheet with its own left nav** (Wispr). Serif section
    titles, grouped rows in soft containers, toggles right.
11. **Detail = content left + properties rail right** (Linear issue/project:
    Status/Priority/Labels rail). Our session/memory detail pattern.
12. **Density where you triage, air where you read.** Linear-dense inbox rows;
    Granola/Wispr-airy reading surfaces.

## 2. Theme strategy — "Snowy Rainforest" (LOCKED, Jul 1 2026)

**Duotone shell (the user's design):** the Evergreen rail/frame is PERMANENT —
it never changes between themes. Only the sheet flips: warm snow in light
mode, deep green-charcoal in dark. The dark rail + light paper reads as "a
dark instrument panel holding a sheet of paper", and it rhymes with the
charcoal terminal well. (Slack-aubergine architecture, evergreen + snow skin.)

Two-level accent: **deep pine does structure** (buttons, links, active states
on the sheet — it replaces ink for interactive elements), **fresh green does
the LIVE tier** (capture pulse, Review badge, mic button). Green listening
dot = the Zoom/Meet convention; and green-brand means approve/reject in Review
gets natural semantics (brand-green approve vs red reject) with zero collision.

## 3. Tokens (CSS custom properties)

```css
:root {
  /* THE FRAME — permanent, both themes (Evergreen) */
  --frame:        #032113;  /* window frame + rail, never changes */
  --rail-ink:     #EAF2EC;  /* rail primary text */
  --rail-ink-2:   #AFC4B5;  /* rail muted text */
  --rail-active:  rgba(255,255,255,0.10); /* active nav wash */

  /* Surfaces (light theme) */
  --sheet:        #FAFAF7;  /* Bright Snow, warmed 2 points for the serif */
  --card:         #FFFFFF;  /* stat/promo cards on sheet */
  --banner-tint:  #E7F0EA;  /* suggestion banner wash */
  --well:         #16181C;  /* terminal / code well — ALWAYS neutral charcoal
                               (ANSI red/green legibility), both themes */

  /* Ink (on sheet) */
  --ink:          #1B1A17;
  --ink-2:        #6E6C66;
  --ink-3:        #A3A199;
  --ink-on-well:  #E8E6E0;

  /* Lines — hairlines everywhere, shadows almost nowhere */
  --hairline:     rgba(27, 26, 23, 0.08);
  --hairline-2:   rgba(27, 26, 23, 0.14);

  /* Accent, two tiers */
  --accent:       #0B4A32;  /* deep pine — STRUCTURE: buttons, links, active */
  --accent-live:  #2E9E6B;  /* fresh green — LIVE: pulse, badge, mic */
  --accent-soft:  #E7F0EA;  /* selection/chip wash */
  --accent-ink:   #0B4A32;  /* accent-colored text on sheet */

  /* Semantics (muted, chips/dots only — never backgrounds) */
  --ok:           #2E9E6B;  /* shares the live green — approve IS brand */
  --warn:         #C7912B;
  --danger:       #C4483C;  /* reject/destructive; no brand collision */

  /* Geometry */
  --r-sheet: 14px; --r-card: 10px; --r-btn: 8px; --r-pill: 999px;
  --shadow-card: 0 1px 2px rgba(3,33,19,.06);
  --frame-margin: 10px;      /* mahogany… evergreen margin around the sheet —
                                keep THIN or the duotone gets heavy */
  --gutter: 24px; --measure: 720px;
}

[data-theme="dark"] {
  /* Frame/rail unchanged — that's the point. Only the sheet flips. */
  --sheet:        #101915;  /* deep green-charcoal paper */
  --card:         #16221C;
  --banner-tint:  #12241B;
  --ink:          #E9EFE9;
  --ink-2:        #A5B3A9;
  --ink-3:        #77857C;
  --hairline:     rgba(255,255,255,0.08);
  --hairline-2:   rgba(255,255,255,0.15);
  --accent:       #58B98A;  /* pine brightens to stay legible on dark */
  --accent-live:  #3FCF8E;
  --accent-soft:  rgba(46,158,107,0.14);
  --accent-ink:   #7FD3AC;
}
```

**Window chrome:** the title bar is part of the frame — evergreen, custom
(Tauri `titleBarStyle` overlay), so the duotone reads as intentional
architecture, not a dark widget on a light app.

## 4. Typography

| Role | Face | Use |
|---|---|---|
| **Display serif** | `Newsreader` (Google; closest to Granola/Wispr's editorial serif), fallback Georgia | Page titles ("Today", "Memory"), hero greetings, banner headlines, big stat numbers. Regular weight, tight leading; *italic* for one emphasized word, Wispr-style. |
| **UI sans** | `Inter` (fallback system-ui) | Everything interactive: nav, rows, buttons, forms. 13px controls / 14px rows / 15px reading. |
| **Mono** | `JetBrains Mono` (existing) | Terminal well, code, ids, kbd chips. |

Scale: 32/26/20 serif display · 15 reading · 14 row title · 13 UI · 12 meta
(11 uppercase-tracked section labels like GRANOLA'S "Spaces" / Linear's group
headers).

## 5. Component inventory (reference → Grafiki)

| Component | Reference | Spec |
|---|---|---|
| **Rail** | Granola/Wispr sidebar + Slack duotone | On `--frame` (evergreen, permanent), `--rail-ink-2` muted labels, active = `--rail-active` white wash + `--rail-ink`, live badge in `--accent-live`, search pinned top with `⌘K` hint, project switcher bottom (Granola workspace pattern). |
| **Sheet** | Wispr | Content in one rounded sheet, `--r-sheet`, hairline border, no shadow. |
| **Page header** | Wispr Dictionary | Serif title left · one dark-pill primary action right · underline tabs under. |
| **Ledger row** | Granola timeline | 40px icon (agent glyph), title + participants/meta line, time right in `--ink-3`. Hover: `--hairline-2` underlay. Date-group headers between. |
| **Inbox row (Review)** | Linear issues | Dense 36px: type icon · title · confidence bar (thin, 32px) · scope chip · source chip · date. Selected = accent-soft wash. Group headers with counts ("Pending 12"). |
| **Ask bar** | Granola bottom pill | Floating bottom-center, rounded-full, icon + placeholder "Ask your memory…" + mic-style accent button. Lives on Home AND Memory. |
| **Suggestion banner** | Granola "Add teammates" | `--banner-tint` row: ✦ icon + sentence + [primary chip action] + dismiss ✕. Used for "3 new memories from your last session → Review". |
| **Promo/education card** | Wispr banners | Dark photo/tint card, serif headline w/ one italic word, sub-line, single button. Used ONLY in onboarding + first-run empty states. |
| **Stat card** | Wispr Insights | White card, big light serif number, 11px uppercase label. Home top strip (sessions this week, memories, pending). |
| **kbd chip** | Wispr `fn` / `⌥Opt 1` | Rounded 6px, hairline border, mono 12px; accent-filled variant for the hero shortcut. Review shortcuts advertised inline ("press `a` to approve"). |
| **Properties rail** | Linear issue detail | Right rail 280px, collapsible groups (Properties/Evidence/Activity), label left value right. Session + memory detail. |
| **Settings sheet** | Wispr settings | Centered modal sheet, own left nav (serif section titles), grouped rows in `--banner-tint` containers, toggles right. |
| **Tabs** | Wispr underline / Linear pills | Underline tabs for page sections; pill tabs for filters. |
| **Buttons** | All three | Primary = dark ink pill (`--ink` bg, white text). Accent reserved for THE key action per screen (Wispr "Create report"). Secondary = hairline ghost. |
| **Dark well** | our terminal | `--well` rounded `--r-card`, 6px padding, JetBrains Mono 13; also used for chat-lens tool-call cards. |

## 6. Per-screen application (see UX_REDESIGN.md for structure)

- **Home:** serif "Today" title; stat strip (3 cards, Wispr Insights); live
  session card = dark well preview strip w/ pulse dot; suggestion banner when
  pending>0; Granola timeline; ask bar floating at bottom.
- **Session live:** terminal well full-bleed in sheet; top hairline bar (sans
  13); "Learned this session" side peek = Linear-rail styling.
- **Session past / memory detail:** Linear pattern — reading column left
  (--measure), properties rail right (provenance, evidence, status).
- **Memory:** serif greeting header ("Ask your memory") + big rounded input w/
  accent focus ring (Granola "Hi Vishal" screen), recents rows under, browse
  tab = dense rows.
- **Review:** Linear inbox exactly — dense grouped rows + right preview pane +
  kbd chips in the header advertising a/r/e/space.
- **Onboarding:** Wispr promo-card language: full-sheet steps, serif headline
  with one italic word ("Your agent forgets. Grafiki *remembers*."), single
  dark pill CTA, progress dots.

## 7. Motion & feel

- Springs we already ship (framer) stay; add: side-peek slide-in, count-up on
  stat numbers, capture pulse (1.6s ease), suggestion banner slide-down.
- Hover = background wash, never movement. Press = 0.985 scale (existing).
- Skeletons on every async surface; no full-screen spinners, ever.

## 8. Rules

1. Accent appears ≤ 3 times per screen (active nav, one action, one status).
2. Serif never appears in a row/list — display surfaces only.
3. Cards only for stats/promos; data lives in rows.
4. Every screen advertises one keyboard shortcut inline as a kbd chip.
5. Empty state = one serif sentence + one action. Never blank.
6. The terminal well is sacred: no chrome inside it but the top hairline bar.
