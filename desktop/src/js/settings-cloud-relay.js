// Credentials are write-only local IPC input. Do not persist them in app_settings,
// browser storage, diagnostics, URLs or remotely generated QR codes.
export function renderCloudRelaySection({ mobile = false } = {}) {
  return `<section class="settings-section" data-cloud-relay>
    <div class="settings-section-title">Синхронизация устройств</div>
    <p class="settings-hint">После первоначального подключения Hanni обменивается данными о здоровье автоматически, когда устройство доступно. Содержимое зашифровано между вашими устройствами.</p>
    <p class="settings-hint" data-relay-status role="status" aria-live="polite">Проверяем состояние…</p>
    <details data-relay-setup open>
      <summary>Подключение этого устройства</summary>
      <form data-relay-form autocomplete="off">
        <p class="settings-hint">Введите подготовленный для этого устройства код подключения. Он уже должен содержать выбранный основной телефон для сна.</p>
        <label class="settings-label" for="cloud-relay-setup-code">Код подключения</label>
        <input id="cloud-relay-setup-code" class="form-input" type="password" required maxlength="4096"
          autocomplete="off" autocapitalize="off" spellcheck="false" data-relay-code
          style="width:100%;box-sizing:border-box;">
        <button class="btn-primary" type="submit" data-relay-save>Подключить</button>
      </form>
    </details>
    <div class="settings-row" style="gap:var(--space-2);flex-wrap:wrap;">
      <button class="btn-smallall" type="button" data-relay-refresh>Обновить состояние</button>
      <button class="btn-smallall" type="button" data-relay-now disabled>Проверить обмен</button>
    </div>
    <p class="settings-hint" data-relay-feedback role="status" aria-live="polite"></p>
    <details><summary>Диагностика обмена</summary>
      <div class="settings-hint" data-relay-diagnostics>Состояние пока недоступно.</div>
    </details>
    ${mobile ? `<details><summary>Первоначальная настройка основного телефона</summary>
      <p class="settings-hint">На выбранном основном телефоне покажите код источника, чтобы включить его в подключение всех устройств. Эта кнопка не читает данные о здоровье.</p>
      <button class="btn-smallall" type="button" data-relay-source>Показать код источника</button>
      <label class="settings-hint" data-relay-source-result hidden>Код источника этого телефона
        <input class="form-input" data-relay-source-code readonly autocomplete="off" spellcheck="false" style="width:100%;box-sizing:border-box;">
      </label>
    </details>` : ''}
  </section>`;
}

function statusText(status) {
  if (status?.isolated) return 'Подключение недоступно в изолированной сборке.';
  if (status?.status === 'configuration_unavailable') return 'Защищённая конфигурация временно недоступна. Подключение сохранено.';
  if (status?.configured === false) return 'Это устройство ещё не подключено.';
  if (status?.configured !== true) return 'Состояние обмена пока недоступно.';
  if (status.enabled === false) return 'Подключено. Автоматический обмен выключен.';
  if (status.error_code) return 'Подключено. Обмен не завершён; Hanni повторит попытку автоматически.';
  if (status.initializing) return 'Подключено. Подготовка первого обмена.';
  const waiting = ['pending_keys', 'incomplete_parts', 'unresolved_deletions']
    .some(key => typeof status[key] === 'number' && status[key] > 0);
  if (waiting) return 'Подключено. Есть данные, ожидающие передачи или применения.';
  if (status.last_ok) return 'Подключено. Последний обмен с сервером завершён. Доставка на остальные устройства проверяется отдельно.';
  return 'Подключено. Ожидаем первый успешный обмен.';
}

const sourceTypes = new Set(`ActiveCaloriesBurned BasalBodyTemperature BasalMetabolicRate BloodGlucose
  BloodPressure BodyFat BodyTemperature BodyWaterMass BoneMass CervicalMucus CyclingPedalingCadence
  Distance ElevationGained ExerciseSession FloorsClimbed HeartRate HeartRateVariabilityRmssd Height
  Hydration IntermenstrualBleeding LeanBodyMass MenstruationFlow MenstruationPeriod MindfulnessSession
  Nutrition OvulationTest OxygenSaturation PlannedExerciseSession Power RespiratoryRate RestingHeartRate
  SexualActivity SkinTemperature SleepSession Speed StepsCadence Steps TotalCaloriesBurned Vo2Max Weight
  WheelchairPushes`.split(/\s+/).filter(Boolean).map(value => value + 'Record'));
