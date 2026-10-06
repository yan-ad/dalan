"""Preview contract tests; no native app, database, or Bacon install required."""

import importlib.machinery
import importlib.util
import os
from pathlib import Path
import signal
import subprocess
import sys
import tomllib
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
LOADER = importlib.machinery.SourceFileLoader("dev_preview", str(ROOT / "scripts/dev-preview"))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
PREVIEW = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(PREVIEW)


class PreviewTests(unittest.TestCase):
    def test_bacon_foreground_restart_and_scoped_stop(self):
        self.assertTrue(os.access(ROOT / "scripts/dev-preview", os.X_OK))
        config = tomllib.loads((ROOT / "bacon.toml").read_text())
        self.assertEqual(config["default_job"], "preview")
        job = config["jobs"]["preview"]
        self.assertEqual(job["command"], ["./scripts/dev-preview"])
        self.assertEqual(job["kill"], ["./scripts/dev-preview", "--stop"])
        self.assertEqual(job["on_change_strategy"], "kill_then_restart")
        self.assertFalse(job["background"])
        self.assertTrue(job["need_stdout"])
        self.assertIn("crates", job["watch"])
        self.assertNotIn("target", job["watch"])
        for name in ("check", "test", "lint", "ui-tests"):
            self.assertIn("--locked", config["jobs"][name]["command"])
        for action in config["keybindings"].values():
            self.assertIn(action.removeprefix("job:"), config["jobs"])

    def test_launch_execs_existing_bundle_helper_and_preserves_backtrace(self):
        with patch.object(PREVIEW.sys, "platform", "darwin"), \
             patch.object(PREVIEW.os, "getpgrp", return_value=101), \
             patch.object(PREVIEW.os, "getpid", return_value=202), \
             patch.object(PREVIEW.os, "setpgid") as group, \
             patch.object(PREVIEW.subprocess, "run") as check, \
             patch.object(PREVIEW.os, "execv") as execute, \
             patch.dict(os.environ, {"RUST_BACKTRACE": "full"}):
            PREVIEW.main([])
            group.assert_called_once_with(0, 0)
            check.assert_called_once_with([
                "cargo", "check", "-p", "dalan-app", "--bin", "dalan",
                "--features", "desktop", "--locked",
            ], cwd=ROOT, check=True)
            helper = str(ROOT / "scripts/macos")
            execute.assert_called_once_with(helper, [helper, "--run"])
            self.assertEqual(os.environ["RUST_BACKTRACE"], "full")

    def test_failed_check_never_launches_stale_bundle(self):
        with patch.object(PREVIEW.sys, "platform", "darwin"), \
             patch.object(PREVIEW.os, "getpgrp", return_value=os.getpid()), \
             patch.object(PREVIEW.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "cargo")), \
             patch.object(PREVIEW.os, "execv") as execute:
            with self.assertRaises(subprocess.CalledProcessError):
                PREVIEW.main([])
            execute.assert_not_called()

    def test_stop_targets_only_verified_owned_group(self):
        with patch.object(PREVIEW.os, "getpgid", return_value=12345), \
             patch.object(PREVIEW.os, "killpg") as kill:
            PREVIEW.stop("12345")
            kill.assert_called_once_with(12345, signal.SIGKILL)
        with patch.object(PREVIEW.os, "getpgid", return_value=54321), \
             patch.object(PREVIEW.os, "killpg") as kill:
            with self.assertRaises(ValueError):
                PREVIEW.stop("12345")
            kill.assert_not_called()
        for pid in ("0", "1", "-1", "not-a-pid"):
            with self.assertRaises(ValueError):
                PREVIEW.stop(pid)

    def test_stop_already_exited_group(self):
        with patch.object(PREVIEW.os, "getpgid", side_effect=ProcessLookupError):
            PREVIEW.stop("12345")

    @unittest.skipUnless(os.name == "posix", "Unix process-group lifecycle")
    def test_stop_terminates_descendants_without_touching_other_jobs(self):
        # The descendant inherits stdout. EOF therefore proves it was stopped,
        # not merely that the parent exited (without depending on zombie reaping).
        code = "import os, subprocess, sys, time; os.setpgid(0, 0); subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); print('ready', flush=True); time.sleep(60)"
        unrelated = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
        owned = subprocess.Popen([sys.executable, "-c", code], stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(owned.stdout.readline().strip(), "ready")
            subprocess.run([sys.executable, str(ROOT / "scripts/dev-preview"), "--stop", str(owned.pid)], check=True, timeout=5)
            output, _ = owned.communicate(timeout=5)
            self.assertEqual(output, "")
            self.assertEqual(owned.returncode, -signal.SIGKILL)
            self.assertIsNone(unrelated.poll())
        finally:
            if owned.poll() is None:
                os.killpg(owned.pid, signal.SIGKILL)
            owned.wait()
            unrelated.kill()
            unrelated.wait()

    def test_rejects_unexpected_arguments(self):
        with self.assertRaises(ValueError):
            PREVIEW.main(["--open"])


if __name__ == "__main__":
    unittest.main()
