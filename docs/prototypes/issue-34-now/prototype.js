(() => {
  'use strict';

  const syntheticTask = Object.freeze({
    id: 'synthetic-interview-questions',
    title: 'Подготовить вопросы к интервью',
    reason: 'Срок сегодня',
    durationMinutes: 25,
    source: 'План дня · синтетическая задача',
  });

  const stateCopy = Object.freeze({
    recommendation: {
      status: 'Рекомендация',
      title: syntheticTask.title,
      meta: 'Срок сегодня · около 25 мин',
      support: 'Одна задача, которую можно начать без перестройки плана.',
    },
    clarified: {
      status: 'Рекомендация',
      title: syntheticTask.title,
      meta: 'Срок сегодня · около 25 мин',
      support: 'Причина и первый шаг раскрыты ниже.',
    },
    dismissed: {
      status: 'Скрыто',
      title: 'Рекомендация скрыта до обновления.',
      meta: 'План не изменён.',
      support: 'В прототипе скрытие действует только до перезагрузки страницы.',
    },
    active: {
      status: 'В работе',
      title: syntheticTask.title,
      meta: '',
      support: 'Текущая задача остаётся здесь, пока ты её не приостановишь или не завершишь.',
    },
    paused: {
      status: 'Приостановлено',
      title: syntheticTask.title,
      meta: '',
      support: 'Можно продолжить с того же места. План не перестроен.',
    },
    finishPending: {
      status: 'Завершаем…',
      title: syntheticTask.title,
      meta: 'Сохраняем результат…',
      support: 'Повторное действие временно недоступно.',
    },
    empty: {
      status: 'Спокойный режим',
      title: 'На сейчас нет подходящей задачи.',
      meta: 'План не изменён.',
      support: 'Hanni не подставляет случайную задачу, если уверенной рекомендации нет.',
    },
    error: {
      status: 'Требуется действие',
      title: 'Не удалось обновить «Сейчас».',
      meta: 'Попробуй ещё раз.',
      support: 'Предыдущий контекст сохранён.',
    },
  });

  const byId = (id) => document.getElementById(id);
  const elements = {
    card: byId('now-card'),
    content: byId('now-content'),
    loading: byId('loading-state'),
    title: byId('now-title'),
    meta: byId('now-meta'),
    support: byId('now-support'),
    status: byId('now-status-label'),
    live: byId('now-live'),
    clarification: byId('clarification-panel'),
    clarify: byId('clarify-action'),
    closeClarification: byId('close-clarification'),
    collapseClarification: byId('collapse-clarification'),
    error: byId('error-callout'),
    errorContext: byId('error-context'),
    start: byId('start-action'),
    pause: byId('pause-action'),
    finish: byId('finish-action'),
    dismiss: byId('dismiss-action'),
    restore: byId('restore-action'),
    retry: byId('retry-action'),
    scenario: byId('scenario-select'),
    failNext: byId('simulate-failure'),
    reset: byId('reset-prototype'),
    theme: byId('theme-toggle'),
    themeLabel: byId('theme-label'),
    previousDay: byId('previous-day'),
    nextDay: byId('next-day'),
    today: byId('today-button'),
  };

  const model = {
    state: 'recommendation',
    pending: false,
    pendingLabel: '',
    recoveryState: 'recommendation',
    recommendationMode: 'start',
    lastAction: 'load',
    elapsedSeconds: 12 * 60 + 48,
    actionTimer: null,
    elapsedTimer: null,
    syntheticDayOffset: 0,
  };

  const actionButtons = [
    elements.start,
    elements.pause,
    elements.finish,
    elements.dismiss,
    elements.clarify,
    elements.restore,
    elements.retry,
  ];

  function formatElapsed(totalSeconds) {
    const safeSeconds = Math.max(0, Number(totalSeconds) || 0);
    const minutes = Math.floor(safeSeconds / 60);
    const seconds = safeSeconds % 60;
    return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')} прошло`;
  }

  function announce(message) {
    elements.live.textContent = '';
    window.setTimeout(() => {
      elements.live.textContent = message;
    }, 0);
  }

  function setVisible(element, visible) {
    element.hidden = !visible;
  }

  function stopActionTimer() {
    if (model.actionTimer !== null) {
      window.clearTimeout(model.actionTimer);
      model.actionTimer = null;
    }
  }

  function stopElapsedTimer() {
    if (model.elapsedTimer !== null) {
      window.clearInterval(model.elapsedTimer);
      model.elapsedTimer = null;
    }
  }

  function startElapsedTimer() {
    stopElapsedTimer();
    if (model.state !== 'active' || model.pending) return;
    model.elapsedTimer = window.setInterval(() => {
      model.elapsedSeconds += 1;
      elements.meta.textContent = formatElapsed(model.elapsedSeconds);
    }, 1000);
  }

  function actionVisibility(state) {
    return {
      start: state === 'recommendation' || state === 'clarified' || state === 'paused',
      pause: state === 'active',
      finish: state === 'active' || state === 'finishPending',
      dismiss: state === 'recommendation' || state === 'clarified' || state === 'paused',
      clarify: state === 'recommendation' || state === 'clarified' || state === 'paused',
      restore: state === 'dismissed',
      retry: state === 'error',
    };
  }

  function render(options = {}) {
    const state = model.state;
    const isLoading = state === 'loading';
    const copy = stateCopy[state] || stateCopy.recommendation;
    const visible = actionVisibility(state);

    elements.card.dataset.state = state;
    elements.card.dataset.mode = model.recommendationMode;
    elements.card.setAttribute('aria-busy', String(isLoading || model.pending || state === 'finishPending'));
    setVisible(elements.loading, isLoading);
    setVisible(elements.content, !isLoading);

    if (!isLoading) {
      elements.status.textContent = model.pending && model.pendingLabel ? model.pendingLabel : copy.status;
      elements.title.textContent = copy.title;
      elements.meta.textContent = copy.meta;
      elements.support.textContent = copy.support;

      if (state === 'active' || state === 'paused') {
        elements.meta.textContent = formatElapsed(model.elapsedSeconds);
      }

      const resumeMode = model.recommendationMode === 'resume'
        && (state === 'paused' || state === 'clarified');
      elements.start.textContent = resumeMode ? 'Продолжить' : 'Начать';
      elements.finish.textContent = state === 'finishPending' ? 'Завершаем…' : 'Завершить';

      if (resumeMode && state === 'clarified') {
        elements.status.textContent = 'Приостановлено';
        elements.meta.textContent = formatElapsed(model.elapsedSeconds);
        elements.support.textContent = 'Причина раскрыта ниже. Можно продолжить с того же места.';
      }

      Object.entries(visible).forEach(([name, shouldShow]) => {
        setVisible(elements[name], shouldShow);
      });

      const clarified = state === 'clarified';
      elements.clarify.setAttribute('aria-expanded', String(clarified));
      setVisible(elements.clarification, clarified);
      setVisible(elements.error, state === 'error');

      if (state === 'error') {
        const operation = {
          load: 'Загрузка не завершилась.',
          start: 'Задача не была запущена.',
          resume: 'Задача осталась приостановленной.',
          pause: 'Активная задача не была изменена.',
          finish: 'Задача осталась в работе.',
        }[model.lastAction] || 'Задача осталась на месте.';
        elements.errorContext.textContent = `${operation} Контекст «${syntheticTask.title}» сохранён.`;
      }
    }

    actionButtons.forEach((button) => {
      button.disabled = model.pending || state === 'finishPending';
    });

    if (elements.scenario.querySelector(`option[value="${state}"]`)) {
      elements.scenario.value = state;
    }

    startElapsedTimer();

    if (options.focus instanceof HTMLElement && !options.focus.hidden) {
      options.focus.focus();
    }
  }

  function setScenario(state, options = {}) {
    stopActionTimer();
    model.pending = false;
    model.pendingLabel = '';
    model.state = state;

    if (state === 'error') {
      model.lastAction = 'load';
      model.recoveryState = 'recommendation';
      model.recommendationMode = 'start';
    } else if (state === 'paused') {
      model.recoveryState = 'paused';
      model.recommendationMode = 'resume';
    } else if (state === 'recommendation' || state === 'clarified' || state === 'dismissed') {
      model.recoveryState = state === 'clarified' || state === 'dismissed' ? 'recommendation' : state;
      model.recommendationMode = 'start';
    } else if (state !== 'finishPending') {
      model.recoveryState = state;
    }

    render(options);
    if (options.announce) announce(options.announce);
  }

  function shouldFailAction() {
    if (!elements.failNext.checked) return false;
    elements.failNext.checked = false;
    return true;
  }

  function enterError(operation, recoveryState) {
    model.pending = false;
    model.pendingLabel = '';
    model.state = 'error';
    model.lastAction = operation;
    model.recoveryState = recoveryState;
    render({ focus: elements.retry });
    announce('Не удалось обновить «Сейчас». Попробуй ещё раз.');
  }

  function completeAfterDelay(operation, recoveryState, nextState, announcement, options = {}) {
    if (model.pending) return;

    const fail = shouldFailAction();
    model.pending = true;
    model.pendingLabel = options.pendingLabel || '';
    model.lastAction = operation;
    model.recoveryState = recoveryState;

    if (operation === 'finish') model.state = 'finishPending';
    render();
    announce(options.pendingAnnouncement || 'Обновляем состояние задачи…');

    model.actionTimer = window.setTimeout(() => {
      model.actionTimer = null;
      if (fail) {
        enterError(operation, recoveryState);
        return;
      }

      model.pending = false;
      model.pendingLabel = '';
      model.state = nextState;
      if (nextState === 'paused') model.recommendationMode = 'resume';
      if (nextState === 'empty') model.recommendationMode = 'start';
      render(options.focusAfter ? { focus: options.focusAfter } : {});
      announce(announcement);
    }, options.delayMs || 620);
  }

  function runAction(operation, fromRetry = false) {
    if (model.pending) return;

    const state = model.state;
    if (operation === 'start' || operation === 'resume') {
      const recoveryState = operation === 'resume' ? 'paused' : 'recommendation';
      completeAfterDelay(
        operation,
        recoveryState,
        'active',
        operation === 'resume' ? 'Задача продолжена.' : 'Задача начата.',
        {
          pendingLabel: operation === 'resume' ? 'Продолжаем…' : 'Начинаем…',
          pendingAnnouncement: operation === 'resume' ? 'Продолжаем задачу…' : 'Запускаем задачу…',
          focusAfter: elements.pause,
        },
      );
      return;
    }

    if (operation === 'pause' && (state === 'active' || fromRetry)) {
      completeAfterDelay('pause', 'active', 'paused', 'Задача приостановлена.', {
        pendingLabel: 'Приостанавливаем…',
        pendingAnnouncement: 'Приостанавливаем задачу…',
        focusAfter: elements.start,
      });
      return;
    }

    if (operation === 'finish' && (state === 'active' || fromRetry)) {
      completeAfterDelay('finish', 'active', 'empty', 'Задача завершена. На сейчас нет подходящей задачи.', {
        pendingAnnouncement: 'Завершаем задачу…',
        delayMs: 840,
        focusAfter: elements.card,
      });
    }
  }

  function retryLastAction() {
    if (model.pending || model.state !== 'error') return;
    const operation = model.lastAction;
    const recoveryState = model.recoveryState;

    if (operation === 'load') {
      setScenario('recommendation', { announce: 'Рекомендация загружена.', focus: elements.start });
      return;
    }

    model.state = recoveryState;
    model.pending = false;
    model.pendingLabel = '';
    render();
    runAction(operation, true);
  }

  function openClarification() {
    if (model.pending) return;
    if (model.state === 'clarified') {
      closeClarification();
      return;
    }
    if (model.state !== 'recommendation' && model.state !== 'paused') return;
    model.recoveryState = model.state;
    model.state = 'clarified';
    render({ focus: elements.clarification });
    announce('Пояснение рекомендации раскрыто.');
  }

  function closeClarification() {
    if (model.state !== 'clarified') return;
    model.state = model.recoveryState === 'paused' ? 'paused' : 'recommendation';
    render({ focus: elements.clarify });
    announce('Пояснение рекомендации свёрнуто.');
  }

  function dismissRecommendation() {
    if (model.pending) return;
    const returnState = model.state === 'clarified' ? model.recoveryState : model.state;
    model.state = 'dismissed';
    model.recoveryState = returnState === 'paused' ? 'paused' : 'recommendation';
    render({ focus: elements.restore });
    announce('Рекомендация скрыта до обновления. План не изменён.');
  }

  function restoreRecommendation() {
    model.state = model.recoveryState === 'paused' ? 'paused' : 'recommendation';
    render({ focus: elements.start });
    announce('Рекомендация возвращена.');
  }

  function resetPrototype() {
    stopActionTimer();
    elements.failNext.checked = false;
    model.state = 'recommendation';
    model.pending = false;
    model.pendingLabel = '';
    model.recoveryState = 'recommendation';
    model.recommendationMode = 'start';
    model.lastAction = 'load';
    model.elapsedSeconds = 12 * 60 + 48;
    model.syntheticDayOffset = 0;
    elements.today.textContent = 'Сегодня, 2 сентября';
    render({ focus: elements.start });
    announce('Прототип сброшен к рекомендации.');
  }

  function shiftSyntheticDay(delta) {
    const labels = ['Вчера, 1 сентября', 'Сегодня, 2 сентября', 'Завтра, 3 сентября'];
    model.syntheticDayOffset = Math.max(-1, Math.min(1, model.syntheticDayOffset + delta));
    elements.today.textContent = labels[model.syntheticDayOffset + 1];
    announce(`Показана синтетическая дата: ${elements.today.textContent}.`);
  }

  elements.start.addEventListener('click', () => {
    runAction(model.recommendationMode === 'resume' ? 'resume' : 'start');
  });
  elements.pause.addEventListener('click', () => runAction('pause'));
  elements.finish.addEventListener('click', () => runAction('finish'));
  elements.dismiss.addEventListener('click', dismissRecommendation);
  elements.restore.addEventListener('click', restoreRecommendation);
  elements.retry.addEventListener('click', retryLastAction);
  elements.clarify.addEventListener('click', openClarification);
  elements.closeClarification.addEventListener('click', closeClarification);
  elements.collapseClarification.addEventListener('click', closeClarification);
  elements.reset.addEventListener('click', resetPrototype);

  elements.scenario.addEventListener('change', () => {
    const nextState = elements.scenario.value;
    const nextFocus = nextState === 'clarified' ? elements.clarification : undefined;
    setScenario(nextState, {
      announce: `Сценарий прототипа: ${elements.scenario.selectedOptions[0].textContent}.`,
      focus: nextFocus,
    });
  });

  elements.theme.addEventListener('click', () => {
    const nextTheme = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = nextTheme;
    const isDark = nextTheme === 'dark';
    elements.theme.setAttribute('aria-pressed', String(isDark));
    elements.theme.setAttribute('aria-label', isDark ? 'Включить светлую тему' : 'Включить тёмную тему');
    elements.themeLabel.textContent = isDark ? 'Светлая тема' : 'Тёмная тема';
    announce(isDark ? 'Включена тёмная тема.' : 'Включена светлая тема.');
  });

  elements.previousDay.addEventListener('click', () => shiftSyntheticDay(-1));
  elements.nextDay.addEventListener('click', () => shiftSyntheticDay(1));
  elements.today.addEventListener('click', () => {
    model.syntheticDayOffset = 0;
    elements.today.textContent = 'Сегодня, 2 сентября';
    announce('Показана синтетическая дата: сегодня, 2 сентября.');
  });

  document.querySelectorAll('.pane-tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      document.querySelectorAll('.pane-tab').forEach((candidate) => {
        const selected = candidate === tab;
        candidate.classList.toggle('pane-tab--active', selected);
        if (selected) candidate.setAttribute('aria-current', 'page');
        else candidate.removeAttribute('aria-current');
      });
      announce(`Открыт демонстрационный раздел «${tab.textContent.trim()}».`);
    });
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && model.state === 'clarified') {
      event.preventDefault();
      closeClarification();
    }
  });

  render();
})();
