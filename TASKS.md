# Tasks → приватный трекер

Задачи и backlog переехали в **приватный** репо GitHub Issues:
**[`sultanjakhan/hanni-tasks`](https://github.com/sultanjakhan/hanni-tasks/issues)**
(репо `hanni` публичный — backlog держим отдельно и приватно).

## Из терминала

```sh
# добавить
gh issue create -R sultanjakhan/hanni-tasks -t "..." -l work

# список / фильтр
gh issue list   -R sultanjakhan/hanni-tasks
gh issue list   -R sultanjakhan/hanni-tasks -l health --json number,title,state

# закрыть
gh issue close 12 -R sultanjakhan/hanni-tasks
```

Короткая обёртка в `~/.zshrc`:
```sh
ht(){ gh issue "$@" -R sultanjakhan/hanni-tasks; }   # ht create -t "..."  |  ht list  |  ht close 12
```

Лейблы: `work` · `focus` · `health` · `projects` · `ui` · `infra` · `viz` · `in-progress`
