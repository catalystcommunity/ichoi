#!/usr/bin/env python3
"""Build the Ichoi brand kit from vector sources. No network access is required."""

from pathlib import Path
import argparse
import hashlib
import html
import json
import shutil
import struct
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parent
SVG = "http://www.w3.org/2000/svg"
ET.register_namespace("", SVG)
INK, PAPER, TEAL, AMBER = "#101417", "#f3ecdd", "#37d6c6", "#ffb454"
PALETTES = {
    "dark": (INK, PAPER, TEAL, AMBER),
    "light": (PAPER, INK, "#087f78", "#a85c00"),
}
FILES = {}
MARK = ET.parse(ROOT / "ichoi-mark.svg").getroot()
GEOMETRY = "".join(ET.tostring(n, encoding="unicode") for n in MARK
                   if n.tag.rsplit("}", 1)[-1] not in ("title", "desc"))


def run(*args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def write(path, data, use="", width=None, height=None):
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, bytes):
        target.write_bytes(data)
    else:
        target.write_text(data)
    FILES[path] = {"use": use}
    if width is not None:
        FILES[path].update(width=width, height=height)
    return target


def document(width, height, body, title="Ichoi"):
    return (f'<svg xmlns="{SVG}" width="{width}" height="{height}" '
            f'viewBox="0 0 {width} {height}" role="img" aria-labelledby="title">'
            f'<title id="title">{html.escape(title)}</title>{body}</svg>\n')


def rect(width, height, color, radius=0):
    return f'<rect width="{width}" height="{height}" rx="{radius}" fill="{color}"/>'


def mark(x=0, y=0, size=512, palette="dark", mono=None):
    _, _, teal, amber = PALETTES[palette]
    shapes = GEOMETRY.replace(TEAL, mono or teal).replace(AMBER, mono or amber)
    return f'<g transform="translate({x} {y}) scale({size / 512})">{shapes}</g>'


def lettering(name, x, y, width, color):
    root = ET.parse(ROOT / "source" / f"{name}.svg").getroot()
    box = [float(v) for v in root.attrib["viewBox"].split()]
    height = width * box[3] / box[2]
    root.attrib.update(x=str(x), y=str(y), width=str(width), height=str(height), color=color)
    return ET.tostring(root, encoding="unicode")


def pair(path, width, height, body, use, png_width=None, mode="rgba"):
    svg = write(path + ".svg", document(width, height, body, use), use, width, height)
    png_width = png_width or width
    png_height = round(png_width * height / width)
    png(path + ".png", svg, png_width, png_height, use, mode)


def png(path, source, width, height, use, mode="rgba"):
    with tempfile.TemporaryDirectory(prefix="ichoi-brand-") as temporary:
        raw = Path(temporary) / "render.png"
        run("rsvg-convert", "--width", width, "--height", height, source, "-o", raw)
        target = ROOT / path
        target.parent.mkdir(parents=True, exist_ok=True)
        args = ["magick", raw, "-strip", "-colorspace", "sRGB", "-depth", "8"]
        if mode == "rgb":
            args += ["-background", INK, "-alpha", "remove", "-alpha", "off"]
        args += ["-define", "png:exclude-chunks=date,time", f"PNG{'24' if mode == 'rgb' else '32'}:{target}"]
        run(*args)
    FILES[path] = {"use": use, "width": width, "height": height, "mode": mode}


def lockup(palette, stacked=False, mono=None):
    color = mono or PALETTES[palette][1]
    if stacked:
        return mark(128, 0, 512, palette, mono) + lettering("name", 202, 490, 364, color)
    return mark(0, 0, 320, palette, mono) + lettering("name", 338, 84, 450, color)


def backdrop(width, height, palette):
    bg, _, teal, _ = PALETTES[palette]
    body = rect(width, height, bg)
    # Faint record grooves stay outside the main text area.
    for radius in (height * .52, height * .72, height * .92):
        body += (f'<circle cx="{width * .92}" cy="{height * .58}" r="{radius}" '
                 f'fill="none" stroke="{teal}" stroke-opacity=".08" stroke-width="2"/>')
    return body


