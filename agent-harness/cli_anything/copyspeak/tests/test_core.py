import json, pytest
from cli_anything.copyspeak.core import project, queue


def test_create_project_defaults():
    p = project.create_project("Demo")
    assert p["schema"] == "cli-anything-copyspeak/v1"
    assert p["name"] == "Demo"
    assert p["config"]["engine"] == "kitten"


def test_save_load_roundtrip(tmp_path):
    path = tmp_path / "p.json"
    p = project.create_project()
    project.save_project(p, path)
    assert project.load_project(path)["id"] == p["id"]


def test_load_rejects_invalid_schema(tmp_path):
    path = tmp_path / "bad.json"
    path.write_text(json.dumps({"schema": "bad"}))
    with pytest.raises(ValueError): project.load_project(path)


def test_set_config_rejects_unknown():
    with pytest.raises(ValueError): project.set_config(project.create_project(), nope=True)


def test_queue_add():
    p = project.create_project(); item = queue.add_text(p, "hello", "h")
    assert item["text"] == "hello" and len(p["queue"]) == 1


def test_queue_blank_rejected():
    with pytest.raises(ValueError): queue.add_text(project.create_project(), "  ")


def test_queue_remove():
    p = project.create_project(); item = queue.add_text(p, "hello")
    res = queue.remove_item(p, item["id"])
    assert res["queue_count"] == 0


def test_queue_clear():
    p = project.create_project(); queue.add_text(p, "a"); queue.add_text(p, "b")
    assert queue.clear(p)["cleared"] == 2
