"""Run with python scripts/test-pocket-wrapper.py; no pocket-tts or torch needed."""
import importlib.util
import io
import sys
import types
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


def load():
    spec = importlib.util.spec_from_file_location(
        "pocket_wrapper", Path(__file__).parent / "pocket" / "copyspeak-pocket.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class StopAfterLoad(Exception):
    """Ends main() right after the model load; no synthesis is needed."""


class FakeModel:
    def get_state_for_audio_prompt(self, voice):
        raise StopAfterLoad()


class PocketLanguageTests(unittest.TestCase):
    def setUp(self):
        self.wrapper = load()

    def run_main(self, voice, load_model):
        fake = types.ModuleType("pocket_tts")
        fake.TTSModel = SimpleNamespace(load_model=load_model)
        argv = ["copyspeak-pocket.py", "--text", "hello", "--output", "out.wav", "--voice", voice]
        stderr = io.StringIO()
        with patch.dict(sys.modules, {"pocket_tts": fake}), patch.object(
            sys, "argv", argv
        ), redirect_stderr(stderr):
            code = self.wrapper.main()
        return code, stderr.getvalue()

    def test_non_english_voices_map_to_their_language_model(self):
        self.assertEqual(self.wrapper.model_language("estelle"), "french")
        self.assertEqual(self.wrapper.model_language("lola"), "spanish")
        self.assertEqual(self.wrapper.model_language("giovanni"), "italian")
        self.assertEqual(self.wrapper.model_language("juergen"), "german")
        self.assertEqual(self.wrapper.model_language("rafael"), "portuguese")

    def test_default_voices_keep_the_default_model(self):
        self.assertIsNone(self.wrapper.model_language("alba"))
        self.assertIsNone(self.wrapper.model_language("Juergen"))

    def test_mapped_voice_loads_its_language_model(self):
        calls = []

        def load_model(**kwargs):
            calls.append(kwargs)
            return FakeModel()

        code, _ = self.run_main("juergen", load_model)
        self.assertEqual(calls, [{"language": "german"}])
        self.assertEqual(code, 1)

    def test_default_voice_loads_without_a_language_argument(self):
        calls = []

        def load_model(**kwargs):
            calls.append(kwargs)
            return FakeModel()

        code, _ = self.run_main("alba", load_model)
        self.assertEqual(calls, [{}])
        self.assertEqual(code, 1)

    def test_old_pocket_tts_without_language_models_fails_loudly_with_a_reinstall_hint(self):
        def load_model():  # an old pocket-tts: no language parameter
            return FakeModel()

        code, stderr = self.run_main("giovanni", load_model)
        self.assertEqual(code, 1)
        self.assertIn("ERROR:", stderr)
        self.assertIn("install-pocket.ps1 -Force", stderr)


if __name__ == "__main__":
    unittest.main()