def social(width, height, palette="dark", layout="landscape"):
    _, fg, _, amber = PALETTES[palette]
    body = backdrop(width, height, palette)
    if layout == "landscape":
        scale = min(width / 1200, height / 630)
        x, y = (width - 1200 * scale) / 2, (height - 630 * scale) / 2
        content = lettering("name", 86, 88, 184, fg)
        content += lettering("headline-1", 80, 230, 570, fg)
        content += lettering("headline-2", 80, 326, 586, amber)
        content += lettering("description", 84, 485, 495, fg)
        content += mark(735, 133, 412, palette)
        body += f'<g transform="translate({x} {y}) scale({scale})">{content}</g>'
    elif layout in ("square", "portrait", "story"):
        top = {"square": 72, "portrait": 160, "story": 365}[layout]
        body += lettering("name", 422, top, 236, fg)
        body += mark(275, top + 125, 530, palette)
        body += lettering("headline-1", 210, top + 680, 660, fg)
        body += lettering("headline-2", 200, top + 785, 680, amber)
        if layout != "square":
            body += lettering("description", 257, top + 955, 566, fg)
    elif layout == "header":
        # Keep the lower-left region clear for profile images.
        body += lettering("name", width * .27, height * .22, height * .45, fg)
        body += lettering("description", width * .27, height * .57, width * .43, fg)
        body += mark(width * .75, height * .12, height * .76, palette)
    elif layout == "channel":
        # Keep the complete lockup in the central 1546 by 423 area.
        body += mark(600, 545, 350, palette)
        body += lettering("name", 1000, 580, 330, fg)
        body += lettering("description", 1004, 750, 700, fg)
    return body


def android_vector(mono=False):
    """Convert the source shapes to Android paths without changing their geometry."""
    ns = "http://schemas.android.com/apk/res/android"
    root = ET.Element("vector", {"xmlns:android": ns, "android:width": "108dp",
                                "android:height": "108dp", "android:viewportWidth": "108",
                                "android:viewportHeight": "108"})
    # The 392-unit outer diameter becomes 64 dp, within the 66 dp safe circle.
    scale = 64 / 392
    offset = 54 - 256 * scale
    group = ET.SubElement(root, "group", {"android:scaleX": str(scale), "android:scaleY": str(scale),
                                         "android:translateX": str(offset), "android:translateY": str(offset)})
    for node in MARK:
        tag = node.tag.rsplit("}", 1)[-1]
        if tag == "circle":
            cx, cy, radius = (float(node.attrib[key]) for key in ("cx", "cy", "r"))
            ET.SubElement(group, "path", {"android:fillColor": "#00000000",
                "android:strokeColor": "#ffffff" if mono else node.attrib["stroke"],
                "android:strokeWidth": node.attrib["stroke-width"],
                "android:pathData": f"M{cx-radius},{cy}a{radius},{radius} 0,1 0,{2*radius},0a{radius},{radius} 0,1 0,{-2*radius},0"})
        elif tag == "g":
            for shape in node:
                target = group
                if shape.tag.endswith("ellipse"):
                    cx, cy, rx, ry = (float(shape.attrib[key]) for key in ("cx", "cy", "rx", "ry"))
                    angle, px, py = shape.attrib["transform"].removeprefix("rotate(").removesuffix(")").split()
                    target = ET.SubElement(group, "group", {"android:rotation": angle,
                        "android:pivotX": px, "android:pivotY": py})
                    path = f"M{cx-rx},{cy}a{rx},{ry} 0,1 0,{2*rx},0a{rx},{ry} 0,1 0,{-2*rx},0"
                else:
                    path = shape.attrib["d"]
                ET.SubElement(target, "path", {"android:fillColor": "#ffffff" if mono else node.attrib["fill"],
                                                "android:pathData": path})
    ET.indent(root)
    return ET.tostring(root, encoding="unicode") + "\n"


