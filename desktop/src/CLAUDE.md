> Дополняет корневой ~/hanni/CLAUDE.md — здесь только специфика фронтенда (JS / CSS / HTML).

# src — Frontend (JS ES-модули, CSS, HTML)

## Style
- **CSS**: всегда переменные из `base.css` — никогда не хардкодить цвета, отступы, радиусы, тени
- **JS state**: только персистентные данные в объекте `S` — никаких temp-переменных
- **Settings UI**: layout `settings-row` / `settings-label` / toggle — никаких сырых `<select>`

## Pre-commit
- [ ] `node --check` на каждом изменённом JS-файле — ловит SyntaxError до рантайма
- [ ] Нет хардкод-цветов/размеров (использовать CSS-переменные)
- [ ] Нет дублей `const`/`let` в одном scope
