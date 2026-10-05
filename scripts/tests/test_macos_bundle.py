import importlib.util
import json
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest import mock

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
            self.assertEqual(info["CFBundleIconFile"], "Dalan.icns")
            self.assertNotIn("CFBundleIconName", info)
            self.assertTrue((destination / "Contents/Resources").is_dir())
            self.assertEqual((destination / "Contents/Resources/Dalan.icns").read_bytes(),
                             (bundle.ROOT / "crates/app/assets/brand/Dalan.icns").read_bytes())
            self.assertEqual((destination / "Contents/Resources/dalan.png").read_bytes(),
                             (bundle.ROOT / "crates/app/assets/brand/dalan.png").read_bytes())
            notices = (destination / "Contents/Resources/THIRD_PARTY_NOTICES.md").read_text()
            self.assertIn("Lucide", notices)
            self.assertIn("Apache", notices)
            self.assertIn("Carbonfox - opaque", notices)
            self.assertIn("Copyright (c) 2024 Christian Angermann", notices)
            self.assertIn("Copyright (c) 2021 James Simpson", notices)
            self.assertIn("ISC License", (destination / "Contents/Resources/lucide-LICENSE.txt").read_text())
            self.assertEqual((destination / "Contents/Resources/dbx-Apache-2.0.txt").read_bytes(),
                             (bundle.ROOT / "licenses/dbx-Apache-2.0.txt").read_bytes())
            self.assertIn("DBX", notices)
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

    def test_icon_resources_have_valid_headers_and_original_layer_reference(self):
        brand = bundle.ROOT / "crates/app/assets/brand"
        png = (brand / "dalan.png").read_bytes()
        self.assertTrue(png.startswith(b"\x89PNG\r\n\x1a\n"))
        self.assertEqual(int.from_bytes(png[16:20], "big"), 1024)
        self.assertEqual(int.from_bytes(png[20:24], "big"), 1024)
        icns = (brand / "Dalan.icns").read_bytes()
        self.assertEqual(icns[:4], b"icns")
        self.assertEqual(int.from_bytes(icns[4:8], "big"), len(icns))
        package = json.loads((brand / "Dalan.icon/icon.json").read_text())
        for group in package["groups"]:
            for layer in group["layers"]:
                self.assertTrue((brand / "Dalan.icon/Assets" / layer["image-name"]).is_file())

    def test_compiled_icon_is_selected_only_with_asset_catalog(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "dalan"
            executable.write_text("fixture")
            compiled = root / "compiled"
            compiled.mkdir()
            destination = root / "Dalan.app"
            with self.assertRaises(ValueError):
                bundle.assemble_bundle(executable, destination, "0.1.0", "local.dalan.debug", compiled, "Dalan")
            self.assertFalse(destination.exists())
            (compiled / "Assets.car").write_bytes(b"fixture catalog, not a real compiled asset")
            bundle.assemble_bundle(executable, destination, "0.1.0", "local.dalan.debug", compiled, "Dalan")
            with (destination / "Contents/Info.plist").open("rb") as file:
                metadata = plistlib.load(file)
            self.assertEqual(metadata["CFBundleIconName"], "Dalan")
            self.assertEqual(metadata["CFBundleIconFile"], "Dalan.icns")
            self.assertEqual((destination / "Contents/Resources/Assets.car").read_bytes(), (compiled / "Assets.car").read_bytes())

    def test_icon_compiler_requires_tool_and_valid_output(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "compiled"
            with mock.patch.object(bundle.sys, "platform", "darwin"), mock.patch.object(bundle.subprocess, "run") as run:
                run.return_value = mock.Mock(returncode=1)
                with self.assertRaisesRegex(ValueError, "actool is unavailable"):
                    bundle.compile_icon_composer(output)
            with mock.patch.object(bundle.sys, "platform", "darwin"), mock.patch.object(bundle.subprocess, "run") as run:
                run.side_effect = [mock.Mock(returncode=0, stdout="/fixture/actool\n"), mock.Mock(returncode=0)]
                with self.assertRaisesRegex(ValueError, "did not generate"):
                    bundle.compile_icon_composer(output)
            def compiler(command, **kwargs):
                if command[:2] == ["xcrun", "--find"]:
                    return mock.Mock(returncode=0, stdout="/fixture/actool\n")
                (output / "Assets.car").write_bytes(b"test fixture")
                with (output / "icon-info.plist").open("wb") as file:
                    plistlib.dump({"CFBundleIconName": "Dalan"}, file)
                self.assertIn("--app-icon", command)
                self.assertIn(str(bundle.ROOT / "crates/app/assets/brand/Dalan.icon"), command)
                return mock.Mock(returncode=0)
            with mock.patch.object(bundle.sys, "platform", "darwin"), mock.patch.object(bundle.subprocess, "run", side_effect=compiler):
                self.assertEqual(bundle.compile_icon_composer(output), "Dalan")


if __name__ == "__main__":
    unittest.main()
