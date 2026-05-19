from __future__ import annotations
import time, uuid


def add_text(project, text, label=None):
    if not text or not text.strip():
        raise ValueError("Text is required")
    item = {"id": str(uuid.uuid4()), "text": text, "label": label or text[:40], "created_at": time.time(), "status": "queued"}
    project.setdefault("queue", []).append(item)
    project["modified_at"] = time.time()
    return item


def list_items(project):
    return list(project.get("queue", []))


def remove_item(project, item_id):
    before = len(project.get("queue", []))
    project["queue"] = [x for x in project.get("queue", []) if x.get("id") != item_id]
    if len(project["queue"]) == before:
        raise ValueError(f"Queue item not found: {item_id}")
    project["modified_at"] = time.time()
    return {"removed": item_id, "queue_count": len(project["queue"])}


def clear(project):
    count = len(project.get("queue", []))
    project["queue"] = []
    project["modified_at"] = time.time()
    return {"cleared": count}
