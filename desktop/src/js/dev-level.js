// ── dev-level.js — score (0–10) level rendering helpers for the dev matrix ──
// Colour lives in CSS via [data-tier]; never inline styles.

export function scoreTier(v) {
  if (v == null || v <= 0) return 'none';
  if (v >= 7) return 'high';
  if (v >= 4) return 'mid';
  return 'low';
}

export function fmtScore(v) {
  return v == null ? '—' : String(Math.round(v * 10) / 10);
}

// Mini progress bar for a skill leaf (score 0–10).
export function levelBarHtml(v) {
  const pct = v == null ? 0 : Math.max(0, Math.min(100, v * 10));
  return `<div class="dev-lvl" data-tier="${scoreTier(v)}"><div class="dev-lvl-fill" style="width:${pct}%"></div></div>`;
}

// Aggregate badge for an area / competency (readable while collapsed).
export function levelBadgeHtml(v) {
  return `<span class="dev-lvl-badge" data-tier="${scoreTier(v)}">${fmtScore(v)}</span>`;
}