const sourceLabels = {
  not_started: 'ещё не читали', snapshot_pending: 'читается история', replay_pending: 'проверяются изменения',
  changes_pending: 'есть следующие страницы', caught_up: 'доступные изменения прочитаны',
  caught_up_with_deletion_gap: 'прочитано; есть пробел в истории удалений',
  token_expired_rescan_pending: 'нужна повторная проверка истории', source_timeout: 'источник не ответил вовремя',
  permission_required: 'нужно разрешение', background_permission_required: 'нужно разрешение фонового чтения',
  feature_unavailable: 'тип недоступен на этом телефоне', feature_probe_failed: 'проверка доступности не завершена',
  background_feature_probe_failed: 'проверка фонового доступа не завершена',
};
const projectionLabels = {
  authority_not_configured: 'основной телефон ещё не выбран', projection_not_initialized: 'ожидает подготовки',
  projection_partial: 'часть записей требует повторной обработки', projection_pending: 'есть необработанные записи',
  projected: 'доступные записи обработаны', projection_deferred: 'обработка отложена',
};
function count(value) { return Number.isSafeInteger(value) && value >= 0 ? value : '—'; }
function time(value, epoch = false) {
  if (epoch ? !Number.isSafeInteger(value) || value < 0 : typeof value !== 'string' || !/^\d{4}-\d\d-\d\d[T ][\d:.+-]+Z?$/.test(value)) return '—';
  // SQLite CURRENT_TIMESTAMP is UTC without an explicit suffix.
  const normalized = epoch ? value * 1000 : value.replace(' ', 'T') + (/(Z|[+-]\d\d:\d\d)$/.test(value) ? '' : 'Z');
  const date = new Date(normalized);
  return Number.isNaN(date.getTime()) ? '—' : date.toLocaleString('ru-RU');
}
function renderDiagnostics(target, status) {
  target.replaceChildren();
  const line = text => { const paragraph = target.ownerDocument.createElement('p'); paragraph.textContent = text; target.append(paragraph); };
  line(`Ожидают отправки: ${count(status?.pending_keys)}. Неполные части: ${count(status?.incomplete_parts)}. Неразрешённые удаления: ${count(status?.unresolved_deletions)}.`);
  line(`Получено подтверждений устройств: ${Array.isArray(status?.device_receipts) ? status.device_receipts.length : 0}. Последний обмен: ${time(status?.last_ok)}.`);
  for (const [index, receipt] of (Array.isArray(status?.device_receipts) ? status.device_receipts : []).slice(0, 16).entries()) {
    // The local cursor also includes receipt-only packets. Comparing a peer ACK
    // with it would falsely report a lag after all health changes were delivered.
    line(`Устройство ${index + 1}: подтверждено сохранение пакетов до ${count(receipt?.applied_seq)}. Получено: ${time(receipt?.received_at)}.`);
  }
  if (status?.projection && typeof status.projection === 'object') {
    const p = status.projection;
    line(`Сон в календаре: ${projectionLabels[p.status] || 'состояние пока недоступно'}. Ожидают обработки: ${count(p.pending_records)}. Ошибок: ${count(p.errors)}.`);
    line(`Последняя обработка: ${time(p.last_projected_epoch, true)}.${p.retry_needed === true ? ` Повтор: ${time(p.next_retry_epoch, true)}.` : ''}`);
  }
  const freshnessNames = { sleep_sessions: 'Сон', sleep_stages: 'Фазы сна', health_log: 'Активность', heart_rate_samples: 'Пульс', health_records: 'Архив Health Connect', 'health_log:steps': 'Шаги', 'health_log:exercise': 'Тренировки' };
  for (const item of (Array.isArray(status?.freshness) ? status.freshness : []).slice(0, 64)) {
    const rawType = typeof item?.type === 'string' && item.type.startsWith('raw:') ? item.type.slice(4) : '';
    const name = sourceTypes.has(rawType) ? `Health Connect — ${rawType}`
      : Object.hasOwn(freshnessNames, item?.type) ? freshnessNames[item.type] : null;
    if (!name) continue;
    line(`${name}: изменение ${time(item.record_updated_at)}, получено ${time(item.received_at)}.`);
  }
  const imports = Array.isArray(status?.source_import) ? status.source_import.slice(0, 41) : [];
  if (!imports.length) line('Чтение Health Connect на этом устройстве: состояние пока отсутствует.');
  for (const item of imports) {
    if (!sourceTypes.has(item?.type)) continue;
    const phase = { idle: 'ожидание', snapshot: 'история', replay: 'проверка изменений', changes: 'новые изменения' }[item.phase] || '—';
    const coverage = { not_started: 'ещё не проверена', history_permission_scan: 'читается по разрешению истории', limited_unknown_grant_start: 'ограничена разрешением; начало неизвестно' }[item.history_coverage] || 'не подтверждена';
    line(`${item.type}: ${sourceLabels[item.status] || 'чтение не завершено'}. Этап: ${phase}. История: ${coverage}.${item.more_pending === true ? ' Есть следующие страницы.' : ''}${item.deletion_gap === true ? ' Есть пробел в истории удалений.' : ''} Попытка: ${time(item.last_attempt_at)}. Успешно: ${time(item.last_success_at)}.`);
  }
}

