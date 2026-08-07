# Instance federation

## Purpose

Instance federation lets one browser use more than one Ichoi instance. The browser can
search all connected instances. It can copy a remote track to the selected instance. It
can then add the local copy to a queue or a playlist.

The media belongs to an instance. It does not belong to the user who copies it. A copy
becomes a normal file in the destination library. All users with library access can find
that file.

## Data flow

The selected instance is the destination. The browser does these steps:

1. It sends a music search and an audiobook search to each connected instance.
2. It shows the source instance and library for each result.
3. It asks the source instance for a transfer manifest.
4. The manifest lists each file and each 16 MiB chunk.
5. The manifest gives a SHA-256 hash for each file and chunk.
6. The destination returns a list of chunks that it does not have.
7. The browser moves each missing chunk from the source to the destination.
8. The destination verifies each chunk and each completed file.
9. The destination writes and indexes the audio file.
10. The destination returns its local track ID.
11. The browser adds that local track ID to the local queue.

An album action copies all tracks in the album. An artist action copies all tracks in all
albums for that artist. The queue can use the existing **Save queue** action to make an m3u
playlist.

The source instance does not connect to the destination instance. The browser moves the
data. This rule works through NAT and keeps instance credentials in the browser session.

## Access rules

An administrator or member can export from an instance. A guest account cannot export.
An administrator can import into an instance. A login-less instance with no accounts also
permits these operations.

These rules protect the instance library. They do not create user-owned media or a
user-to-user share. The destination administrator controls writes to the destination
library.

Jukebox targets are not part of federation. A remote target does not appear on the
destination instance. All queue and playlist changes use the selected destination.

## File rules

The browser puts remote files below `imports/<source-name>/`. The destination rejects an
absolute path, a parent path, and a backslash. It verifies the manifest before it opens a
transfer session.

The destination uses a stored hash to find an existing file in the same library. It returns
the existing local track when it finds one. It does not write a second copy. If a different
file already has the requested path, the destination returns a conflict.

## Sidecar files

The manifest includes adjacent folder images in JPEG, PNG, GIF, or WebP format. It includes
`.lrc` lyric files. It also includes a text file when its name matches the track name or is
`lyrics.txt`. Embedded art and embedded lyrics stay in the audio file.

The transfer does not include an m3u file. The local queue can make a new local playlist.
It does not include an unrelated text file.

## Transfer limits

One request contains at most one 16 MiB chunk. The manifest can describe a file of any size
that the local file system supports. The browser keeps only the current chunk in transfer
memory. An incomplete transfer expires after one hour. A failed browser transfer also asks
the destination to cancel its session.
