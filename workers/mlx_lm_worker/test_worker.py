import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


HERE = pathlib.Path(__file__).resolve().parent
WORKER_PATH = HERE / "worker.py"


def load_worker():
    spec = importlib.util.spec_from_file_location("kairos_mlx_worker", WORKER_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class WorkerContractTests(unittest.TestCase):
    def request(self):
        return {
            "protocol_version": 1,
            "run_id": "run-001",
            "operation": "smoke_noop",
            "requested_at": "2026-08-20T00:00:00Z",
            "total_iterations": 20,
        }

    def test_validation_is_versioned_exact_and_smoke_only(self):
        worker = load_worker()
        self.assertEqual(worker.validate_request(self.request())["run_id"], "run-001")

        for key, value in [
            ("protocol_version", 2),
            ("operation", "train_qlora"),
            ("run_id", "../escape"),
            ("total_iterations", 0),
        ]:
            invalid = self.request()
            invalid[key] = value
            with self.assertRaises(worker.RequestError):
                worker.validate_request(invalid)

        extra = self.request()
        extra["model_path"] = "/tmp/untrusted"
        with self.assertRaises(worker.RequestError):
            worker.validate_request(extra)

    def test_stdio_mode_is_deterministic_and_emits_only_safe_protocol_events(self):
        payload = json.dumps(self.request(), separators=(",", ":")).encode()
        with tempfile.TemporaryDirectory() as directory:
            before = set(pathlib.Path(directory).iterdir())
            first = subprocess.run(
                [sys.executable, str(WORKER_PATH), "--stdio"],
                input=payload,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                cwd=directory,
                check=False,
            )
            second = subprocess.run(
                [sys.executable, str(WORKER_PATH), "--stdio"],
                input=payload,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                cwd=directory,
                check=False,
            )
            self.assertEqual(before, set(pathlib.Path(directory).iterdir()))

        self.assertEqual(first.returncode, 0, first.stderr.decode())
        self.assertEqual(first.stderr, b"")
        self.assertEqual(first.stdout, second.stdout)
        events = [json.loads(line) for line in first.stdout.splitlines()]
        self.assertEqual([event["type"] for event in events], ["started", "progress", "metric", "completed"])
        self.assertTrue(all(event["protocol_version"] == 1 for event in events))
        self.assertTrue(all(event["run_id"] == "run-001" for event in events))
        rendered = first.stdout.decode()
        for forbidden in ["prompt", "reasoning", "secret", "source_text", "model_path"]:
            self.assertNotIn(forbidden, rendered)

    def test_invalid_and_oversized_input_fails_without_protocol_output(self):
        invalid = self.request()
        invalid["operation"] = "train_qlora"
        for payload in [json.dumps(invalid).encode(), b"{" + b" " * (64 * 1024) + b"}"]:
            result = subprocess.run(
                [sys.executable, str(WORKER_PATH), "--stdio"],
                input=payload,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, b"")
            self.assertLessEqual(len(result.stderr), 256)

    def test_scaffold_contains_no_training_or_network_runtime(self):
        source = WORKER_PATH.read_text(encoding="utf-8")
        for forbidden in ["import mlx", "import socket", "import urllib", "import requests", "huggingface_hub"]:
            self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
