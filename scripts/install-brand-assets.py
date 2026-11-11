#!/usr/bin/env python3
"""Copy approved brand exports into the apps or the website source tree."""
import argparse
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BRAND = ROOT / "assets/brand"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target", choices=("apps", "website"), default="apps")
args = parser.parse_args()
if args.target == "website":
    destination = ROOT / "website/site-src/content/extra_files/assets"
    destination.mkdir(parents=True, exist_ok=True)
    for source, name in (
        ("logos/mark-light.svg", "ichoi-mark.svg"),
        ("logos/mark-dark.svg", "ichoi-mark-dark.svg"),
        ("web/favicon.svg", "favicon.svg"),
        ("web/favicon.ico", "favicon.ico"),
        ("web/apple-touch-icon.png", "apple-touch-icon.png"),
        ("social/share-dark-1200x630.png", "ichoi-share.png"),
    ):
        shutil.copyfile(BRAND / source, destination / name)
    raise SystemExit(0)

for source, destination in (
    (BRAND / "apps/apple/AppIcon.appiconset", ROOT / "ios/Ichoi/Assets.xcassets/AppIcon.appiconset"),
    (BRAND / "apps/android/res", ROOT / "android/app/src/main/res"),
):
    shutil.copytree(source, destination, dirs_exist_ok=True)

web = ROOT / "server/web/themes/default/public"
shutil.copytree(BRAND / "web", web / "brand", dirs_exist_ok=True)
shutil.copy(BRAND / "ichoi-mark.svg", web / "brand/ichoi-mark.svg")
manifest = json.loads((web / "manifest.webmanifest").read_text())
icons = json.loads((BRAND / "web/manifest-icons.json").read_text())
manifest["icons"] = [{**icon, "src": "/brand" + icon["src"]} for icon in icons["icons"]]
manifest["background_color"] = "#16130e"
manifest["theme_color"] = "#16130e"
(web / "manifest.webmanifest").write_text(json.dumps(manifest, indent=2) + "\n")

image_set = ROOT / "ios/Ichoi/Assets.xcassets/BrandMark.imageset"
image_set.mkdir(exist_ok=True)
shutil.copy(BRAND / "web/icon-192.png", image_set / "brand.png")
(image_set / "Contents.json").write_text(json.dumps({
    "images": [{"filename": "brand.png", "idiom": "universal"}],
    "info": {"author": "xcode", "version": 1},
}, indent=2) + "\n")
