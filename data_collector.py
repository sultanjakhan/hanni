"""
Data Collector for Fine-tuning
Собирает данные из разговоров для дообучения модели
"""
import json
import os
from datetime import datetime
from pathlib import Path
from typing import Optional

DATA_DIR = Path(__file__).parent / "training_data"
DATA_DIR.mkdir(exist_ok=True)

class DataCollector:
    """Собирает данные разговоров для fine-tuning"""

    def __init__(self):
        self.session_file = DATA_DIR / f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.jsonl"
        self.conversations_file = DATA_DIR / "all_conversations.jsonl"
        self.good_examples_file = DATA_DIR / "good_examples.jsonl"  # Отмеченные как хорошие
        self.corrections_file = DATA_DIR / "corrections.jsonl"  # Исправления пользователя

    def log_interaction(
        self,
        user_message: str,
        assistant_response: str,
        system_prompt: Optional[str] = None,
        tools_used: Optional[list] = None,
        metadata: Optional[dict] = None
    ):
        """Логирует взаимодействие"""
        entry = {
            "timestamp": datetime.now().isoformat(),
            "messages": []
        }

        if system_prompt:
            entry["messages"].append({
                "role": "system",
                "content": system_prompt
            })

        entry["messages"].append({
            "role": "user",
            "content": user_message
        })

        entry["messages"].append({
            "role": "assistant",
            "content": assistant_response
        })

        if tools_used:
            entry["tools_used"] = tools_used

        if metadata:
            entry["metadata"] = metadata

        # Сохраняем в оба файла
        self._append_jsonl(self.session_file, entry)
        self._append_jsonl(self.conversations_file, entry)

        return entry

    def mark_as_good(self, entry: dict, reason: Optional[str] = None):
        """Отмечает пример как хороший для обучения"""
        entry["marked_good"] = True
        entry["marked_at"] = datetime.now().isoformat()
        if reason:
            entry["reason"] = reason
        self._append_jsonl(self.good_examples_file, entry)

    def log_correction(
        self,
        original_response: str,
        corrected_response: str,
        user_message: str,
        feedback: Optional[str] = None
    ):
        """Логирует исправление пользователя"""
        entry = {
            "timestamp": datetime.now().isoformat(),
            "user_message": user_message,
            "original_response": original_response,
            "corrected_response": corrected_response,
            "feedback": feedback
        }
        self._append_jsonl(self.corrections_file, entry)

    def _append_jsonl(self, filepath: Path, data: dict):
        """Добавляет запись в JSONL файл"""
        with open(filepath, "a", encoding="utf-8") as f:
            f.write(json.dumps(data, ensure_ascii=False) + "\n")

    def get_stats(self) -> dict:
        """Возвращает статистику собранных данных"""
        stats = {
            "total_conversations": self._count_lines(self.conversations_file),
            "good_examples": self._count_lines(self.good_examples_file),
            "corrections": self._count_lines(self.corrections_file),
            "session_interactions": self._count_lines(self.session_file)
        }
        return stats

    def _count_lines(self, filepath: Path) -> int:
        """Считает строки в файле"""
        if not filepath.exists():
            return 0
        with open(filepath, "r") as f:
            return sum(1 for _ in f)

    def export_for_training(self, output_file: str = "training_dataset.jsonl"):
        """Экспортирует данные в формате для fine-tuning"""
        output_path = DATA_DIR / output_file

        # Приоритет: corrections > good_examples > all
        entries = []

        # Добавляем исправления (самые ценные)
        if self.corrections_file.exists():
            with open(self.corrections_file, "r") as f:
                for line in f:
                    data = json.loads(line)
                    entries.append({
                        "messages": [
                            {"role": "user", "content": data["user_message"]},
                            {"role": "assistant", "content": data["corrected_response"]}
                        ],
                        "weight": 3.0  # Высокий вес для исправлений
                    })

        # Добавляем хорошие примеры
        if self.good_examples_file.exists():
            with open(self.good_examples_file, "r") as f:
                for line in f:
                    data = json.loads(line)
                    entries.append({
                        "messages": data["messages"],
                        "weight": 2.0
                    })

        # Сохраняем
        with open(output_path, "w", encoding="utf-8") as f:
            for entry in entries:
                f.write(json.dumps(entry, ensure_ascii=False) + "\n")

        return output_path, len(entries)


# Глобальный экземпляр
collector = DataCollector()


def log(user_msg: str, assistant_msg: str, **kwargs):
    """Быстрый способ логировать"""
    return collector.log_interaction(user_msg, assistant_msg, **kwargs)


def good(entry: dict, reason: str = None):
    """Отметить как хороший пример"""
    collector.mark_as_good(entry, reason)


def correct(original: str, corrected: str, user_msg: str, feedback: str = None):
    """Записать исправление"""
    collector.log_correction(original, corrected, user_msg, feedback)


def stats():
    """Получить статистику"""
    return collector.get_stats()


def export():
    """Экспорт для обучения"""
    return collector.export_for_training()
