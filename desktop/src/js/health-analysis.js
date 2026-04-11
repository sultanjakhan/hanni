// health-analysis.js — Sleep analysis card ("Why am I sleepy?")
import { invoke } from './state.js';

export async function renderSleepAnalysis(el) {
  let analysis;
  try {
    analysis = await invoke('get_sleep_analysis', { days: 30 });
  } catch {
    el.innerHTML = '';
    return;
  }

  if (!analysis || analysis.optimal_bedtime === '—') {
    el.innerHTML = '<div class="uni-empty">Недостаточно данных для анализа. Нужно минимум 7 дней данных сна.</div>';
    return;
  }

  const trendIcon = analysis.sleep_quality_trend === 'improving' ? '📈' : analysis.sleep_quality_trend === 'declining' ? '📉' : '➡️';
  const trendLabel = analysis.sleep_quality_trend === 'improving' ? 'Улучшается' : analysis.sleep_quality_trend === 'declining' ? 'Ухудшается' : 'Стабильно';
  const debtLabel = analysis.sleep_debt_hours > 0 ? `${analysis.sleep_debt_hours.toFixed(1)}ч` : 'Нет';
  const qualityColor = analysis.avg_sleep_quality >= 70 ? 'green' : analysis.avg_sleep_quality >= 40 ? 'yellow' : 'red';

  el.innerHTML = `
    <div style="font-size:13px;font-weight:600;margin:16px 0 8px;color:var(--text-primary);">Анализ сна</div>

    <div class="uni-dash-grid">
      <div class="uni-dash-card green">
        <div class="uni-dash-value">${analysis.optimal_bedtime}</div>
        <div class="uni-dash-label">🎯 Оптимальное засыпание</div>
      </div>
      <div class="uni-dash-card blue">
        <div class="uni-dash-value">${analysis.current_avg_bedtime}</div>
        <div class="uni-dash-label">🕐 Текущее среднее</div>
      </div>
    </div>

    <div class="uni-dash-grid" style="margin-top:var(--space-2);">
      <div class="uni-dash-card ${qualityColor}">
        <div class="uni-dash-value">${Math.round(analysis.avg_sleep_quality)}%</div>
        <div class="uni-dash-label">💤 Качество сна</div>
      </div>
      <div class="uni-dash-card gray">
        <div class="uni-dash-value">${trendIcon} ${trendLabel}</div>
        <div class="uni-dash-label">Тренд</div>
      </div>
    </div>

    ${analysis.recommendations.length ? `
    <div class="health-recs">
      <div class="health-recs-title">💡 Рекомендации</div>
      ${analysis.recommendations.map(r => `<div class="health-rec-item">${r}</div>`).join('')}
    </div>` : ''}
  `;
}
