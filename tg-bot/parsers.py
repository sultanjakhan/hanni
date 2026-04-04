"""Parse vacancy URLs into structured data."""
import re
import aiohttp
from bs4 import BeautifulSoup

HH_API = "https://api.hh.kz/vacancies"
HEADERS = {"User-Agent": "HanniBot/1.0"}


def detect_source(url: str) -> str:
    if "hh.kz" in url or "hh.ru" in url:
        return "hh"
    if "djinni.co" in url:
        return "djinni"
    if "linkedin.com" in url:
        return "linkedin"
    if "wellfound.com" in url or "angel.co" in url:
        return "wellfound"
    if "t.me/" in url:
        return "telegram"
    return "other"


def extract_hh_id(url: str) -> str | None:
    m = re.search(r"/vacancy/(\d+)", url)
    return m.group(1) if m else None


async def parse_url(url: str) -> dict:
    """Parse any vacancy URL into structured data."""
    source = detect_source(url)
    base = {"url": url, "source": source, "company": "", "position": "",
            "salary": "", "contact": "", "city": "", "experience": "", "notes": ""}

    try:
        if source == "hh":
            return await _parse_hh(url, base)
        else:
            return await _parse_generic(url, base)
    except Exception as e:
        base["notes"] = f"Parse error: {e}"
        return base


async def _parse_hh(url: str, base: dict) -> dict:
    vid = extract_hh_id(url)
    if not vid:
        return await _parse_generic(url, base)

    async with aiohttp.ClientSession() as s:
        async with s.get(f"{HH_API}/{vid}", headers=HEADERS) as resp:
            if resp.status != 200:
                return await _parse_generic(url, base)
            data = await resp.json()

    base["company"] = data.get("employer", {}).get("name", "")
    base["position"] = data.get("name", "")
    base["city"] = data.get("area", {}).get("name", "")
    base["experience"] = data.get("experience", {}).get("name", "")
    base["url"] = data.get("alternate_url", url)

    sal = data.get("salary")
    if sal:
        parts = []
        if sal.get("from"):
            parts.append(f"от {sal['from']}")
        if sal.get("to"):
            parts.append(f"до {sal['to']}")
        cur = sal.get("currency", "")
        base["salary"] = " ".join(parts) + f" {cur}" if parts else ""

    # Extract contact if available
    contacts = data.get("contacts")
    if contacts:
        name = contacts.get("name", "")
        phones = contacts.get("phones", [])
        emails = contacts.get("email", "")
        parts = [name]
        if emails:
            parts.append(emails)
        if phones:
            parts.append(phones[0].get("formatted", ""))
        base["contact"] = ", ".join(p for p in parts if p)

    # Snippet for notes
    snippet = data.get("snippet", {})
    req = snippet.get("requirement", "") or ""
    resp_text = snippet.get("responsibility", "") or ""
    base["notes"] = f"{req}\n{resp_text}".strip()

    return base


async def _parse_generic(url: str, base: dict) -> dict:
    """Scrape page title and meta for non-HH sites."""
    async with aiohttp.ClientSession() as s:
        async with s.get(url, headers=HEADERS, timeout=aiohttp.ClientTimeout(total=10)) as resp:
            if resp.status != 200:
                base["notes"] = f"HTTP {resp.status}"
                return base
            html = await resp.text()

    soup = BeautifulSoup(html, "html.parser")
    title = soup.title.string.strip() if soup.title and soup.title.string else ""

    # Try to extract company and position from title
    if " at " in title:
        pos, company = title.split(" at ", 1)
        base["position"] = pos.strip().split(" – ")[0].strip()
        base["company"] = company.strip().split(" – ")[0].strip()
    elif " — " in title:
        parts = title.split(" — ")
        base["position"] = parts[0].strip()
        base["company"] = parts[1].strip() if len(parts) > 1 else ""
    else:
        base["position"] = title[:100]

    # Try meta description
    meta = soup.find("meta", attrs={"name": "description"})
    if meta and meta.get("content"):
        base["notes"] = meta["content"][:300]

    return base
