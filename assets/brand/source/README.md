# Lettering sources

These SVG files contain fixed vector outlines. No font is required to export
the brand kit. The outlines use DejaVu Sans and DejaVu Sans Bold.

| File | Text |
| --- | --- |
| `name.svg` | ichoi |
| `headline-1.svg` | Your music. |
| `headline-2.svg` | Every room. |
| `description.svg` | Your library. A shared jukebox. |

The source text was rendered with `rsvg-convert --format svg`. The view boxes
include two units of space around the visible shapes. The fill uses
`currentColor`, so each layout can set the color. To change the text, make new
outlines and replace the applicable source file. Do not put live text elements
in these files. Keep the font license with the sources.
