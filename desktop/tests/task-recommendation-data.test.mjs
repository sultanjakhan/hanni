import assert from 'node:assert/strict';
import test from 'node:test';

const storage = new Map();
const fixtures = new Map();

globalThis.localStorage = {
  getItem: key => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, String(value)),
};
Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: { userAgent: 'node-test' },
});
globalThis.document = {
  documentElement: {
    classList: { add() {} },
    setAttribute() {},
  },
  getElementById() { return null; },
};
globalThis.window = {
  innerWidth: 1280,
  __TAURI__: {
    core: { invoke: async command => fixtures.get(command) ?? [] },
    event: { listen: async () => {}, emit: async () => {} },
  },
};

const { loadActiveTaskBlock, loadTaskRecommendationData } =
  await import('../src/js/task-recommendation-data.js');

function resetFixtures() {
  fixtures.clear();
  fixtures.set('get_today_planned', []);
  fixtures.set('get_app_setting', null);
  fixtures.set('get_task_pins', []);
  fixtures.set('get_task_avg_durations', []);
  fixtures.set('get_routine_chains', []);
  fixtures.set('get_routine_now', []);
  fixtures.set('get_completed_routine_chains', []);
  fixtures.set('get_schedules', []);
  fixtures.set('get_timeline_blocks', []);
}

test('active routine step outranks a regular task', async () => {
  resetFixtures();
  fixtures.set('get_today_planned', [{
    source_type: 'note', source_id: 4, title: 'Обычная задача', priority: 5,
  }]);
  fixtures.set('get_routine_now', [{
    run_id: 12,
    chain_id: 2,
    tasks: [{
      id: 9, title: 'Следующий шаг рутины', requirement: 'required', priority: 1,
      source_type: 'schedule', source_id: 7, tracking_mode: 'track',
    }],
  }]);

  const data = await loadTaskRecommendationData();
  assert.equal(data.recommendations[0].kind, 'routine-task');
  assert.equal(data.recommendations[0].title, 'Следующий шаг рутины');
  assert.equal(data.recommendations[1].title, 'Обычная задача');
});

test('pinned task stays in picker but does not become the Now fallback', async () => {
  resetFixtures();
  fixtures.set('get_today_planned', [
    { source_type: 'event', source_id: 1, title: 'Закреплено', priority: 5 },
    { source_type: 'schedule', source_id: 2, title: 'Действие', priority: 1 },
  ]);
  fixtures.set('get_task_pins', ['event:1']);

  const data = await loadTaskRecommendationData();
  assert.equal(data.startable.length, 2);
  assert.equal(data.recommendations[0].title, 'Действие');
});

test('multi-slot chain does not suppress a regular recommendation', async () => {
  resetFixtures();
  fixtures.set('get_today_planned', [{
    source_type: 'note', source_id: 5, title: 'Написать заметку', priority: 1,
  }]);
  fixtures.set('get_routine_chains', [{
    id: 3, title: 'Еда', trigger_type: 'time', trigger_time: '00:00,00:01',
    nodes: [], edges: [],
  }]);

  const data = await loadTaskRecommendationData();
  assert.equal(data.chainRecId, null);
  assert.equal(data.recommendations[0].title, 'Написать заметку');
});

test('active timeline block is returned as the current state', async () => {
  resetFixtures();
  fixtures.set('get_timeline_blocks', [
    { id: 1, notes: 'Старый блок', is_active: false },
    { id: 2, notes: 'Текущая задача', is_active: true },
  ]);

  const active = await loadActiveTaskBlock();
  assert.equal(active.id, 2);
  assert.equal(active.notes, 'Текущая задача');
});