def build():
    # Preserve the two original export names.
    source = ROOT / "ichoi-mark.svg"
    png("ichoi-mark.png", source, 2048, 2048, "Transparent color mark")
    pair("ichoi-logo", 512, 512, rect(512, 512, INK, 112) + mark(),
         "Rounded logo for general use", 2048)
    for theme in PALETTES:
        pair(f"logos/mark-{theme}", 512, 512, mark(palette=theme),
             f"Transparent mark for {theme} backgrounds", 2048)
        pair(f"logos/horizontal-{theme}", 840, 320, lockup(theme),
             f"Horizontal logo for {theme} backgrounds", 2520)
        pair(f"logos/stacked-{theme}", 768, 680, lockup(theme, True),
             f"Stacked logo for {theme} backgrounds", 1536)
        bg = PALETTES[theme][0]
        pair(f"social/avatar-{theme}-1024", 1024, 1024,
             rect(1024, 1024, bg) + mark(32, 32, 960, theme),
             f"Social avatar, {theme} background", mode="rgb")
        png(f"social/avatar-{theme}-512.png", ROOT / f"social/avatar-{theme}-1024.svg",
            512, 512, "Social avatar", "rgb")
        pair(f"social/share-{theme}-1200x630", 1200, 630, social(1200, 630, theme),
             "Link preview card", mode="rgb")
    for label, color in (("black", "#000000"), ("white", "#ffffff")):
        pair(f"logos/mark-{label}", 512, 512, mark(mono=color), "Single-color mark", 2048)
        pair(f"logos/horizontal-{label}", 840, 320, lockup("dark", mono=color), "Single-color horizontal logo", 2520)

    for label, width, height, layout in (
        ("wide", 1200, 600, "landscape"),
        ("repository", 1280, 640, "landscape"),
        ("square", 1080, 1080, "square"),
        ("portrait", 1080, 1350, "portrait"),
        ("story", 1080, 1920, "story"),
        ("header", 1500, 500, "header"),
        ("professional-header", 1584, 396, "header"),
        ("channel", 2560, 1440, "channel"),
    ):
        pair(f"social/{label}-{width}x{height}", width, height,
             social(width, height, layout=layout), f"{label.title()} social artwork", mode="rgb")

    # Store and launcher icons have square backgrounds. The platform applies its mask.
    pair("apps/app-icon", 1024, 1024, rect(1024, 1024, INK) + mark(size=1024),
         "Square app icon; platform applies the corner mask", mode="rgb")
    app = ROOT / "apps/app-icon.svg"
    png("stores/google-play-icon-512.png", app, 512, 512, "Google Play listing icon", "rgba")
    feature = rect(1024, 500, "#174b5b")
    feature += lettering("name", 98, 70, 160, PAPER)
    feature += lettering("headline-1", 94, 182, 548, PAPER)
    feature += lettering("headline-2", 94, 276, 564, AMBER)
    # An abstract row of audio levels extends the identity without repeating the app icon.
    for i, bar_height in enumerate((55, 105, 165, 110, 65)):
        feature += f'<rect x="{748+i*28}" y="{250-bar_height/2}" width="12" height="{bar_height}" rx="6" fill="{TEAL}"/>'
    pair("stores/google-play-feature-1024x500", 1024, 500, feature,
         "Ichoi. Your music. Every room. Audio level bars on a teal background.", mode="rgb")

    ios = "apps/apple/AppIcon.appiconset"
    png(f"{ios}/AppIcon.png", app, 1024, 1024, "Apple default and dark app icon", "rgb")
    pair("apps/apple/tinted", 1024, 1024, rect(1024, 1024, "#101010") + mark(size=1024, mono="#ededed"),
         "Apple tinted source", mode="rgb")
    png(f"{ios}/AppIcon-tinted.png", ROOT / "apps/apple/tinted.svg", 1024, 1024, "Apple tinted app icon", "rgb")
    images = [{"filename": "AppIcon.png", "idiom": "universal", "platform": "ios", "size": "1024x1024"}]
    for appearance, filename in (("dark", "AppIcon.png"), ("tinted", "AppIcon-tinted.png")):
        images.append({"filename": filename, "idiom": "universal", "platform": "ios", "size": "1024x1024",
                       "appearances": [{"appearance": "luminosity", "value": appearance}]})
    write(f"{ios}/Contents.json", json.dumps({"images": images, "info": {"author": "xcode", "version": 1}}, indent=2) + "\n", "Import this set into an Xcode asset catalog")
    write("apps/apple/layers/01-background.svg", document(1024, 1024, rect(1024, 1024, INK)), "Icon Composer background reference")
    for index, node in enumerate(n for n in MARK if n.tag.endswith("circle") or n.tag.endswith("g")):
        write(f"apps/apple/layers/0{index+2}-{'ring' if index == 0 else 'notes'}.svg",
              document(1024, 1024, f'<g transform="scale(2)">{ET.tostring(node, encoding="unicode")}</g>'),
              "Transparent Icon Composer foreground layer")

    for size in (16, 24, 32, 48, 64, 128, 256, 512, 1024):
        png(f"apps/desktop/png/ichoi-{size}.png", ROOT / "ichoi-logo.svg", size, size, "Desktop icon")
    ico_sources = [ROOT / f"apps/desktop/png/ichoi-{s}.png" for s in (16, 24, 32, 48, 64, 128, 256)]
    run("magick", *ico_sources, ROOT / "apps/desktop/ichoi.ico")
    FILES["apps/desktop/ichoi.ico"] = {"use": "Windows icon with seven sizes"}
    # ICNS supports PNG payloads for these modern icon types.
    chunks = []
    for code, size in ((b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024)):
        data = (ROOT / f"apps/desktop/png/ichoi-{size}.png").read_bytes()
        chunks.append(code + struct.pack(">I", 8 + len(data)) + data)
    data = b"".join(chunks)
    write("apps/desktop/ichoi.icns", b"icns" + struct.pack(">I", len(data) + 8) + data, "macOS desktop icon")

    for mono in (False, True):
        name = "monochrome" if mono else "foreground"
        write(f"apps/android/res/drawable/ic_launcher_{name}.xml", android_vector(mono), "Android adaptive icon layer")
        size = 512 * 64 / 392
        pair(f"apps/android/{name}", 108, 108, mark((108-size)/2, (108-size)/2, size, mono="#ffffff" if mono else None),
             "Android 108 dp layer; artwork fits the central 64 dp circle", 432)
    write("apps/android/res/values/ic_launcher_background.xml", f'<resources><color name="ic_launcher_background">{INK}</color></resources>\n', "Android background color")
    for version in (26, 33):
        mono = '\n  <monochrome android:drawable="@drawable/ic_launcher_monochrome"/>' if version == 33 else ""
        write(f"apps/android/res/mipmap-anydpi-v{version}/ic_launcher.xml",
              '<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">\n'
              '  <background android:drawable="@color/ic_launcher_background"/>\n'
              '  <foreground android:drawable="@drawable/ic_launcher_foreground"/>' + mono + '\n</adaptive-icon>\n', "Android adaptive launcher icon")
    for density, size in (("mdpi", 48), ("hdpi", 72), ("xhdpi", 96), ("xxhdpi", 144), ("xxxhdpi", 192)):
        png(f"apps/android/res/mipmap-{density}/ic_launcher.png", ROOT / "ichoi-logo.svg", size, size, "Android legacy launcher icon")

    write("web/favicon.svg", (ROOT / "ichoi-logo.svg").read_text(), "SVG favicon", 512, 512)
    for size in (16, 32, 48):
        png(f"web/favicon-{size}.png", ROOT / "ichoi-logo.svg", size, size, "PNG favicon")
    run("magick", *[ROOT / f"web/favicon-{s}.png" for s in (16, 32, 48)], ROOT / "web/favicon.ico")
    FILES["web/favicon.ico"] = {"use": "Browser favicon with 16, 32, and 48 pixel sizes"}
    png("web/apple-touch-icon.png", app, 180, 180, "Apple touch icon", "rgb")
    for size in (192, 512):
        png(f"web/icon-{size}.png", ROOT / "ichoi-logo.svg", size, size, "PWA icon")
        png(f"web/maskable-{size}.png", app, size, size, "PWA maskable icon; artwork fits the central 80% diameter", "rgb")
    write("web/pinned-tab.svg", document(512, 512, mark(mono="#000000")), "Single-color pinned tab icon")
    icons = []
    for purpose, name in (("any", "icon"), ("maskable", "maskable")):
        for size in (192, 512):
            icons.append({"src": f"/{name}-{size}.png", "sizes": f"{size}x{size}", "type": "image/png", "purpose": purpose})
    write("web/manifest-icons.json", json.dumps({"icons": icons}, indent=2) + "\n", "Merge these entries into the app manifest")

    preview = rect(1120, 320, "#293137")
    preview += '<defs><clipPath id="apple-mask"><rect width="240" height="240" rx="54"/></clipPath>'
    preview += '<clipPath id="android-circle"><circle cx="54" cy="54" r="36"/></clipPath>'
    preview += '<clipPath id="android-square"><rect x="18" y="18" width="72" height="72" rx="16"/></clipPath></defs>'
    preview += '<g transform="translate(20 40)" clip-path="url(#apple-mask)">' + rect(240, 240, INK) + mark(size=240) + '</g>'
    size = 512 * 64 / 392
    for x, mask, themed in ((300, "android-circle", False), (580, "android-square", False), (860, "android-circle", True)):
        preview += f'<svg x="{x}" y="40" width="240" height="240" viewBox="18 18 72 72"><g clip-path="url(#{mask})">'
        preview += rect(108, 108, "#d3e8e2" if themed else INK)
        preview += mark((108-size)/2, (108-size)/2, size, mono="#174b5b" if themed else None)
        preview += '</g></svg>'
    pair("previews/app-masks", 1120, 320, preview,
         "Mask previews: Apple, Android circle, Android rounded square, Android themed circle", mode="rgb")


