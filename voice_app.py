#!/usr/bin/env python3
"""
OpenClaw Voice Assistant - macOS Menu Bar App
Hanni Project
"""

import os
import sys
import json
import subprocess
import threading
import time
import logging
import re
from datetime import datetime
from pathlib import Path

# Пути
APP_NAME = "Hanni Voice"
DATA_DIR = Path.home() / "Documents" / "Hanni"
LOG_DIR = DATA_DIR / "logs"
LOG_FILE = DATA_DIR / "activity_log.json"
CONFIG_FILE = DATA_DIR / "config.json"

# Создаём папки
DATA_DIR.mkdir(parents=True, exist_ok=True)
LOG_DIR.mkdir(parents=True, exist_ok=True)

# Логирование
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.FileHandler(LOG_DIR / "voice_app.log"),
        logging.StreamHandler()
    ]
)
log = logging.getLogger(__name__)

try:
    import rumps
    import speech_recognition as sr
    import requests
except ImportError as e:
    log.error(f"Missing: {e}. Run: pip install rumps SpeechRecognition requests pyaudio")
    sys.exit(1)

# Конфиг по умолчанию
DEFAULT_CONFIG = {
    "mlx_url": "http://localhost:8000/v1/chat/completions",
    "model": "mlx-community/GLM-4.7-Flash-6bit",
    "version": "2.0.0",
    "timeout": 30,
    "max_tokens": 300,
    "listen_timeout": 10,
    "phrase_timeout": 15,
    "auto_listen": False,
    "wake_word": "ханни"
}

def load_config():
    if CONFIG_FILE.exists():
        try:
            with open(CONFIG_FILE) as f:
                return {**DEFAULT_CONFIG, **json.load(f)}
        except:
            pass
    return DEFAULT_CONFIG.copy()

def save_config(cfg):
    with open(CONFIG_FILE, 'w') as f:
        json.dump(cfg, f, indent=2, ensure_ascii=False)

CONFIG = load_config()


