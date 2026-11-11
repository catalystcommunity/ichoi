# Cover art

Ichoi stores a shared cover for each album, audiobook, or artist. An administrator
can change covers. A member needs the **Edit library artwork** permission. Grant
or remove this permission in Settings, under Accounts and LinkKeys access. The
server checks the current permission on each save and reset request.

Right-click a cover on the web to open the picker. A keyboard user can focus the
cover and press Shift+F10. On Android and iOS, press and hold the album or artist
row. Select a local library image, upload an image, or select **Use automatic
cover** to remove the manual choice. The local picker shows previews, dimensions,
image roles, and the current automatic choice. It includes backs and booklets for
manual selection. Android uses the system document picker for uploads. iOS uses
the system photo picker. The apps do not request access to the full photo library.

The server accepts JPEG, PNG, GIF, and WebP images. The limit is 8 MB and 8192
pixels per side. The server decodes each image, removes metadata, and stores a
JPEG with a maximum dimension of 1600 pixels. Mobile apps also prepare selected
photos before upload. A failed upload leaves the previous cover in place.

## Automatic discovery

The resolver uses this order:

1. A manual choice from an authorized user.
2. A local front cover with a clear album match. This includes embedded front
   pictures and files named Front, Cover, Folder, Album, or the album title.
3. A local image with less certain labels but a clear association. A single
   numbered book cover is valid. Multiple unknown scans are not automatic covers.
4. A small thumbnail.
5. A verified external match.
6. A placeholder with the album title, folder name, or artist name.

Within each local group, useful image resolution takes precedence over filename
conventions. The shorter image side sets the quality score, up to 1600 pixels.
Exact repeated-pixel enlargement does not increase that score. Other enlargement
methods are not reliably detected. File size does not affect the score. Filename
conventions break quality ties. An existing choice remains stable on an equal rank.

The scanner checks filenames without regard to letter case. Art, Artwork, Cover,
Covers, Scans, and Images folders have the same rank as the album folder. A
recognized disc folder can use its parent album's cover. The search does not
cross the library root. Portrait book covers are valid. Back covers, disc labels,
interior pages, and wide sleeve spreads are available for manual selection only.
A filename such as Front-back does not make a sleeve spread a front cover.

A folder with several albums needs an image that matches the album. A generic
Folder image or a Windows AlbumArt identifier must not apply to every album in
that folder. Invalid images are skipped. Identical image bytes appear once in
the picker. Different resolutions or crops remain available as separate choices.

The scan stores candidate metadata and the automatic selection in the database.
A single-file import refreshes covers for the affected album folder. An external
lookup requires one high-confidence release with an exact normalized title and
artist match, plus a matching year when known. An ambiguous release is skipped.

An artist uses an album cover until an authorized user selects an artist image.
Cards with no image show a readable title or name. If an album tag contains no
letters or numbers, the album response uses the track folder name.

Manual covers are stored separately from scan results in the database. Downloads
from the existing external artwork lookup are also stored in the database. Media
folders can remain read-only. A scan or external lookup cannot replace a manual
choice. A reset removes only the manual choice. It restores the local, embedded,
or downloaded cover, if one exists.

Choices use the catalog ID. A normal scan retains them. A change to tags or album
grouping that creates a different catalog ID can require a new choice. Records
are retained when a scan temporarily removes and restores an album ID.

## M4B audiobooks

The scanner reads M4B files without regard to extension case. It reads MP4 tags
and embedded covers, and probes the audio codec. AAC and ALAC files are covered
by integration tests. A book is listed as one track; this change does not add a
chapter navigation interface.

The HTTP endpoint sends MP4 audio with byte-range support. It streams the file
instead of loading the entire audiobook into server memory. The iOS player uses
authenticated requests of at most 512 KiB for direct playback and seeking. Stop
cancels these requests. Codecs that require server conversion retain the previous
iOS buffered playback path, now limited to 64 MiB. Long converted files can exceed
that limit. Direct AAC and ALAC M4B files do not use that path.

## Settings correction

The request dispatcher now routes content-report list and status requests to the
existing handlers. It also routes the existing account-deletion operations. A
failed content-report load stays in that panel and does not replace Settings.
An older server shows a message that the report panel requires a server update.

## Validation

Automated checks cover cover quality, sleeve exclusion, mixed folders, numbered
book covers, stable choices, imports, manual persistence, reset, permissions,
changed candidate files, request dispatch, and cache invalidation. M4B tests cover
AAC and ALAC metadata, embedded art, full delivery, byte ranges, and invalid ranges.
The synthetic M4B fixture contains two chapters.

The Rust workspace suite, web tests, Android build and lint, and unsigned iOS
build pass. Core iOS tests cover byte-range validation. The Mac playback check uses
the app's resource loader to decode the M4B fixture from the one-second position.
It also checks authenticated range requests and cancellation during preparation.
Run it with `ios/scripts/check-m4b-playback.sh` on macOS. Physical phone playback
and the native image pickers still need device checks.

## Next Actions

1. Release the server and web assets together, then release the mobile apps.
2. Run a library rescan to find local images missed by the old scanner.
3. Grant artwork permission only to members who can change shared covers.
4. Check the photo and document pickers on physical Android and iOS devices.
   Check cancel, save, reset, a large image, a server change, and loss of connection.
5. Play and seek through a long M4B book on each phone. Check pause and stop.