def finish():
    # Use relative links so the catalog also works after the ZIP is extracted.
    cards = []
    for path, meta in FILES.items():
        if not path.endswith(".png") or path.startswith("apps/") or path.startswith("web/"):
            continue
        bg = "#20262b" if "-dark" in path or "-white" in path or path == "ichoi-mark.png" else "#eeeae1"
        svg = path[:-4] + ".svg"
        links = f'<a href="{path}">PNG</a>'
        if svg in FILES:
            links += f' · <a href="{svg}">SVG</a>'
        cards.append(f'<article><div class="preview" style="background:{bg}"><img src="{path}" alt="{html.escape(meta["use"])}"></div>'
                     f'<p>{html.escape(path)}</p><small>{meta["width"]} × {meta["height"]}</small><p>{links}</p></article>')
    write("index.html", '<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">'
          '<title>Ichoi brand assets</title><style>body{margin:0;padding:40px;background:#101417;color:#f3ecdd;font:16px system-ui}h1{font-size:40px}'
          'main{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:24px}article{padding:16px;background:#20262b;border-radius:12px}'
          '.preview{height:260px;display:grid;place-items:center;border-radius:6px}.preview img{max-width:100%;max-height:240px}'
          'p{overflow-wrap:anywhere}a{color:#37d6c6}small{color:#b0b8bc}</style><h1>Ichoi brand assets</h1>'
          '<p>Two notes. One circle. <a href="README.md">File guide</a> · <a href="ichoi-brand-kit.zip">Download ZIP</a></p>'
          '<main>' + "".join(cards) + '</main></html>\n', "Visual asset catalog")
    run("magick", "montage", *[ROOT / path for path in (
        "social/share-dark-1200x630.png", "social/share-light-1200x630.png",
        "social/square-1080x1080.png", "social/portrait-1080x1350.png",
        "social/story-1080x1920.png", "stores/google-play-feature-1024x500.png",
        "social/header-1500x500.png", "previews/app-masks.png")],
        "-background", "#293137", "-geometry", "480x300+16+16", "-tile", "2x4",
        "-strip", "-define", "png:exclude-chunks=date,time", f"PNG24:{ROOT / 'preview.png'}")
    FILES["preview.png"] = {"use": "Visual overview of the brand kit", "width": 1024, "height": 1328, "mode": "rgb"}
    for path, meta in FILES.items():
        meta["sha256"] = hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
    write("manifest.json", json.dumps({"assets": FILES}, indent=2) + "\n", "Asset dimensions, uses, and checksums")
    # Fixed timestamps and sorted entries make the archive reproducible.
    paths = set(FILES) | {"ichoi-mark.svg", "build.py", "export.sh", "README.md"}
    paths.update(str(p.relative_to(ROOT)) for p in (ROOT / "source").iterdir() if p.is_file())
    with zipfile.ZipFile(ROOT / "ichoi-brand-kit.zip", "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(paths):
            if not (ROOT / path).is_file():
                continue
            info = zipfile.ZipInfo("ichoi-brand-kit/" + path, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, (ROOT / path).read_bytes())


def check():
    assets = json.loads((ROOT / "manifest.json").read_text())["assets"]
    count = 0
    for path, meta in assets.items():
        data = (ROOT / path).read_bytes()
        assert hashlib.sha256(data).hexdigest() == meta["sha256"], path
        if path.endswith(".png"):
            width, height, depth, mode = struct.unpack(">IIBB", data[16:26])
            assert (width, height, depth) == (meta["width"], meta["height"], 8), path
            assert mode == (2 if meta["mode"] == "rgb" else 6), path
        elif path.endswith((".svg", ".xml")):
            root = ET.fromstring(data)
            if path.endswith(".svg"):
                assert not list(root.iter(f"{{{SVG}}}text")), path
                assert not list(root.iter(f"{{{SVG}}}image")), path
        count += 1
    contents = json.loads((ROOT / "apps/apple/AppIcon.appiconset/Contents.json").read_text())
    for image in contents["images"]:
        assert (ROOT / "apps/apple/AppIcon.appiconset" / image["filename"]).is_file()
    assert (ROOT / "stores/google-play-icon-512.png").stat().st_size < 1024 * 1024
    # Validate the safe circle directly from the source ring dimensions.
    ring = MARK.find(f"{{{SVG}}}circle")
    radius = float(ring.attrib["r"]) + float(ring.attrib["stroke-width"]) / 2
    assert radius <= .4 * 512, "PWA maskable safe area"
    assert radius * 64 / 392 <= 33, "Android adaptive safe area"
    with zipfile.ZipFile(ROOT / "ichoi-brand-kit.zip") as archive:
        assert archive.testzip() is None
    print(f"Checked {count} assets: dimensions, PNG modes, XML, checksums, safe areas, and ZIP.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Check existing exports without rebuilding")
    args = parser.parse_args()
    if not args.check:
        for tool in ("rsvg-convert", "magick"):
            if not shutil.which(tool):
                parser.error(f"Required program not found: {tool}")
        build()
        finish()
    check()
