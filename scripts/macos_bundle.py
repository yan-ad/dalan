#!/usr/bin/env python3
"""Build a local macOS application bundle without extra Cargo tooling."""

import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def build_command(release, runtime_shaders):
    command = [
        "cargo", "build", "-p", "dalan-app", "--bin", "dalan", "--locked",
        "--features", "runtime-shaders" if runtime_shaders else "desktop",
        "--message-format", "json-render-diagnostics",
    ]
    if release:
        command.append("--release")
    return command


def executable_from_messages(output, package_id):
    executables = []
    for line in output.splitlines():
        message = json.loads(line)
        if (
            message.get("reason") == "compiler-artifact"
            and message.get("package_id") == package_id
            and message.get("target", {}).get("name") == "dalan"
            and "bin" in message.get("target", {}).get("kind", [])
            and message.get("executable")
        ):
            executables.append(Path(message["executable"]))
    if len(executables) != 1:
        raise ValueError("Cargo did not report exactly one dalan executable")
    return executables[0]


def bundle_info(version, identifier):
    # Pre-release suffixes are not valid CFBundleVersion components.
    numeric_version = version.split("-", 1)[0].split("+", 1)[0]
    return {
        "CFBundleName": "Dalan",
        "CFBundleDisplayName": "Dalan",
        "CFBundleExecutable": "Dalan",
        "CFBundleIdentifier": identifier,
        "CFBundlePackageType": "APPL",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleShortVersionString": numeric_version,
        "CFBundleVersion": numeric_version,
        "LSApplicationCategoryType": "public.app-category.developer-tools",
        "NSPrincipalClass": "NSApplication",
        "NSHighResolutionCapable": True,
        "NSSupportsAutomaticGraphicsSwitching": True,
    }


def assemble_bundle(executable, destination, version, identifier):
    if destination.name != "Dalan.app":
        raise ValueError("Bundle destination must be named Dalan.app")
    if destination.is_symlink():
        raise ValueError("Refusing to replace a symlinked app bundle")
    if destination.exists() and not (destination / "Contents/Info.plist").is_file():
        raise ValueError("Refusing to replace a directory that is not an app bundle")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".dalan-bundle-", dir=destination.parent) as staging:
        app = Path(staging) / "Dalan.app"
        contents = app / "Contents"
        (contents / "MacOS").mkdir(parents=True)
        (contents / "Resources").mkdir()
        shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", contents / "Resources/THIRD_PARTY_NOTICES.md")
        shutil.copy2(ROOT / "crates/app/assets/lucide-LICENSE.txt", contents / "Resources/lucide-LICENSE.txt")
        binary = contents / "MacOS/Dalan"
        shutil.copy2(executable, binary)
        binary.chmod(0o755)
        with (contents / "Info.plist").open("wb") as file:
            plistlib.dump(bundle_info(version, identifier), file)
        if destination.exists():
            shutil.rmtree(destination)
        app.rename(destination)


def main(argv=None):
    parser = argparse.ArgumentParser(description="Build Dalan.app for local macOS development.")
    parser.add_argument("--release", action="store_true", help="Optimized build; offline Metal shaders by default")
    shaders = parser.add_mutually_exclusive_group()
    shaders.add_argument("--runtime-shaders", dest="runtime_shaders", action="store_true", help="Compile shaders at runtime (debug default)")
    shaders.add_argument("--offline-shaders", dest="runtime_shaders", action="store_false", help="Use the Xcode Metal compiler (release default)")
    parser.set_defaults(runtime_shaders=None)
    launch = parser.add_mutually_exclusive_group()
    launch.add_argument("--open", action="store_true", help="Launch the bundle with macOS open")
    launch.add_argument("--run", action="store_true", help="Run the bundle executable in this terminal for logs/debugging")
    args = parser.parse_args(argv)
    if sys.platform != "darwin":
        parser.error("Dalan.app bundling currently requires macOS")
    runtime_shaders = args.runtime_shaders if args.runtime_shaders is not None else not args.release
    if not runtime_shaders:
        for tool in ("metal", "metallib"):
            result = subprocess.run(["xcrun", "--find", tool], capture_output=True, text=True)
            if result.returncode:
                parser.error(f"Missing Xcode {tool} compiler. Install/select full Xcode, or use --runtime-shaders for local development.")
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"], cwd=ROOT, text=True,
    ))
    package = next(package for package in metadata["packages"] if package["name"] == "dalan-app")
    result = subprocess.run(build_command(args.release, runtime_shaders), cwd=ROOT, stdout=subprocess.PIPE, text=True, check=True)
    executable = executable_from_messages(result.stdout, package["id"])
    destination = executable.parent / "bundles/Dalan.app"
    identifier = "local.dalan.release" if args.release else "local.dalan.debug"
    assemble_bundle(executable, destination, package["version"], identifier)
    subprocess.run(["plutil", "-lint", str(destination / "Contents/Info.plist")], check=True)
    subprocess.run(["codesign", "--force", "--sign", "-", "--identifier", identifier, "--timestamp=none", str(destination)], check=True)
    subprocess.run(["codesign", "--verify", "--strict", "--verbose=2", str(destination)], check=True)
    print(f"Built {destination}", flush=True)
    print("Local ad-hoc signed build, not notarized for distribution. Quit an older instance before relaunching.", flush=True)
    if args.open:
        subprocess.run(["open", str(destination)], check=True)
    elif args.run:
        os.execv(str(destination / "Contents/MacOS/Dalan"), [str(destination / "Contents/MacOS/Dalan")])


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError, StopIteration) as error:
        print(f"Bundle build failed: {error}", file=sys.stderr)
        sys.exit(1)
