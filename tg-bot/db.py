"""SQLite storage for vacancies."""
import aiosqlite
import os

DB_PATH = os.environ.get("BOT_DB_PATH", "vacancies.db")


async def init_db():
    async with aiosqlite.connect(DB_PATH) as db:
        await db.execute("""
            CREATE TABLE IF NOT EXISTS vacancies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                company TEXT NOT NULL DEFAULT '',
                position TEXT NOT NULL DEFAULT '',
                salary TEXT NOT NULL DEFAULT '',
                url TEXT NOT NULL UNIQUE,
                stage TEXT NOT NULL DEFAULT 'found',
                contact TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT '',
                city TEXT NOT NULL DEFAULT '',
                experience TEXT NOT NULL DEFAULT '',
                notes TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
        """)
        await db.commit()


async def add_vacancy(data: dict) -> int | None:
    """Insert vacancy, return id. None if duplicate URL."""
    async with aiosqlite.connect(DB_PATH) as db:
        try:
            cursor = await db.execute(
                """INSERT INTO vacancies (company, position, salary, url, stage, contact, source, city, experience, notes)
                   VALUES (:company, :position, :salary, :url, 'found', :contact, :source, :city, :experience, :notes)""",
                data,
            )
            await db.commit()
            return cursor.lastrowid
        except aiosqlite.IntegrityError:
            return None


async def update_stage(vacancy_id: int, stage: str):
    async with aiosqlite.connect(DB_PATH) as db:
        await db.execute(
            "UPDATE vacancies SET stage=?, updated_at=datetime('now') WHERE id=?",
            (stage, vacancy_id),
        )
        await db.commit()


async def update_contact(vacancy_id: int, contact: str):
    async with aiosqlite.connect(DB_PATH) as db:
        await db.execute(
            "UPDATE vacancies SET contact=?, updated_at=datetime('now') WHERE id=?",
            (contact, vacancy_id),
        )
        await db.commit()


async def get_vacancy(vacancy_id: int) -> dict | None:
    async with aiosqlite.connect(DB_PATH) as db:
        db.row_factory = aiosqlite.Row
        cursor = await db.execute("SELECT * FROM vacancies WHERE id=?", (vacancy_id,))
        row = await cursor.fetchone()
        return dict(row) if row else None


async def list_vacancies(stage: str | None = None) -> list[dict]:
    async with aiosqlite.connect(DB_PATH) as db:
        db.row_factory = aiosqlite.Row
        if stage:
            cursor = await db.execute(
                "SELECT * FROM vacancies WHERE stage=? ORDER BY created_at DESC", (stage,)
            )
        else:
            cursor = await db.execute("SELECT * FROM vacancies ORDER BY created_at DESC")
        return [dict(r) for r in await cursor.fetchall()]


async def get_new_since(after_id: int) -> list[dict]:
    """Get vacancies added after given id (for Hanni sync)."""
    async with aiosqlite.connect(DB_PATH) as db:
        db.row_factory = aiosqlite.Row
        cursor = await db.execute(
            "SELECT * FROM vacancies WHERE id > ? ORDER BY id", (after_id,)
        )
        return [dict(r) for r in await cursor.fetchall()]