class HanniVoiceApp(rumps.App):
    def __init__(self):
        super().__init__("🎤", quit_button=None)

        self.auto_listen_item = rumps.MenuItem(
            "🔄 Авто-слушание",
            callback=self.toggle_auto_listen
        )
        self.auto_listen_item.state = CONFIG.get("auto_listen", False)

        self.menu = [
            rumps.MenuItem("🎙 Слушать", callback=self.on_listen),
            rumps.MenuItem("📝 Написать", callback=self.on_text),
            self.auto_listen_item,
            None,
            rumps.MenuItem("📁 Открыть данные", callback=self.on_open_data),
            rumps.MenuItem("⚙️ Настройки", callback=self.on_settings),
            None,
            rumps.MenuItem("❌ Выход", callback=self.on_quit)
        ]

        self.is_busy = False
        self.auto_listening = False
        self.stop_auto_listen = threading.Event()
        self.activity_log = self._load_log()
        self.recognizer = sr.Recognizer()

        # Проверка MLX при старте
        threading.Thread(target=self._check_mlx, daemon=True).start()

        # Авто-слушание если включено
        if CONFIG.get("auto_listen", False):
            self._start_auto_listen()

        log.info(f"{APP_NAME} запущен")

    def _load_log(self):
        if LOG_FILE.exists():
            try:
                with open(LOG_FILE) as f:
                    return json.load(f)
            except:
                pass
        return []

    def _save_log(self):
        with open(LOG_FILE, 'w') as f:
            json.dump(self.activity_log, f, ensure_ascii=False, indent=2)

    def _log_activity(self, atype, data):
        self.activity_log.append({
            "timestamp": datetime.now().isoformat(),
            "type": atype,
            "data": data
        })
        self._save_log()

    def _check_mlx(self):
        try:
            r = requests.get(CONFIG["mlx_url"].replace("/chat/completions", "/models"), timeout=5)
            if r.status_code == 200:
                log.info("MLX OK")
                rumps.notification(APP_NAME, "Готов", "MLX подключен")
            else:
                raise Exception("Bad status")
        except:
            log.warning("MLX недоступен")
            rumps.notification(APP_NAME, "⚠️", "MLX сервер недоступен")

    def _speak(self, text):
        if text:
            subprocess.run(["say", "-v", "Milena", "-r", "210", text[:500]], check=False)

    def _ask_mlx(self, text):
        # Простой промпт для быстрого ответа
        system = """Ты голосовой ассистент. Отвечай ТОЛЬКО на русском языке.
Давай короткий прямой ответ, 1-2 предложения. Не объясняй свои рассуждения."""

        try:
            log.info(f"MLX: {text[:50]}")
            r = requests.post(
                CONFIG["mlx_url"],
                json={
                    "model": CONFIG["model"],
                    "messages": [
                        {"role": "system", "content": system},
                        {"role": "user", "content": text}
                    ],
                    "max_tokens": CONFIG.get("max_tokens", 300),
                    "temperature": 0.7
                },
                timeout=CONFIG.get("timeout", 30)
            )

            if r.status_code == 200:
                msg = r.json()["choices"][0]["message"]
                answer = msg.get("content", "").strip()

                # Для reasoning моделей - ищем русский текст
                if not answer and msg.get("reasoning"):
                    reasoning = msg["reasoning"]
                    # Ищем строки на русском (содержат кириллицу)
                    import re
                    russian_lines = [l.strip() for l in reasoning.split("\n")
                                   if re.search('[а-яА-ЯёЁ]', l) and not l.startswith('*')]
                    if russian_lines:
                        answer = russian_lines[-1]
                    else:
                        answer = "Не могу ответить"

                # Убираем маркеры списков
                answer = re.sub(r'^[\*\-\d\.]+\s*', '', answer).strip()

                log.info(f"Response: {answer[:50]}")
                return answer if answer else "Не понял вопрос"
            return f"Ошибка {r.status_code}"
        except requests.Timeout:
            return "Модель думает слишком долго"
        except Exception as e:
            log.error(f"MLX error: {e}")
            return "Ошибка подключения"

    def _process(self, text):
        t = text.lower()

        # Быстрые команды
        if any(w in t for w in ["проснулся", "доброе утро", "встал"]):
            self._log_activity("wake_up", {"time": datetime.now().strftime("%H:%M")})
            return "Доброе утро!"

        if any(w in t for w in ["спокойной ночи", "иду спать"]):
            self._log_activity("sleep", {"time": datetime.now().strftime("%H:%M")})
            return "Спокойной ночи!"

        if "стоп" in t or "хватит" in t:
            if self.auto_listening:
                self._stop_auto_listen()
                return "Выключаю авто-слушание"

        return self._ask_mlx(text)

    def _listen_once(self, auto_mode=False):
        """Одиночное прослушивание"""
        try:
            with sr.Microphone() as source:
                self.recognizer.energy_threshold = 300
                self.recognizer.dynamic_energy_threshold = True
                self.recognizer.adjust_for_ambient_noise(source, duration=0.3)

                if auto_mode:
                    # В авто-режиме слушаем постоянно
                    audio = self.recognizer.listen(source, phrase_time_limit=10)
                else:
                    audio = self.recognizer.listen(
                        source,
                        timeout=CONFIG.get("listen_timeout", 10),
                        phrase_time_limit=CONFIG.get("phrase_timeout", 15)
                    )

            text = self.recognizer.recognize_google(audio, language="ru-RU")
            log.info(f"Heard: {text}")
            return text

        except sr.WaitTimeoutError:
            return None
        except sr.UnknownValueError:
            return None
        except Exception as e:
            log.error(f"Listen error: {e}")
            return None

    def _listen_thread(self):
        """Слушание по кнопке"""
        try:
            self.title = "🔴"
            text = self._listen_once(auto_mode=False)

            if text:
                rumps.notification(APP_NAME, "Ты:", text)
                self.title = "🤔"
                response = self._process(text)
                rumps.notification(APP_NAME, "Ханни:", response[:100])
                self._speak(response)
            else:
                self.title = "🎤"

        except Exception as e:
            log.error(f"Error: {e}")
        finally:
            self.title = "🟢" if self.auto_listening else "🎤"
            self.is_busy = False

    def _auto_listen_loop(self):
        """Цикл авто-прослушивания"""
        log.info("Auto-listen started")
        wake_word = CONFIG.get("wake_word", "ханни").lower()

        while not self.stop_auto_listen.is_set():
            try:
                self.title = "🟢"
                text = self._listen_once(auto_mode=True)

                if text and not self.stop_auto_listen.is_set():
                    t = text.lower()

                    # Проверяем wake word или обрабатываем всё
                    if wake_word in t or not wake_word:
                        # Убираем wake word из текста
                        query = t.replace(wake_word, "").strip()
                        if not query:
                            query = text  # Если только wake word - используем всё

                        log.info(f"Processing: {query}")
                        self.title = "🤔"
                        rumps.notification(APP_NAME, "Ты:", text)

                        response = self._process(query)
                        rumps.notification(APP_NAME, "Ханни:", response[:100])
                        self._speak(response)

            except Exception as e:
                log.error(f"Auto-listen error: {e}")
                time.sleep(1)

        log.info("Auto-listen stopped")
        self.title = "🎤"
        self.auto_listening = False

    def _start_auto_listen(self):
        if not self.auto_listening:
            self.auto_listening = True
            self.stop_auto_listen.clear()
            threading.Thread(target=self._auto_listen_loop, daemon=True).start()
            self.title = "🟢"
            log.info("Auto-listen enabled")

    def _stop_auto_listen(self):
        if self.auto_listening:
            self.stop_auto_listen.set()
            self.auto_listening = False
            self.title = "🎤"
            log.info("Auto-listen disabled")

    @rumps.clicked("🎙 Слушать")
    def on_listen(self, _):
        if self.is_busy:
            return
        self.is_busy = True
        threading.Thread(target=self._listen_thread, daemon=True).start()

    @rumps.clicked("📝 Написать")
    def on_text(self, _):
        if self.is_busy:
            return

        w = rumps.Window("Сообщение:", APP_NAME, ok="Отправить", cancel="Отмена")
        r = w.run()
        if r.clicked and r.text:
            self.is_busy = True
            self.title = "🤔"

            def process():
                try:
                    response = self._process(r.text)
                    rumps.notification(APP_NAME, "Ханни:", response[:100])
                    self._speak(response)
                finally:
                    self.title = "🟢" if self.auto_listening else "🎤"
                    self.is_busy = False

            threading.Thread(target=process, daemon=True).start()

    def toggle_auto_listen(self, sender):
        if self.auto_listening:
            self._stop_auto_listen()
            sender.state = False
            CONFIG["auto_listen"] = False
        else:
            self._start_auto_listen()
            sender.state = True
            CONFIG["auto_listen"] = True
        save_config(CONFIG)

    @rumps.clicked("📁 Открыть данные")
    def on_open_data(self, _):
        subprocess.run(["open", str(DATA_DIR)])

    @rumps.clicked("⚙️ Настройки")
    def on_settings(self, _):
        info = f"""Model: {CONFIG['model'].split('/')[-1]}
Auto-listen: {'Вкл' if self.auto_listening else 'Выкл'}
Wake word: "{CONFIG.get('wake_word', 'ханни')}"
Version: {CONFIG['version']}"""
        rumps.alert("Настройки", info)

    @rumps.clicked("❌ Выход")
    def on_quit(self, _):
        self._stop_auto_listen()
        log.info("Shutting down")
        rumps.quit_application()


if __name__ == "__main__":
    HanniVoiceApp().run()
