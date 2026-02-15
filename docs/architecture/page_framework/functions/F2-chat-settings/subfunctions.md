# F2 — Chat settings (proactive, TTS): Подфункции

> Декомпозиция бизнес-функции на атомарные шаги.
> Phase 3 drill-down.

## Обзор

| Метрика | Значение |
|---------|----------|
| Бизнес-функция | F2: Chat settings (proactive, TTS) |
| Файл(ы) | main.js:L714-963 |
| LOC | 250 |
| Подфункций | 9 |
| Сложность (max) | High |

## Подфункции

### Frontend

| # | Подфункция | Строки | LOC | Trigger → Effect | Тип | Сложность |
|---|-----------|--------|-----|-----------------|-----|-----------|
| F2.1 | Загрузка настроек (async) | L714-735 | 22 | loadChatSettings() → Promise.all: proactive settings, TTS voices, server URL | bridge | Low |
| F2.2 | Рендер панели «Основные» | L737-812 | 76 | proactive data + voices → DOM: toggle enabled, slider interval, quiet hours, voice select, TTS server URL | render | High |
| F2.3 | Рендер панели «Стили сообщений» | L813-840 | 28 | enabledStyles + PROACTIVE_STYLE_DEFINITIONS → DOM: стили-карточки с toggle | render | Low |
| F2.4 | Переключение вкладок настроек | L842-850 | 9 | click .chat-settings-tab → toggle active class на tab и panel | interaction | Trivial |
| F2.5 | Утилиты сбора значений + автосохранение | L852-893 | 42 | getEnabledStyles(), getChatProactiveValues(), saveChatSettings() → invoke('set_proactive_settings') | bridge | Medium |
| F2.6 | Обработчики стилей (toggle, select all/none) | L895-920 | 26 | click на карточку/checkbox/select all/none → toggle .enabled + saveChatSettings() | interaction | Low |
| F2.7 | Тест голоса TTS | L922-931 | 10 | click #chat-test-voice → invoke('speak_text') с тестовой фразой | bridge | Trivial |
| F2.8 | Сохранение TTS-сервера | L933-946 | 14 | click #chat-tts-server-save → invoke('set_app_setting') + fetch /health | bridge | Trivial |
| F2.9 | Авто-проверка TTS-сервера | L948-959 | 12 | ttsServerUrl существует → fetch /health → отображение model/gpu | external | Trivial |

## Data Flow

```
[Input: loadChatSettings()]
    │
    ▼
┌─ F2.1 Загрузка данных ──────────┐
│  Promise.all([                     │
│    invoke('get_proactive_settings') │ ──→ [Backend: proactive settings]
│    invoke('get_tts_voices')         │ ──→ [Backend: edge-tts voices]
│    invoke('get_app_setting',        │
│      {key:'tts_server_url'})        │ ──→ [Backend: app_settings DB]
│  ])                                 │
└──────┬──────────────────────────────┘
       │ proactive, ttsVoices, ttsServerUrl
       ▼
┌─ F2.2 Рендер «Основные» ────────┐
│  innerHTML: toggle, slider,        │
│  quiet hours, voice select,        │ ──→ [DOM: #cs-panel-general]
│  TTS server section                │
└──────┬──────────────────────────────┘
       │
       ▼
┌─ F2.3 Рендер «Стили» ───────────┐
│  карточки стилей с toggle          │ ──→ [DOM: #cs-panel-styles]
└──────┬──────────────────────────────┘
       │
       ▼
┌─ F2.4 Вкладки ──────────────────┐
│  click → toggle .active            │ ──→ [DOM: .chat-settings-tab/panel]
└──────────────────────────────────────┘

┌─ F2.5 Автосохранение ───────────┐
│  change на любом контроле →        │
│  getChatProactiveValues() →        │
│  invoke('set_proactive_settings')  │ ──→ [Backend: proactive_config DB]
└──────────────────────────────────────┘

┌─ F2.7 Тест голоса ──────────────┐
│  click → invoke('speak_text')      │ ──→ [Backend: edge-tts / TTS server]
└──────────────────────────────────────┘

┌─ F2.8 + F2.9 TTS сервер ────────┐
│  save URL → invoke('set_app_setting')│ ──→ [Backend: app_settings DB]
│  fetch /health                      │ ──→ [External: PC TTS server]
│  → display model/gpu/status         │
└──────────────────────────────────────┘
```

## Проблемные зоны

| # | Подфункция | Проблема | Рекомендация | Приоритет |
|---|-----------|---------|-------------|-----------|
| F2.2 | Рендер панели «Основные» | 76 LOC — одна огромная template literal, трудно поддерживать | Разбить на helper-функции: renderProactiveSection(), renderVoiceSection(), renderTTSServerSection() | Medium |
| F2.5 | Автосохранение | Много повторяющихся addEventListener('change', saveChatSettings) | Делегирование событий на общий контейнер | Low |
