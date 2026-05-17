// ── icons.js — Shared minimalist SVG icons for toolbars/buttons ──
// All icons: 16x16, stroke 1.5, currentColor. Match the filter SVG in tab-food-recipes.js.

const SVG_ATTRS = 'width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"';

export const ICONS = {
  share: `<svg ${SVG_ATTRS}><circle cx="12" cy="3.5" r="1.75"/><circle cx="4" cy="8" r="1.75"/><circle cx="12" cy="12.5" r="1.75"/><path d="M5.5 7.1l5-2.7M5.5 8.9l5 2.7"/></svg>`,
  ban: `<svg ${SVG_ATTRS}><circle cx="8" cy="8" r="6"/><path d="M3.8 3.8l8.4 8.4"/></svg>`,
};
