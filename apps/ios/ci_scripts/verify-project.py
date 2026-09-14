#!/usr/bin/env python3
"""Check reproducible XcodeGen output without replacing Cloud's discoverable project."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--xcodegen", required=True)
    args = parser.parse_args()
    ios = Path(__file__).resolve().parents[1]
    generated = ["project.pbxproj", "xcshareddata/xcschemes/Den.xcscheme",
                 "project.xcworkspace/contents.xcworkspacedata"]
    lock = ios / "Den.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved"
    package_lock = ios / "Packages/DenAPI/Package.resolved"
    for path in [*(ios / "Den.xcodeproj" / item for item in generated), lock, package_lock]:
        if not path.is_file():
            raise SystemExit(f"Missing committed Cloud discovery input: {path.relative_to(ios)}")
        if os.environ.get("CI_XCODE_CLOUD") == "TRUE":
            subprocess.run(["git", "ls-files", "--error-unmatch", str(path)], cwd=ios, check=True, stdout=subprocess.DEVNULL)
    with tempfile.TemporaryDirectory(prefix="den-xcodegen-check-") as temporary:
        copy = Path(temporary) / "ios"
        shutil.copytree(ios, copy, ignore=shutil.ignore_patterns(
            "Den.xcodeproj", ".build", ".swiftpm", "DerivedData", ".DS_Store", "ci_scripts"))
        subprocess.run([args.xcodegen, "generate", "--spec", str(copy / "project.yml")], check=True)
        for name in generated:
            if (copy / "Den.xcodeproj" / name).read_bytes() != (ios / "Den.xcodeproj" / name).read_bytes():
                raise SystemExit(f"Generated {name} is stale. Regenerate with XcodeGen 2.45.4 and commit it.")
    def pins(path):
        value = json.loads(path.read_text())
        return {item["identity"]: item["state"] for item in value["pins"]}
    package_pins = pins(package_lock)
    project_pins = pins(lock)
    if any(project_pins.get(identity) != state for identity, state in package_pins.items()):
        raise SystemExit("The Xcode and DenAPI Swift package locks disagree. Resolve locally, review, and commit both locks.")
    print("Committed project, shared scheme and Swift package pins match.")
