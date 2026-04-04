# Dimkey Site Redesign — Content + Design Upgrade

**Date**: 2026-04-04
**Status**: Approved
**Repo**: dimkey-site (public, GitHub Pages, dimkey.com)

## Goal

Upgrade the existing single-page product site with:
1. Missing feature content (restore capability, PDF/TXT support)
2. Visual refinement toward Typora-style minimalism (more whitespace, less borders, stronger typography hierarchy)
3. Hero区 animated redaction demo replacing empty screenshot placeholder
4. Scroll-triggered fade-in micro-animations

## Design Decisions

- **Style direction**: Typora-like — restrained, typography-driven, generous whitespace, no decorative clutter
- **Language**: English only (international audience)
- **Download links**: Keep as placeholder (#), both platforms not yet public
- **Tech**: Single index.html, Tailwind CDN, zero dependencies, pure CSS/JS animations
- **No framework, no build step** — stays as static HTML

## Page Structure (top to bottom)

### 1. Nav
- No structural change, micro-adjust spacing

### 2. Hero
- Title: bump to `text-5xl sm:text-6xl`, tighter tracking
- Subtitle: relax line-height, max-w-2xl
- CTA buttons: increase gap
- **NEW: Animated redaction demo** below CTAs, replacing screenshot placeholder
  - Shows a realistic text line (e.g. contract snippet with name, ID, amount)
  - Sensitive words get covered by ink-900 blocks with smooth CSS transition
  - After 2s pause, blocks fade away (restore) then cycle repeats
  - Pure CSS @keyframes + minimal JS for sequencing (~30 lines)
  - Monospace font for the demo text to feel "document-like"

### 3. Trust Bar
- Remove background color and border-y
- Pure text + icons with generous spacing
- Lighter, more breathable

### 4. How It Works — 4 Steps
- Add Step 4: "Restore anytime" — redacted files carry an encrypted mapping; only you can reverse
- Add connecting arrows/lines between steps for flow visualization
- Keep centered layout

### 5. What It Detects
- Keep 2×2 card grid
- Remove card borders, use subtle bottom-line or pure spacing separation
- Typora-style: content speaks, chrome disappears

### 6. Redact ↔ Restore (NEW section)
- Headline: "Redact now. Restore later."
- Short copy: explain that Dimkey saves a local mapping file, allowing full restoration. Only the original machine can restore. No cloud, no key server.
- Minimal visual — possibly a simple bidirectional arrow icon

### 7. Use Cases
- Add third case: **Consulting** (consultants redacting deliverables)
- Keep card-based layout, 3 columns on desktop

### 8. Download
- Keep dark (ink-900) background
- Update supported formats: Excel, Word, PDF, CSV, TXT
- macOS + Windows buttons remain placeholder
- Update "macOS 11+" requirement text

### 9. Footer
- Update copyright to 2026

## Visual Upgrades

| Aspect | Before | After |
|--------|--------|-------|
| Hero title | text-4xl/5xl | text-5xl/6xl |
| Section titles | text-2xl | text-3xl |
| Section spacing | py-24 | py-28 or py-32 |
| Card borders | border border-ink-100 | Remove or minimal bottom-line |
| Trust bar | bg-ink-50 border-y | Transparent, pure spacing |
| Border radius | Mixed | Unified rounded-xl |
| Color palette | No change | Keep ink + cream + gold |

## Micro-animations

- **Scroll fade-in**: Each section fades in (opacity 0→1, translateY 20→0, 0.6s ease) on viewport entry
- Implementation: Intersection Observer, ~15 lines vanilla JS
- **Hero redaction animation**: CSS @keyframes + JS timing, ~30 lines
- Zero external dependencies

## Content Updates

- Supported formats: add PDF, TXT
- New restore feature section
- New consulting use case
- Copyright 2025 → 2026
- Keep all English copy

## Out of Scope

- No i18n / language switching
- No real download links (placeholder only)
- No product screenshots (animation replaces this)
- No analytics / tracking scripts
- No build tooling — stays as single HTML file
