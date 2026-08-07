import type { ServerApi } from "./services.ts";
import type { ImportResult, Library, SearchResponse, Track } from "./schema.ts";

export interface FederationServer {
  id: string;
  name: string;
  api: ServerApi;
}

export interface FederatedSearchResult {
  server: FederationServer;
  library: Library;
  response?: SearchResponse;
  error?: string;
}

export async function searchAllInstances(
  instances: FederationServer[],
  query: string,
  limit = 50,
): Promise<FederatedSearchResult[]> {
  const requests = instances.flatMap((server) =>
    (["music", "audiobook"] as const).map(async (library): Promise<FederatedSearchResult> => {
      try {
        const response = await server.api.library.search({ query, library, limit });
        return { server, library, response };
      } catch (error) {
        return { server, library, error: error instanceof Error ? error.message : String(error) };
      }
    }),
  );
  return Promise.all(requests);
}

function safeInstanceFolder(name: string): string {
  const value = name
    .normalize("NFKD")
    .replace(/[^a-zA-Z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
  return value || "instance";
}

export function importedTrackPath(instanceName: string, sourcePath: string): string {
  return `imports/${safeInstanceFolder(instanceName)}/${sourcePath}`;
}

export async function copyTrackToInstance(
  source: FederationServer,
  destination: FederationServer,
  track: Track,
): Promise<Track> {
  if (source.id === destination.id) return track;
  const manifest = await source.api.library.exportManifest({ track_id: track.id });
  const sourceTrackPath = manifest.files[0]?.root_relative_path;
  if (!sourceTrackPath) throw new Error("The source returned an empty transfer manifest");
  const targetTrackPath = importedTrackPath(source.name, sourceTrackPath);
  const sourceFolder = sourceTrackPath.includes("/")
    ? sourceTrackPath.slice(0, sourceTrackPath.lastIndexOf("/") + 1)
    : "";
  const targetFolder = targetTrackPath.includes("/")
    ? targetTrackPath.slice(0, targetTrackPath.lastIndexOf("/") + 1)
    : "";
  const files = manifest.files.map((file, index) => ({
    ...file,
    root_relative_path: index === 0
      ? targetTrackPath
      : `${targetFolder}${file.root_relative_path.slice(sourceFolder.length)}`,
  }));
  const begun = await destination.api.admin.beginImport({
    library: manifest.track.library,
    track_file_index: 0,
    files,
  });
  let result: ImportResult;
  try {
    for (const missing of begun.missing_chunks) {
      const sourceFile = manifest.files[missing.file_index];
      if (!sourceFile) throw new Error("The destination requested an unknown file");
      const chunk = await source.api.library.exportChunk({
        track_id: track.id,
        root_relative_path: sourceFile.root_relative_path,
        chunk_index: missing.chunk_index,
      });
      await destination.api.admin.importChunk({
        transfer_id: begun.transfer_id,
        file_index: missing.file_index,
        chunk_index: missing.chunk_index,
        data: chunk.data,
      });
    }
    result = await destination.api.admin.finishImport({ transfer_id: begun.transfer_id });
  } catch (error) {
    await destination.api.admin.cancelImport({ transfer_id: begun.transfer_id }).catch(() => undefined);
    throw error;
  }
  if (!result.track) throw new Error("The destination did not return the imported track");
  return result.track;
}

export async function copyTracksToInstance(
  source: FederationServer,
  destination: FederationServer,
  tracks: Track[],
): Promise<Track[]> {
  const copied: Track[] = [];
  for (const track of tracks) copied.push(await copyTrackToInstance(source, destination, track));
  return copied;
}
