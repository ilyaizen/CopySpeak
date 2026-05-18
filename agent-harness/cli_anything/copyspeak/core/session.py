from __future__ import annotations
import json, os
from copy import deepcopy
from .project import create_project


def _locked_save_json(path, data, **dump_kwargs):
    try: f = open(path, "r+", encoding="utf-8")
    except FileNotFoundError:
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        f = open(path, "w", encoding="utf-8")
    with f:
        locked = False
        try:
            import fcntl
            fcntl.flock(f.fileno(), fcntl.LOCK_EX); locked = True
        except (ImportError, OSError): pass
        try:
            f.seek(0); f.truncate(); json.dump(data, f, **dump_kwargs); f.flush()
        finally:
            if locked: fcntl.flock(f.fileno(), fcntl.LOCK_UN)


def new_session(project_path=None):
    return {"project_path": project_path, "project": create_project(), "undo": [], "redo": [], "modified": False}


def load_session(path):
    with open(path, "r", encoding="utf-8") as f: return json.load(f)


def save_session(session, path):
    _locked_save_json(path, session, indent=2, ensure_ascii=False)
    return {"session": os.path.abspath(path)}


def snapshot(session):
    session.setdefault("undo", []).append(deepcopy(session["project"]))
    session["redo"] = []


def undo(session):
    if not session.get("undo"): raise ValueError("Nothing to undo")
    session.setdefault("redo", []).append(deepcopy(session["project"]))
    session["project"] = session["undo"].pop(); session["modified"] = True
    return session["project"]


def redo(session):
    if not session.get("redo"): raise ValueError("Nothing to redo")
    session.setdefault("undo", []).append(deepcopy(session["project"]))
    session["project"] = session["redo"].pop(); session["modified"] = True
    return session["project"]
