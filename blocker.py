#!/usr/bin/env python3
"""
Focus Blocker - Блокировка отвлекающих сайтов и приложений
Использование:
  python blocker.py block    - Включить блокировку
  python blocker.py unblock  - Выключить блокировку
  python blocker.py status   - Статус
  python blocker.py add <site>    - Добавить сайт
  python blocker.py remove <site> - Удалить сайт
  python blocker.py list     - Список заблокированных
"""
import subprocess
import sys
import os
import json
from pathlib import Path
from datetime import datetime

CONFIG_FILE = Path.home() / "hanni" / "blocker_config.json"
HOSTS_FILE = "/etc/hosts"
BLOCK_MARKER = "# === HANNI FOCUS BLOCKER ==="

# Дефолтные сайты для блокировки
DEFAULT_SITES = [
    "youtube.com",
    "www.youtube.com",
    "twitter.com",
    "www.twitter.com",
    "x.com",
    "www.x.com",
    "instagram.com",
    "www.instagram.com",
    "facebook.com",
    "www.facebook.com",
    "tiktok.com",
    "www.tiktok.com",
    "reddit.com",
    "www.reddit.com",
    "vk.com",
    "www.vk.com",
    "netflix.com",
    "www.netflix.com",
]

# Приложения для блокировки (Mac)
DEFAULT_APPS = [
    "Telegram",
    "Discord",
    "Slack",
    "Safari",  # можно блокировать если нужен только Chrome для работы
]


def load_config():
    if CONFIG_FILE.exists():
        return json.loads(CONFIG_FILE.read_text())
    return {
        "sites": DEFAULT_SITES.copy(),
        "apps": DEFAULT_APPS.copy(),
        "blocked": False,
        "schedule": {
            "work_start": "09:00",
            "work_end": "18:00",
            "block_on_work": True
        }
    }


def save_config(config):
    CONFIG_FILE.write_text(json.dumps(config, indent=2, ensure_ascii=False))


def block_sites(sites):
    """Add sites to /etc/hosts"""
    try:
        # Read current hosts
        with open(HOSTS_FILE, 'r') as f:
            content = f.read()

        # Remove old blocks
        if BLOCK_MARKER in content:
            lines = content.split('\n')
            new_lines = []
            skip = False
            for line in lines:
                if line.strip() == BLOCK_MARKER:
                    skip = not skip
                    continue
                if not skip:
                    new_lines.append(line)
            content = '\n'.join(new_lines)

        # Add new blocks
        block_lines = [BLOCK_MARKER]
        for site in sites:
            block_lines.append(f"127.0.0.1 {site}")
        block_lines.append(BLOCK_MARKER)

        new_content = content.rstrip() + '\n\n' + '\n'.join(block_lines) + '\n'

        # Write with sudo
        process = subprocess.run(
            ['sudo', 'tee', HOSTS_FILE],
            input=new_content.encode(),
            capture_output=True
        )

        # Flush DNS cache
        subprocess.run(['sudo', 'dscacheutil', '-flushcache'], capture_output=True)
        subprocess.run(['sudo', 'killall', '-HUP', 'mDNSResponder'], capture_output=True)

        return True
    except Exception as e:
        print(f"Ошибка: {e}")
        return False


def unblock_sites():
    """Remove blocks from /etc/hosts"""
    try:
        with open(HOSTS_FILE, 'r') as f:
            content = f.read()

        if BLOCK_MARKER not in content:
            return True

        lines = content.split('\n')
        new_lines = []
        skip = False
        for line in lines:
            if line.strip() == BLOCK_MARKER:
                skip = not skip
                continue
            if not skip:
                new_lines.append(line)

        new_content = '\n'.join(new_lines)

        process = subprocess.run(
            ['sudo', 'tee', HOSTS_FILE],
            input=new_content.encode(),
            capture_output=True
        )

        # Flush DNS
        subprocess.run(['sudo', 'dscacheutil', '-flushcache'], capture_output=True)
        subprocess.run(['sudo', 'killall', '-HUP', 'mDNSResponder'], capture_output=True)

        return True
    except Exception as e:
        print(f"Ошибка: {e}")
        return False


def block_apps(apps):
    """Block apps using macOS permissions (requires Screen Time or custom method)"""
    # Простой метод - переименовать приложения (требует sudo)
    # Более мягкий метод - показывать уведомление
    for app in apps:
        app_path = f"/Applications/{app}.app"
        if os.path.exists(app_path):
            # Создаём скрипт который показывает предупреждение при запуске
            print(f"  Блокировка {app}: используй Screen Time в настройках Mac")
    return True


def get_status():
    """Check if blocking is active"""
    try:
        with open(HOSTS_FILE, 'r') as f:
            content = f.read()
        return BLOCK_MARKER in content
    except:
        return False


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return

    command = sys.argv[1].lower()
    config = load_config()

    if command == "block":
        print("🔒 Включаю блокировку...")
        if block_sites(config["sites"]):
            config["blocked"] = True
            config["blocked_at"] = datetime.now().isoformat()
            save_config(config)
            print(f"✅ Заблокировано {len(config['sites'])} сайтов")
            print("\n📱 Для блокировки приложений:")
            print("   Настройки → Экранное время → Ограничения приложений")
        else:
            print("❌ Ошибка блокировки")

    elif command == "unblock":
        print("🔓 Выключаю блокировку...")
        if unblock_sites():
            config["blocked"] = False
            save_config(config)
            print("✅ Блокировка снята")
        else:
            print("❌ Ошибка")

    elif command == "status":
        is_blocked = get_status()
        print(f"Статус: {'🔒 Заблокировано' if is_blocked else '🔓 Разблокировано'}")
        print(f"Сайтов в списке: {len(config['sites'])}")

    elif command == "list":
        print("📋 Заблокированные сайты:")
        for site in config["sites"]:
            print(f"  • {site}")

    elif command == "add" and len(sys.argv) > 2:
        site = sys.argv[2].lower()
        if not site.startswith("www."):
            config["sites"].append(site)
            config["sites"].append(f"www.{site}")
        else:
            config["sites"].append(site)
        save_config(config)
        print(f"✅ Добавлено: {site}")
        if config["blocked"]:
            print("   Перезапусти блокировку: python blocker.py block")

    elif command == "remove" and len(sys.argv) > 2:
        site = sys.argv[2].lower()
        config["sites"] = [s for s in config["sites"] if site not in s]
        save_config(config)
        print(f"✅ Удалено: {site}")

    else:
        print(__doc__)


if __name__ == "__main__":
    main()
