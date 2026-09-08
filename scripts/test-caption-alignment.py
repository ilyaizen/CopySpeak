"""Run with python scripts/test-caption-alignment.py; no engine/model dependency."""
import importlib.util
import io
import json
from pathlib import Path
from types import SimpleNamespace
import unittest


def load(relative):
    spec = importlib.util.spec_from_file_location("wrapper", Path(__file__).parent / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


piper = load("piper/copyspeak-piper.py")
edge = load("edge/copyspeak-edge.py")


class AlignmentTests(unittest.TestCase):
    def test_native_sample_counts_preserve_silence_and_sentence_offsets(self):
        alignment = [SimpleNamespace(phoneme=p, num_samples=n) for p, n in
                     [("^", 100), ("h", 40), ("i", 60), (",", 250), (" ", 50), ("b", 70), ("$", 100)]]
        self.assertEqual(piper.timed_phone_groups(alignment, 1000, 1000),
                         [("hi", 1100, 1200), ("b", 1500, 1570)])

    def test_normalization_and_context_mismatch_do_not_invent_word_boundaries(self):
        class Voice:
            def phonemize(self, text):
                return [list({"Pay": "peɪ", "$12.": "twɛlv dɑləɹz.", "the": "ði",
                              "Pay $12. the": "peɪ twɛlv dɑləɹz. ðə"}[text])]
        actual, mapping = piper.word_phone_map(Voice(), "Pay $12. the")
        self.assertEqual(actual, ["peɪ", "twɛlv", "dɑləɹz", "ðə"])
        self.assertEqual(mapping, [(0, 3, 0, 0), (4, 8, 1, 2)])

    def test_edge_preserves_punctuation_and_unicode_offsets(self):
        words = [{"text_start": 0, "text_end": 2, "start_ms": 150, "end_ms": 400},
                 {"text_start": 3, "text_end": 5, "start_ms": 900, "end_ms": 1200}]
        aligned = edge.preserve_source_text("😀 Hi, hi!", "Hi hi", words)
        self.assertEqual(aligned["text"], "😀 Hi, hi!")
        self.assertEqual([w["text_start"] for w in aligned["words"]], [3, 7])
        self.assertEqual(aligned["words"][1]["start_ms"], 900)
        normalized = edge.preserve_source_text("$12", "twelve", [{**words[0], "text_end": 6}])
        self.assertEqual(normalized["text"], "twelve")


if __name__ == "__main__":
    unittest.main()
