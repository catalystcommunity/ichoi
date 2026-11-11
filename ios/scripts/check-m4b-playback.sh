#!/usr/bin/env bash
# Run on macOS. Test the real resource loader against a local authenticated range server.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
check_dir="$(mktemp -d /tmp/ichoi-m4b-check.XXXXXX)"
server_pid=""
cleanup() {
  if [[ -n "$server_pid" ]]; then kill "$server_pid" 2>/dev/null || true; fi
  rm -rf "$check_dir"
}
trap cleanup EXIT
python3 - "$repo" "$check_dir" <<'PY'
import pathlib,sys
repo=pathlib.Path(sys.argv[1]);out=pathlib.Path(sys.argv[2])
source=(repo/'ios/Ichoi/PlaybackManager.swift').read_text()
loader=source[source.index('@MainActor\nfinal class ServerAssetLoader'):]
(out/'Loader.swift').write_text('import Foundation\nimport AVFoundation\nimport UniformTypeIdentifiers\n'+loader)
PY
cat > "$check_dir/server.py" <<'PY'
import http.server,pathlib,sys
media=pathlib.Path(sys.argv[1]).read_bytes()
log=pathlib.Path(sys.argv[3])
class Handler(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  if self.headers.get('Authorization')!='Bearer test-m4b':self.send_error(401);return
  try:
   start,end=map(int,self.headers['Range'].removeprefix('bytes=').split('-'))
   assert 0<=start<=end and end-start<524288
   if start>=len(media):self.send_response(416);self.send_header('Content-Range',f'bytes */{len(media)}');self.end_headers();return
   end=min(end,len(media)-1)
  except Exception:self.send_error(400);return
  with log.open('a') as file:file.write(f'{start}-{end}\n')
  self.send_response(206);self.send_header('Content-Type','audio/mp4');self.send_header('Content-Range',f'bytes {start}-{end}/{len(media)}');self.send_header('Content-Length',str(end-start+1));self.end_headers();self.wfile.write(media[start:end+1])
 def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
pathlib.Path(sys.argv[2]).write_text(str(server.server_port))
server.serve_forever()
PY
python3 "$check_dir/server.py" "$repo/server/crates/ichoi/tests/fixtures/m4b/TwoChapters.M4B" "$check_dir/port" "$check_dir/requests" &
server_pid=$!
for attempt in {1..50}; do [[ -s "$check_dir/port" ]] && break; sleep 0.1; done
cat > "$check_dir/Check.swift" <<'SWIFT'
import Foundation
import AVFoundation

@main struct Check {
    @MainActor static func main() async throws {
        let base = URL(string: "http://127.0.0.1:\(CommandLine.arguments[1])")!
        let profile = try ServerProfile(name: "M4B test", url: base, allowsPrivateHTTP: true)
        let loader = try ServerAssetLoader(profile: profile, url: base.appendingPathComponent("book.m4b"), token: "test-m4b")
        defer { loader.cancel() }
        let duration = try await loader.asset.load(.duration).seconds
        precondition(duration >= 1.9 && duration < 2.2)
        let asset = loader.asset
        let samples = try await Task.detached {
            let track = try await asset.loadTracks(withMediaType: .audio).first!
            let reader = try AVAssetReader(asset: asset)
            let output = AVAssetReaderTrackOutput(track: track, outputSettings: [AVFormatIDKey: kAudioFormatLinearPCM])
            reader.add(output)
            reader.timeRange = CMTimeRange(start: CMTime(seconds: 1, preferredTimescale: 600), duration: CMTime(seconds: 1, preferredTimescale: 600))
            precondition(reader.startReading())
            var samples = 0
            while let buffer = output.copyNextSampleBuffer() { samples += CMSampleBufferGetNumSamples(buffer) }
            if let error = reader.error { throw error }
            return samples
        }.value
        precondition(samples > 0)
        print("M4B duration, authenticated range loading, and decoding from the one-second seek position passed (\(samples) PCM samples).")
        let cancelled = try ServerAssetLoader(profile: profile, url: base.appendingPathComponent("book.m4b"), token: "test-m4b")
        let pending = Task { try await cancelled.asset.load(.duration) }
        cancelled.cancel(); pending.cancel()
        _ = try? await pending.value
        print("Cancellation during preparation passed.")
    }
}
SWIFT
swiftc -parse-as-library "$repo/ios/IchoiCore/ServerProfile.swift" "$repo/ios/IchoiCore/ConnectionSecurity.swift" "$check_dir/Loader.swift" "$check_dir/Check.swift" -o "$check_dir/check"
"$check_dir/check" "$(cat "$check_dir/port")"
test -s "$check_dir/requests"
