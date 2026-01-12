from __future__ import annotations

import json
import os
import tempfile
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable
from uuid import uuid4


def _utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


@dataclass
class ShoppingItem:
    id: str
    name: str
    qty: str = "1"
    notes: str = ""
    purchased: bool = False
    created_at: str = ""
    updated_at: str = ""

    @staticmethod
    def new(name: str, qty: str = "1", notes: str = "") -> "ShoppingItem":
        now = _utc_now_iso()
        return ShoppingItem(
            id=str(uuid4()),
            name=name.strip(),
            qty=(qty.strip() or "1"),
            notes=notes.strip(),
            purchased=False,
            created_at=now,
            updated_at=now,
        )

    def touch(self) -> None:
        self.updated_at = _utc_now_iso()


def default_data_path(app_name: str = "AuraShopList") -> Path:
    appdata = os.environ.get("APPDATA")
    if appdata:
        base = Path(appdata)
    else:
        base = Path.home() / ".config"

    folder = base / app_name
    folder.mkdir(parents=True, exist_ok=True)
    return folder / "shopping_list.json"


def _coerce_item(obj: dict[str, Any]) -> ShoppingItem:
    return ShoppingItem(
        id=str(obj.get("id") or uuid4()),
        name=str(obj.get("name") or "").strip(),
        qty=str(obj.get("qty") or "1"),
        notes=str(obj.get("notes") or ""),
        purchased=bool(obj.get("purchased") or False),
        created_at=str(obj.get("created_at") or ""),
        updated_at=str(obj.get("updated_at") or ""),
    )


def load_items(path: Path) -> list[ShoppingItem]:
    if not path.exists():
        return []

    raw = path.read_text(encoding="utf-8")
    if not raw.strip():
        return []

    data = json.loads(raw)

    if isinstance(data, dict) and isinstance(data.get("items"), list):
        items = data["items"]
    elif isinstance(data, list):
        items = data
    else:
        raise ValueError("Invalid shopping list format")

    result: list[ShoppingItem] = []
    for it in items:
        if isinstance(it, dict):
            item = _coerce_item(it)
            if item.name:
                result.append(item)

    return result


def save_items(path: Path, items: Iterable[ShoppingItem]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "version": 1,
        "updated_at": _utc_now_iso(),
        "items": [asdict(i) for i in items],
    }

    # atomic-ish save for Windows: write to temp then replace
    fd, tmp_name = tempfile.mkstemp(prefix=path.stem + "_", suffix=path.suffix, dir=str(path.parent))
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
            json.dump(payload, f, ensure_ascii=False, indent=2)

        os.replace(tmp_name, path)
    finally:
        try:
            if os.path.exists(tmp_name):
                os.remove(tmp_name)
        except OSError:
            pass
