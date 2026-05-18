"""CopySpeak project/session document model."""
from __future__ import annotations

import json, os, time, uuid
from copy import deepcopy
from pathlib import Path

DEFAULT_CONFIG = {
    "engine": "kitten",
    "voice": "expr-voice-2-m",
    "speed": 1.0,
    "pitch": 1.0,
    "volume": 1.0,
    "format": "wav",
    "sanitize_markdown": True,
    "normalize_text": True,
}


def create_project(name="Untitled", config=None):
    return {
        "schema": "cli-anything-copyspeak/v1",
        "id": str(uuid.uuid4()),
        "name": name,
        "created_at": time.time(),
        "modified_at": time.time(),
        "config": {**DEFAULT_CONFIG, **(config or {})},
        "queue": [],
        "history": [],
    }


def load_project(path):
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
    if data.get("schema") != "cli-anything-copyspeak/v1":
        raise ValueError("Not a CopySpeak CLI project (expected schema cli-anything-copyspeak/v1)")
    return data


def save_project(project, path):
    project = deepcopy(project)
    project["modified_at"] = time.time()
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(project, f, indent=2, ensure_ascii=False)
    return {"path": os.path.abspath(path), "project": project}


def info(project):
    return {
        "id": project["id"], "name": project["name"], "engine": project["config"].get("engine"),
        "voice": project["config"].get("voice"), "queue_count": len(project.get("queue", [])),
        "history_count": len(project.get("history", [])), "modified_at": project.get("modified_at"),
    }


def set_config(project, **updates):
    allowed = set(DEFAULT_CONFIG)
    bad = [k for k in updates if k not in allowed]
    if bad:
        raise ValueError(f"Unknown config keys: {', '.join(bad)}")
    project["config"].update({k: v for k, v in updates.items() if v is not None})
    project["modified_at"] = time.time()
    return project["config"]
