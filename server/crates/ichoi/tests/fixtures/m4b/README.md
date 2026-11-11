# M4B test media

These files contain a generated tone and a generated test image. They contain no
library media. Both files have two one-second chapters, MP4 tags, and a PNG cover.
`TwoChapters.M4B` uses AAC. `TwoChaptersALAC.m4b` uses ALAC.

The server test checks metadata, cover extraction, full delivery, byte ranges,
and invalid ranges. The macOS script `ios/scripts/check-m4b-playback.sh` uses the
AAC file to check decoding, seeking, authorization, and cancellation.

To make equivalent fixtures, use FFmpeg with a two-second sine source and a
256-by-256 test image. Set the tags as follows:

- title: Two Chapters
- album: The Test Audiobook
- artist: Test Reader
- date: 2026

Set chapter names to Chapter One and Chapter Two. Set their time ranges to 0–1
and 1–2 seconds. Use `-disposition:v attached_pic` for the image and `-f mp4` for
the container. Use `-c:a aac` or `-c:a alac` for the audio codec. Exact bytes can
change with the FFmpeg version; the tests check content and behavior.
