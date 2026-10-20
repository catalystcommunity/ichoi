# Ichoi brand assets

Open `index.html` to see the designs.
These files are ready for import. They are not installed in the app or website.

## Select a file

| Folder or file | Use |
| --- | --- |
| `ichoi-logo.svg`, `ichoi-logo.png` | Original approved logo with a dark rounded square. PNG: 2048 × 2048. |
| `ichoi-mark.svg`, `ichoi-mark.png` | Approved mark with a transparent background. PNG: 2048 × 2048. |
| `logos/` | Transparent marks and horizontal and stacked logos. Use `dark` on dark backgrounds and `light` on light backgrounds. Black and white versions are also available. |
| `social/avatar-*` | Profile images at 512 and 1024 pixels. The background fills the square. The mark stays inside a circular crop. |
| `social/share-*` | Link preview cards at 1200 × 630, with light and dark backgrounds. |
| `social/wide-*` | A 1200 × 600 card for a 2:1 image slot. |
| `social/repository-*` | A 1280 × 640 repository preview card. |
| `social/square-*` | A 1080 × 1080 feed image. |
| `social/portrait-*` | A 1080 × 1350 feed image. |
| `social/story-*` | A 1080 × 1920 story image. Main content stays clear of the top and bottom controls. |
| `social/header-*` | A 1500 × 500 profile header. The lower-left area is clear for a profile image. |
| `social/professional-header-*` | A 1584 × 396 profile header. |
| `social/channel-*` | A 2560 × 1440 channel header. Main content fits the central 1546 × 423 area. |
| `stores/` | Google Play listing icon and feature graphic. |
| `apps/` | App icons for Apple, Android, and desktop systems. |
| `web/` | Favicons, touch icon, PWA icons, and manifest entries. |

Social services can crop images or add controls. Check the preview in the target
service before publication. Use the SVG files to change a layout. All exported
SVG files contain vector shapes. They do not require fonts or external images.

## App and store files

- **Apple:** Import `apps/apple/AppIcon.appiconset` into the Xcode asset catalog.
  Set the app icon name to `AppIcon`. The set contains default, dark, and tinted
  appearances. The 1024 × 1024 PNG files have RGB channels and no alpha channel.
  The platform applies the corner mask. The `layers/` folder contains SVG inputs
  for Icon Composer. Set the background color to `#101417`, then import the ring
  and notes as foreground layers. The background SVG is a color reference.
  A native Icon Composer project must be assembled and checked on macOS.
- **Android:** Copy `apps/android/res/` into the app resources. Set
  `android:icon="@mipmap/ic_launcher"` on the application. The set contains
  adaptive icons, a monochrome layer for themed icons, and legacy PNG files at
  five densities. Each adaptive layer is 108 dp. The mark fits a 64 dp circle
  inside the 66 dp safe area. The standalone foreground PNG files are 432 pixels.
- **Google Play:** Use `stores/google-play-icon-512.png` for the listing icon.
  It is a 512 × 512 RGBA PNG with an opaque square background. Use
  `stores/google-play-feature-1024x500.png` for the feature graphic. It is an
  RGB PNG with no alpha channel. Suggested alternative text: “Ichoi. Your music.
  Every room. Audio level bars on a teal background.” Store screenshots and
  preview videos are separate assets. They must show the actual app.
- **Desktop:** Use `apps/desktop/ichoi.ico` for Windows. It contains seven sizes
  from 16 through 256 pixels. Use `apps/desktop/ichoi.icns` for macOS. It contains
  128, 256, 512, and 1024 pixel images. Individual PNG files are also available.
- **Web:** Use `favicon.svg`, `favicon.ico`, and `apple-touch-icon.png` from
  `web/`. Merge `manifest-icons.json` into the existing web manifest. Change the
  image paths if the files are not served from the site root. Use the separate
  `maskable` files for that purpose. They have opaque square backgrounds, and
  the mark fits the central safe circle. Do not use the rounded logo for this.

For link previews, host the selected card at a public HTTPS URL. Add that URL to
`og:image` and `twitter:image`. Set `twitter:card` to `summary_large_image`. Set
the image width, height, and alternative text to match the selected file. The
kit does not assume a public domain or change the website metadata.

## Design rules

Keep the two notes inside the circle. Do not stretch, rotate, or crop the mark.
Keep at least one ring stroke of clear space around the circle. Use the black
or white version when only one color is available. Use the light-background
versions for small marks on white or cream surfaces.

| Color | Value | Use |
| --- | --- | --- |
| Dark | `#101417` | Background |
| Cream | `#f3ecdd` | Text on dark backgrounds |
| Teal | `#37d6c6` | Ring on dark backgrounds |
| Amber | `#ffb454` | Notes on dark backgrounds |
| Dark teal | `#087f78` | Ring on light backgrounds |
| Dark amber | `#a85c00` | Notes on light backgrounds |

## Build and check

Requirements: Python 3, `rsvg-convert`, and ImageMagick 7 (`magick`).

```sh
bash assets/brand/export.sh
bash assets/brand/export.sh --check
```

Edit `ichoi-mark.svg` for the symbol. Edit the vector sources in `source/` for
the lettering. Edit `build.py` for layouts and output sizes. Do not edit the
generated files. The build does not use the network or installed fonts.

`manifest.json` lists dimensions, uses, and SHA-256 checksums. The check command
checks PNG dimensions and channel types, SVG and XML syntax, checksums, safe
areas, Apple file references, the Play icon file size, and ZIP integrity.
Run the export twice and compare file hashes after changes to the generator.
Different renderer versions can produce different PNG bytes.

## Platform references

Requirements were checked on 2026-10-17. The files follow these references:

- [Apple asset catalogs](https://developer.apple.com/documentation/xcode/configuring-your-app-icon)
- [Apple Icon Composer](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer)
- [Android adaptive icons](https://developer.android.com/develop/ui/compose/system/icon_design_adaptive)
- [Google Play preview assets](https://support.google.com/googleplay/android-developer/answer/9866151?hl=en)
- [Web manifest icon masks](https://www.w3.org/TR/appmanifest/#icon-masks)

## Next Actions

Import the app icons and add the web metadata when the assets are applied.
Check launcher masks on devices and inspect social previews before publication.
These checks confirm how each platform displays the files.
