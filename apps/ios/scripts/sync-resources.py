#!/usr/bin/env python3
"""Copy shared themes, render repo-native branding, and verify pinned bundled fonts.

Run normally after changing the shared theme JSON or Doorway SVG. Rendering needs
librsvg's rsvg-convert. --check is read-only, offline, and needs only Python 3.
--fetch-fonts restores the pinned, hash-checked binaries and licenses from Google
Fonts' official binary distribution; it never follows a moving branch.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[3]
RESOURCES = ROOT / "apps/ios/Den/Resources"
FONTS = RESOURCES / "Fonts"
ASSETS = RESOURCES / "Assets.xcassets"
THEMES = ROOT / "crates/den-core/src/themes.json"
ICON = ROOT / "apps/desktop/icon.svg"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def verify_fonts(fetch):
    manifest = json.loads((FONTS / "manifest.json").read_text())
    base = "https://raw.githubusercontent.com/google/fonts/" + manifest["distribution_commit"]
    for family in manifest["families"]:
        slug = Path(family["license"]).parent
        files = [(font["file"], font["sha256"]) for font in family["fonts"]]
        files += [(family["license"], family["license_sha256"]),
                  (str(slug / "METADATA.pb"), family["metadata_sha256"])]
        for relative, expected in files:
            path = FONTS / relative
            if fetch and (not path.exists() or digest(path.read_bytes()) != expected):
                remote = family["distribution_path"] + "/" + path.name
                url = base + "/" + urllib.parse.quote(remote)
                data = urllib.request.urlopen(url, timeout=60).read()
                if digest(data) != expected:
                    raise SystemExit(f"Downloaded hash mismatch: {relative}")
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            if not path.exists() or digest(path.read_bytes()) != expected:
                raise SystemExit(f"Missing or changed font resource: {relative}. Run with --fetch-fonts.")
    expected_names = {f["family"]: f["default_postscript_name"] for f in manifest["families"]}
    used_names = {name for theme in json.loads(THEMES.read_text()) for name in theme["fonts"].values()}
    if set(expected_names) != used_names:
        raise SystemExit("Bundled font families no longer match the shared built-ins. Update the manifest.")
    return expected_names, sum(len(f["fonts"]) for f in manifest["families"])


def png(path, svg, size, background=None):
    command = ["rsvg-convert", "--width", str(size), "--height", str(size)]
    if background:
        command += ["--background-color", background]
    data = subprocess.run(command, input=svg, check=True, stdout=subprocess.PIPE).stdout
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def render_assets():
    if not shutil.which("rsvg-convert"):
        raise SystemExit("Install librsvg to regenerate branding, or use --check to verify committed assets.")
    info = {"author": "xcode", "version": 1}
    write_json(ASSETS / "Contents.json", {"info": info})
    family = json.loads(THEMES.read_text())[0]
    dark, light = family["dark"], family["light"]
    icon = ASSETS / "AppIcon.appiconset"
    png(icon / "AppIcon.png", ICON.read_bytes(), 1024, dark["bg"])
    write_json(icon / "Contents.json", {"images": [{"filename": "AppIcon.png", "idiom": "universal",
               "platform": "ios", "size": "1024x1024"}], "info": info})

    def srgb(hex_color):
        return {"color-space": "srgb", "components": {
            "alpha": "1.000", **{role: f"0x{hex_color[i:i+2].upper()}"
                                  for role, i in [("red", 1), ("green", 3), ("blue", 5)]}}}
    appearance = [{"appearance": "luminosity", "value": "dark"}]
    write_json(ASSETS / "LaunchBackground.colorset/Contents.json", {"colors": [
        {"idiom": "universal", "color": srgb(light["bg"])},
        {"idiom": "universal", "appearances": appearance, "color": srgb(dark["bg"])}], "info": info})
    images = []
    for mode, colors in [("light", light), ("dark", dark)]:
        tree = ET.fromstring(ICON.read_bytes())
        namespace = "{http://www.w3.org/2000/svg}"
        tree.remove(tree.find(namespace + "rect"))
        for element in tree.iter():
            if element.attrib.get("fill") == dark["ink"]:
                element.set("fill", colors["ink"])
            elif element.attrib.get("fill") == dark["accent"]:
                element.set("fill", colors["accent"])
        for scale in [1, 2, 3]:
            filename = f"LaunchMark-{mode}@{scale}x.png"
            png(ASSETS / "LaunchMark.imageset" / filename, ET.tostring(tree), 128 * scale)
            image = {"filename": filename, "idiom": "universal", "scale": f"{scale}x"}
            if mode == "dark":
                image["appearances"] = appearance
            images.append(image)
    write_json(ASSETS / "LaunchMark.imageset/Contents.json", {"images": images, "info": info})
    write_json(RESOURCES / "branding-manifest.json", {
        "sources": {str(path.relative_to(ROOT)): digest(path.read_bytes()) for path in [ICON, THEMES]},
        "assets": {str(path.relative_to(ASSETS)): digest(path.read_bytes())
                   for path in sorted(ASSETS.rglob("*")) if path.is_file()},
    })


def verify_assets():
    manifest = json.loads((RESOURCES / "branding-manifest.json").read_text())
    for base, entries in [(ROOT, manifest["sources"]), (ASSETS, manifest["assets"])]:
        for relative, expected in entries.items():
            path = base / relative
            if not path.exists() or digest(path.read_bytes()) != expected:
                raise SystemExit(f"Branding is stale or changed: {relative}. Run sync-resources.py.")
    icon = (ASSETS / "AppIcon.appiconset/AppIcon.png").read_bytes()
    width, height = struct.unpack_from(">II", icon, 16)
    if (width, height) != (1024, 1024) or icon[25] not in (0, 2):
        raise SystemExit("AppIcon must be an opaque 1024x1024 PNG.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Verify committed resources without writes or rendering.")
    parser.add_argument("--fetch-fonts", action="store_true", help="Restore pinned font resources over HTTPS.")
    args = parser.parse_args()
    if args.check and args.fetch_fonts:
        parser.error("--check cannot download resources")
    names, count = verify_fonts(args.fetch_fonts)
    names_path = RESOURCES / "font-families.json"
    themes_path = RESOURCES / "themes.json"
    if not args.check:
        shutil.copyfile(THEMES, themes_path)
        write_json(names_path, names)
        render_assets()
    if not themes_path.exists() or themes_path.read_bytes() != THEMES.read_bytes():
        raise SystemExit("Bundled themes differ from den-core. Run sync-resources.py.")
    if json.loads(names_path.read_text()) != names:
        raise SystemExit("The bundled PostScript font map differs from its manifest.")
    verify_assets()
    families = len(json.loads(THEMES.read_text()))
    print(f"Verified {families} shared families, {len(names)} font families, {count} font binaries and Doorway assets.")


if __name__ == "__main__":
    main()
