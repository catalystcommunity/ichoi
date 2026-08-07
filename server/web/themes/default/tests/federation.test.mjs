import assert from "node:assert/strict";
import test from "node:test";
import {
  copyTrackToInstance,
  importedTrackPath,
  searchAllInstances,
} from "../src/lib/federation.ts";

const track = {
  id: "remote-track",
  library: "music",
  title: "A Song",
  duration_ms: 1,
  codec: "flac",
  sample_rate: 44100,
  channels: 2,
  root_relative_path: "Artist/Album/01.flac",
};

test("searches both libraries on every connected instance", async () => {
  const calls = [];
  const instance = (id) => ({
    id,
    name: id,
    api: { library: { search: async (request) => {
      calls.push([id, request.library]);
      return { artists: [], albums: [], tracks: [] };
    } } },
  });

  const results = await searchAllInstances([instance("home"), instance("friend")], "song");

  assert.equal(results.length, 4);
  assert.deepEqual(calls.sort(), [
    ["friend", "audiobook"], ["friend", "music"],
    ["home", "audiobook"], ["home", "music"],
  ]);
});

test("keeps results from available instances when one search fails", async () => {
  const server = {
    id: "home",
    name: "Home",
    api: { library: { search: async ({ library }) => {
      if (library === "audiobook") throw new Error("not configured");
      return { artists: [], albums: [], tracks: [track] };
    } } },
  };

  const results = await searchAllInstances([server], "song");

  assert.equal(results[0].response.tracks.length, 1);
  assert.match(results[1].error, /not configured/);
});

test("copies missing chunks and sidecars and returns the destination track", async () => {
  let begun;
  const chunks = [];
  const source = {
    id: "friend",
    name: "Friend's Cloud",
    api: { library: {
      exportManifest: async () => ({
        track,
        files: [
          {
            root_relative_path: track.root_relative_path,
            content_type: "audio/flac",
            size_bytes: 2,
            sha256: "audio-hash",
            chunks: [{ index: 0, offset: 0, size: 2, sha256: "chunk-a" }],
          },
          {
            root_relative_path: "Artist/Album/cover.jpg",
            content_type: "image/jpeg",
            size_bytes: 1,
            sha256: "art-hash",
            chunks: [{ index: 0, offset: 0, size: 1, sha256: "chunk-b" }],
          },
        ],
      }),
      exportChunk: async (request) => ({ ...request, data: new Uint8Array([request.root_relative_path.endsWith("jpg") ? 3 : 1]) }),
    } },
  };
  const localTrack = { ...track, id: "local-track" };
  const destination = {
    id: "home",
    name: "Home",
    api: { admin: {
      beginImport: async (request) => {
        begun = request;
        return {
          transfer_id: "transfer-1",
          missing_chunks: [{ file_index: 0, chunk_index: 0 }, { file_index: 1, chunk_index: 0 }],
        };
      },
      importChunk: async (request) => { chunks.push(request); return { ok: true }; },
      finishImport: async () => ({ imported: true, track: localTrack }),
      cancelImport: async () => ({ ok: true }),
    } },
  };

  const result = await copyTrackToInstance(source, destination, track);

  assert.equal(result.id, "local-track");
  assert.equal(begun.library, "music");
  assert.equal(begun.files[0].root_relative_path, "imports/Friend-s-Cloud/Artist/Album/01.flac");
  assert.equal(begun.files[1].root_relative_path, "imports/Friend-s-Cloud/Artist/Album/cover.jpg");
  assert.equal(chunks.length, 2);
  assert.equal(chunks[0].data.byteLength, 1);
});

test("does not copy a track that is already on the destination", async () => {
  const instance = { id: "home", name: "Home", api: {} };
  assert.strictEqual(await copyTrackToInstance(instance, instance, track), track);
});

test("cancels the destination session when a chunk fails", async () => {
  let cancelled;
  const file = {
    root_relative_path: track.root_relative_path,
    content_type: "audio/flac",
    size_bytes: 1,
    sha256: "file-hash",
    chunks: [{ index: 0, offset: 0, size: 1, sha256: "chunk-hash" }],
  };
  const source = {
    id: "friend",
    name: "Friend",
    api: { library: {
      exportManifest: async () => ({ track, files: [file] }),
      exportChunk: async () => { throw new Error("source disconnected"); },
    } },
  };
  const destination = {
    id: "home",
    name: "Home",
    api: { admin: {
      beginImport: async () => ({
        transfer_id: "transfer-2",
        missing_chunks: [{ file_index: 0, chunk_index: 0 }],
      }),
      cancelImport: async (request) => { cancelled = request.transfer_id; return { ok: true }; },
    } },
  };

  await assert.rejects(copyTrackToInstance(source, destination, track), /source disconnected/);
  assert.equal(cancelled, "transfer-2");
});

test("creates a stable import folder from an instance name", () => {
  assert.equal(importedTrackPath("  My Friend ☁  ", "A/B.flac"), "imports/My-Friend/A/B.flac");
});
