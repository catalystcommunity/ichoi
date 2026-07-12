# Accessibility — Ichoi default theme

Accessibility is a first-commit requirement (DESIGN §11), not a later pass. This
theme targets **WCAG 2.1 AA** and is verified manually on **Firefox** (the
project's first-class browser). The same i18n and a11y primitives are available to
third-party themes.

## Structure & semantics

- **Landmarks.** One `<nav aria-label="Primary">` rail, one `<main id="main-content">`,
  one `<footer>` transport. A **skip link** (`.skip-link`) is the first focusable
  element and jumps to `#main-content`.
- **Headings.** Each screen opens with a single `<h1>`; sections use `<h2>` tied to
  their region via `aria-labelledby`.
- **Lists.** Album grids, track lists, and the server switcher are real `<ul>/<li>`
  with `role="list"` (preserved even where CSS removes bullets).
- **Native controls.** Everything actionable is a `<button>`, `<a>`, `<input>`, or
  `<select>` — no click-only `<div>`s — so keyboard and AT semantics come for free.

## Keyboard

- Every interactive element is reachable and operable by keyboard; tab order
  follows source order (rail → main → transport).
- Track rows, album tiles, and jukebox transport are buttons: **Enter/Space**
  activate them.
- **Sliders** (scrubber, volume) are `<input type="range">` — arrow keys adjust.
- The **modal dialog** (`Dialog.tsx`) moves focus inside on open, restores it to
  the trigger on close, closes on **Escape**, and closes on backdrop click. It is
  `role="dialog" aria-modal="true"` and labelled by its heading.

## Focus visibility

- `:focus-visible` draws a 2px signal-cyan outline with offset on every focusable
  element. Focus styling is never removed — only refined.

## ARIA

- Icons are `aria-hidden` and decorative; every icon-only button carries an
  `aria-label` (localized).
- The nav uses the router's `aria-current="page"` on the active link (also the
  styling hook, so the visual and programmatic "current" never diverge).
- Live connection status uses a text label alongside the color dot, so state is
  never conveyed by color alone. Segmented/toggle controls expose
  `aria-pressed` / `aria-checked`; the theme picker is a `role="radiogroup"`.
- `Spinner` is `role="status" aria-live="polite"`.

## Color & contrast

- Body and UI text meet AA contrast in both themes. Status is always paired with
  text — the amber "on air" lamp is accompanied by an "On air / Idle" label, and
  lossless/role badges carry text, not just hue.

## Motion

- `prefers-reduced-motion: reduce` collapses all animation and transition
  durations to ~0 and **freezes the signal meter** at a static level. The meter,
  connection pulse, and hover lifts are the only motion, and all are gated.

## Theme & appearance

- `prefers-color-scheme` is honored by default; the user's explicit choice (and
  the server's `theme` setting) override it via `<html data-theme>`, which wins in
  both directions so there is no flash of the wrong theme.
- `color-scheme` is declared so native form controls and scrollbars match.

## Internationalization

- All user-facing strings resolve through the i18n layer (`t("…")`); there are no
  hard-coded English strings in components. `<html lang>` is updated on locale
  change. Layout uses logical, wrap-friendly flex/grid so translated strings that
  run longer do not clip.

## Known gaps / follow-ups

- Drag-to-reorder the queue is not implemented; reordering is keyboard-first once
  added (the CSIL `reorder` command exists).
- A roving-tabindex enhancement for very long track lists would reduce tab stops;
  today every row is individually tabbable.
- Automated axe/Lighthouse runs and Firefox screen-reader passes should be wired
  into CI alongside the playwright-mcp check named in DESIGN §11.
