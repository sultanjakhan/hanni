const MAX_STEP_MINUTES = 24 * 60;
const MAX_STEP_INGREDIENTS = 100;

export function normalizeStepMinutes(value) {
  const minutes = Number(value);
  if (!Number.isFinite(minutes)) return 0;
  return Math.min(MAX_STEP_MINUTES, Math.max(0, Math.trunc(minutes)));
}

export function normalizeRecipeStep(value) {
  const step = value && typeof value === 'object' ? value : {};
  const ingredients = Array.isArray(step.ingredients)
    ? step.ingredients
        .map(item => String(item ?? '').trim())
        .filter(Boolean)
        .slice(0, MAX_STEP_INGREDIENTS)
    : [];
  return {
    text: String(step.text ?? '').trim(),
    min: normalizeStepMinutes(step.min),
    ingredients,
  };
}

export function parseRecipeSteps(raw, legacy = true) {
  const source = String(raw ?? '').trim();
  if (source.startsWith('[')) {
    try {
      const parsed = JSON.parse(source);
      if (Array.isArray(parsed)) {
        return parsed.map(normalizeRecipeStep).filter(step => step.text);
      }
    } catch { /* fall through to escaped legacy text */ }
  }
  if (!legacy) return [];
  return source.split('\n').map(line => line.trim()).filter(Boolean)
    .map(line => ({ text: line.replace(/^\d+\.\s*/, ''), min: 0, ingredients: [] }));
}
