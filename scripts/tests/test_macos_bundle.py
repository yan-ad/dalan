import importlib.util
import json
from pathlib import Path
import plistlib
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("macos_bundle", Path(__file__).resolve().parents[1] / "macos_bundle.py")
bundle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bundle)


class BundleTests(unittest.TestCase):
    def test_build_modes_keep_lockfile_and_shaders_explicit(self):
        debug = bundle.build_command(False, True)
        release = bundle.build_command(True, False)
        self.assertIn("--locked", debug)
        self.assertIn("runtime-shaders", debug)
        self.assertNotIn("--release", debug)
        self.assertIn("--release", release)
        self.assertIn("desktop", release)
        self.assertNotIn("runtime-shaders", release)

    def test_only_matching_executable_is_selected(self):
        artifact = {"reason": "compiler-artifact", "package_id": "dalan", "target": {"name": "dalan", "kind": ["bin"]}, "executable": "/tmp/custom-target/debug/dalan"}
        other = {**artifact, "package_id": "other"}
        output = "\n".join(json.dumps(message) for message in [other, artifact, {"reason": "build-finished", "success": True}])
        self.assertEqual(bundle.executable_from_messages(output, "dalan"), Path(artifact["executable"]))
        with self.assertRaises(ValueError):
            bundle.executable_from_messages(json.dumps(other), "dalan")
        with self.assertRaises(ValueError):
            bundle.executable_from_messages("\n".join([json.dumps(artifact)] * 2), "dalan")

    def test_bundle_layout_metadata_and_rebuild(self):
        with tempfile.TemporaryDirectory(prefix="dalan bundle test ") as directory:
            root = Path(directory)
            executable = root / "dalan"
            executable.write_bytes(b"first build")
            destination = root / "bundles/Dalan.app"
            bundle.assemble_bundle(executable, destination, "0.1.0-dev", "local.dalan.debug")
            with (destination / "Contents/Info.plist").open("rb") as file:
                info = plistlib.load(file)
            self.assertEqual(info["CFBundleDisplayName"], "Dalan")
            self.assertEqual(info["CFBundleExecutable"], "Dalan")
            self.assertEqual(info["CFBundlePackageType"], "APPL")
            self.assertEqual(info["CFBundleVersion"], "0.1.0")
            self.assertTrue((destination / "Contents/Resources").is_dir())
            notices = (destination / "Contents/Resources/THIRD_PARTY_NOTICES.md").read_text()
            self.assertIn("Lucide", notices)
            self.assertIn("Apache", notices)
            self.assertIn("Carbonfox - opaque", notices)
            self.assertIn("Copyright (c) 2024 Christian Angermann", notices)
            self.assertIn("Copyright (c) 2021 James Simpson", notices)
            self.assertIn("ISC License", (destination / "Contents/Resources/lucide-LICENSE.txt").read_text())
            binary = destination / "Contents/MacOS/Dalan"
            self.assertEqual(binary.read_bytes(), b"first build")
            self.assertEqual(binary.stat().st_mode & 0o777, 0o755)
            (destination / "stale").write_text("remove on rebuild")
            executable.write_bytes(b"second build")
            bundle.assemble_bundle(executable, destination, "0.1.0", "local.dalan.debug")
            self.assertEqual(binary.read_bytes(), b"second build")
            self.assertFalse((destination / "stale").exists())

    def test_bundle_refuses_unrelated_directory_or_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "dalan"
            executable.write_text("fixture")
            destination = root / "Dalan.app"
            destination.mkdir()
            with self.assertRaises(ValueError):
                bundle.assemble_bundle(executable, destination, "0.1.0", "local.dalan.debug")
            destination.rmdir()
            destination.symlink_to(root, target_is_directory=True)
            with self.assertRaises(ValueError):
                bundle.assemble_bundle(executable, destination, "0.1.0", "local.dalan.debug")
            with self.assertRaises(ValueError):
                bundle.assemble_bundle(executable, root / "Other.app", "0.1.0", "local.dalan.debug")


if __name__ == "__main__":
    unittest.main()
