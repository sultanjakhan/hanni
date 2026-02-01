#!/usr/bin/env python3
"""
Sleep Mode - Режим сна для экономии батареи
Оставляет только LM Studio и Hanni бота

Использование:
  python sleep_mode.py on   - Включить режим сна
  python sleep_mode.py off  - Выключить (утром)
  python sleep_mode.py auto - Автоматически по расписанию
"""
import subprocess
import sys
import os
from datetime import datetime

# Приложения которые НЕ закрывать
KEEP_ALIVE = [
    "LM Studio",
    "Terminal",
    "iTerm2",
    "Activity Monitor",
]

# Приложения которые закрыть в режиме сна
CLOSE_APPS = [
    "Safari",
    "Google Chrome",
    "Firefox",
    "Telegram",
    "Discord",
    "Slack",
    "Spotify",
    "Music",
    "Mail",
    "Messages",
    "Notes",
    "Finder",  # Закрываем лишние окна
    "Preview",
    "TextEdit",
    "VS Code",
    "Visual Studio Code",
    "Cursor",
]


def get_running_apps():
    """Get list of running apps"""
    result = subprocess.run(
        ['osascript', '-e', 'tell application "System Events" to get name of every process whose background only is false'],
        capture_output=True, text=True
    )
    if result.returncode == 0:
        return [app.strip() for app in result.stdout.split(',')]
    return []


def close_app(app_name):
    """Close an application gracefully"""
    script = f'''
    tell application "{app_name}"
        quit
    end tell
    '''
    subprocess.run(['osascript', '-e', script], capture_output=True)


def enable_sleep_mode():
    """Enable sleep mode - close unnecessary apps"""
    print("🌙 Включаю режим сна...")
    print(f"   Время: {datetime.now().strftime('%H:%M')}")

    running = get_running_apps()
    closed = []

    for app in running:
        app_clean = app.strip()
        # Пропускаем системные и нужные приложения
        if app_clean in KEEP_ALIVE:
            print(f"   ✓ Оставляю: {app_clean}")
            continue
        if app_clean.startswith("LM"):
            print(f"   ✓ Оставляю: {app_clean}")
            continue

        # Закрываем если в списке
        if app_clean in CLOSE_APPS:
            close_app(app_clean)
            closed.append(app_clean)
            print(f"   ✗ Закрыл: {app_clean}")

    # Уменьшаем яркость экрана
    subprocess.run(['brightness', '0.1'], capture_output=True)

    # Отключаем Bluetooth (опционально)
    # subprocess.run(['blueutil', '--power', '0'], capture_output=True)

    print(f"\n✅ Режим сна включён")
    print(f"   Закрыто приложений: {len(closed)}")
    print(f"   LM Studio и Hanni продолжают работать")

    # Показываем уведомление
    subprocess.run([
        'osascript', '-e',
        'display notification "Спокойной ночи! LM Studio и Hanni работают." with title "Sleep Mode"'
    ])


def disable_sleep_mode():
    """Disable sleep mode - restore brightness"""
    print("☀️ Выключаю режим сна...")

    # Восстанавливаем яркость
    subprocess.run(['brightness', '0.7'], capture_output=True)

    # Включаем Bluetooth
    # subprocess.run(['blueutil', '--power', '1'], capture_output=True)

    print("✅ Доброе утро!")

    subprocess.run([
        'osascript', '-e',
        'display notification "Доброе утро! Система готова к работе." with title "Wake Up"'
    ])


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return

    command = sys.argv[1].lower()

    if command == "on":
        enable_sleep_mode()
    elif command == "off":
        disable_sleep_mode()
    elif command == "auto":
        # Автоматический режим по времени
        hour = datetime.now().hour
        if 23 <= hour or hour < 7:
            enable_sleep_mode()
        else:
            disable_sleep_mode()
    else:
        print(__doc__)


if __name__ == "__main__":
    main()