export function wireCloudRelayControls(element, invoke) {
  const root = element.querySelector('[data-cloud-relay]');
  if (!root || root.dataset.wired) return;
  root.dataset.wired = 'true';
  const code = root.querySelector('[data-relay-code]');
  const save = root.querySelector('[data-relay-save]');
  const feedback = root.querySelector('[data-relay-feedback]');
  const now = root.querySelector('[data-relay-now]');
  let submitting = false;
  let refreshVersion = 0;

  async function refresh() {
    const version = ++refreshVersion;
    try {
      const status = await invoke('cloud_relay_status');
      if (version !== refreshVersion) return;
      root.querySelector('[data-relay-status]').textContent = statusText(status);
      renderDiagnostics(root.querySelector('[data-relay-diagnostics]'), status);
      now.disabled = status?.configured !== true || status.enabled === false || Boolean(status.isolated);
      if (status?.configured === true) root.querySelector('[data-relay-setup]').open = false;
    } catch (_) {
      if (version !== refreshVersion) return;
      root.querySelector('[data-relay-status]').textContent = 'Не удалось проверить состояние обмена.';
      root.querySelector('[data-relay-diagnostics]').textContent = 'Состояние пока недоступно.';
      now.disabled = true;
    }
  }

  root.querySelector('[data-relay-form]').addEventListener('submit', async event => {
    event.preventDefault();
    if (submitting) return;
    let config = code.value.trim();
    code.value = '';
    feedback.textContent = '';
    // The native parser remains authoritative. This one precondition prevents
    // first pairing without the immutable sleep authority selected by the user.
    try {
      if (!config || new TextEncoder().encode(config).length > 4096) throw new Error();
      const parsed = JSON.parse(config);
      if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(parsed?.sleep_source_store_id || '')) throw new Error();
    } catch (_) {
      config = '';
      feedback.textContent = 'Код подключения не принят. Нужен полный код для этого устройства с выбранным основным телефоном.';
      return;
    }
    submitting = true;
    save.disabled = true;
    try {
      const result = await invoke('cloud_relay_set_config', { config });
      if (result?.configured !== true) throw new Error();
      feedback.textContent = 'Подключение сохранено. Состояние обмена показано выше.';
      await refresh();
    } catch (_) {
      // Native errors must never be reflected: a future error may contain input.
      feedback.textContent = 'Не удалось сохранить подключение. Проверьте код именно этого устройства. Для смены основного телефона требуется перенос настройки.';
    } finally {
      config = '';
      submitting = false;
      save.disabled = false;
    }
  });

  root.querySelector('[data-relay-refresh]').addEventListener('click', refresh);
  now.addEventListener('click', async () => {
    now.disabled = true;
    try {
      const result = await invoke('cloud_relay_sync_now');
      feedback.textContent = result?.enqueued === true
        ? 'Проверка обмена поставлена в очередь.' : 'Проверку пока не удалось запустить.';
    } catch (_) { feedback.textContent = 'Проверку пока не удалось запустить.'; }
    await refresh();
  });

  root.querySelector('[data-relay-source]')?.addEventListener('click', async event => {
    const button = event.currentTarget;
    button.disabled = true;
    try {
      const result = await invoke('cloud_relay_pairing_source');
      if (result?.supported !== true || !/^[0-9a-f-]{36}$/.test(result.source_store_id || '')) throw new Error();
      root.querySelector('[data-relay-source-code]').value = result.source_store_id;
      root.querySelector('[data-relay-source-result]').hidden = false;
    } catch (_) { feedback.textContent = 'Код источника сейчас недоступен. Данные и настройки не заменены.'; }
    finally { button.disabled = false; }
  });
  // Clearing the DOM input limits accidental retention. JavaScript strings and
  // OS clipboard history cannot be reliably wiped by a web settings page.
  const doc = root.ownerDocument;
  const hide = () => { if (doc.hidden) code.value = ''; };
  doc.addEventListener('visibilitychange', hide);
  const observer = new doc.defaultView.MutationObserver(() => {
    if (!root.isConnected) {
      code.value = '';
      doc.removeEventListener('visibilitychange', hide);
      observer.disconnect();
    }
  });
  observer.observe(doc.documentElement, { childList: true, subtree: true });
  return refresh();
}
