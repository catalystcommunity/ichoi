import AVFoundation
import Combine
import MediaPlayer
import UIKit
import SwiftUI
import UniformTypeIdentifiers

/// Direct playback reads bounded byte ranges. Stop cancels requests and releases the player.
@MainActor
final class PlaybackManager: NSObject, ObservableObject, AVAudioPlayerDelegate {
    struct QueueEntry: Identifiable {
        let id = UUID()
        let track: Track
        let profile: ServerProfile
        let token: String?
    }
    @Published var queue: [QueueEntry] = []
    @Published private(set) var currentQueueID: UUID?
    private var previousQueue: [QueueEntry]?
    func enqueue(_ track: Track, profile: ServerProfile, token: String?) {
        previousQueue = queue
        queue.append(QueueEntry(track: track, profile: profile, token: token))
    }
    func removeQueued(_ index: Int) { previousQueue = queue; queue.remove(at: index) }
    func moveQueued(from: IndexSet, to: Int) { previousQueue = queue; queue.move(fromOffsets: from, toOffset: to) }
    func undoQueueChange() { if let previousQueue { queue = previousQueue }; previousQueue = nil }
    func playQueued(_ entry: QueueEntry) {
        currentQueueID = entry.id
        play(profile: entry.profile, url: entry.profile.url.appendingPathComponent("media").appendingPathComponent(entry.track.id), title: entry.track.title, bearerToken: entry.token, sourceCodec: entry.track.codec.rawValue)
    }
    private func completed() {
        if let index = queue.firstIndex(where: { $0.id == currentQueueID }), index + 1 < queue.count { playQueued(queue[index + 1]) }
        else { stop() }
    }
    @Published private(set) var isPlaying = false
    @Published private(set) var isLoading = false
    @Published private(set) var title = ""
    @Published private(set) var server = ""
    @Published private(set) var position = 0.0
    @Published private(set) var duration = 0.0
    var isActive: Bool { isLoading || player != nil || bufferedPlayer != nil }
    private var bufferedPlayer: AVAudioPlayer?
    private var bufferedRequest: Task<Void, Never>?
    private var player: AVPlayer?
    private var loader: ServerAssetLoader?
    private var statusObservation: NSKeyValueObservation?
    private var endObserver: NSObjectProtocol?
    private var requestID = UUID()
    private var timer: Timer?
    private let messages: AppMessages

    init(messages: AppMessages) {
        self.messages = messages
        super.init()
        UIApplication.shared.beginReceivingRemoteControlEvents()
        let center = MPRemoteCommandCenter.shared()
        center.playCommand.addTarget { [weak self] _ in
            Task { @MainActor in self?.resume() }; return .success
        }
        center.pauseCommand.addTarget { [weak self] _ in
            Task { @MainActor in self?.pause() }; return .success
        }
        center.stopCommand.addTarget { [weak self] _ in
            Task { @MainActor in self?.stop() }; return .success
        }
        center.changePlaybackPositionCommand.addTarget { [weak self] event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            let position = event.positionTime
            Task { @MainActor in self?.seek(to: position) }; return .success
        }
        updateNowPlaying()
    }

    func play(profile: ServerProfile, url: URL, title: String, bearerToken: String?, sourceCodec: String = "aac") {
        stop()
        self.title = title
        server = profile.url.absoluteString
        isLoading = true
        let id = requestID
        updateNowPlaying()
        if !["mp3", "aac", "vorbis", "flac", "alac", "wav"].contains(sourceCodec) {
            playConverted(profile: profile, url: url, token: bearerToken, id: id)
            return
        }
        do {
            let audio = AVAudioSession.sharedInstance()
            try audio.setCategory(.playback, mode: .default, options: [])
            try audio.setActive(true)
            let loader = try ServerAssetLoader(profile: profile, url: url, token: bearerToken)
            self.loader = loader
            let item = AVPlayerItem(asset: loader.asset)
            item.preferredForwardBufferDuration = 600
            let player = AVPlayer(playerItem: item)
            self.player = player
            statusObservation = item.observe(\.status, options: [.initial, .new]) { [weak self] item, _ in
                Task { @MainActor in
                    guard let self, self.requestID == id else { return }
                    switch item.status {
                    case .readyToPlay:
                        self.duration = item.duration.seconds.isFinite ? item.duration.seconds : 0
                        self.isLoading = false
                        self.isPlaying = true
                        self.player?.play()
                        self.startTimer()
                        self.updateNowPlaying()
                    case .failed: self.failPlayback()
                    default: break
                    }
                }
            }
            endObserver = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main) { [weak self] _ in
                Task { @MainActor in if self?.requestID == id { self?.completed() } }
            }
        } catch { failPlayback() }
    }

    func pause() {
        if let bufferedPlayer {
            bufferedPlayer.pause(); position = bufferedPlayer.currentTime; isPlaying = false
            timer?.invalidate(); timer = nil; updateNowPlaying(); return
        }
        guard let player, isPlaying else { return }
        player.pause()
        position = player.currentTime().seconds
        isPlaying = false
        timer?.invalidate(); timer = nil
        updateNowPlaying()
    }

    func resume() {
        if let bufferedPlayer {
            do {
                try AVAudioSession.sharedInstance().setActive(true)
                guard bufferedPlayer.play() else { throw URLError(.cannotDecodeContentData) }
                isPlaying = true; startTimer(); updateNowPlaying()
            } catch { failPlayback() }
            return
        }
        guard let player, !isLoading else { return }
        do {
            try AVAudioSession.sharedInstance().setActive(true)
            player.play()
            isPlaying = true
            startTimer()
            updateNowPlaying()
        } catch { failPlayback() }
    }

    func seek(to value: Double) {
        if let bufferedPlayer, value.isFinite {
            bufferedPlayer.currentTime = min(duration,max(0,value)); position = bufferedPlayer.currentTime; updateNowPlaying(); return
        }
        guard let player, value.isFinite else { return }
        position = min(duration, max(0, value))
        player.seek(to: CMTime(seconds: position, preferredTimescale: 600))
        updateNowPlaying()
    }

    func stop() {
        requestID = UUID()
        bufferedRequest?.cancel(); bufferedRequest = nil
        bufferedPlayer?.stop(); bufferedPlayer = nil
        statusObservation?.invalidate(); statusObservation = nil
        if let endObserver { NotificationCenter.default.removeObserver(endObserver) }
        endObserver = nil
        loader?.cancel(); loader = nil
        timer?.invalidate(); timer = nil
        player?.pause(); player?.replaceCurrentItem(with: nil); player = nil
        isPlaying = false; isLoading = false
        title = ""; server = ""; position = 0; duration = 0
        updateNowPlaying()
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    private func failPlayback() {
        let failedTitle = title
        stop()
        messages.show("Could not play \(failedTitle). Check the connection to the server and try again.")
    }

    private func startTimer() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                if let bufferedPlayer = self.bufferedPlayer {
                    self.position = bufferedPlayer.currentTime; self.updateNowPlaying(); return
                }
                guard let player = self.player else { return }
                let seconds = player.currentTime().seconds
                if seconds.isFinite { self.position = seconds }
                if let duration = player.currentItem?.duration.seconds, duration.isFinite { self.duration = duration }
                self.updateNowPlaying()
            }
        }
    }

    private func updateNowPlaying() {
        let center = MPRemoteCommandCenter.shared()
        center.playCommand.isEnabled = (player != nil || bufferedPlayer != nil) && !isPlaying
        center.pauseCommand.isEnabled = isPlaying
        center.stopCommand.isEnabled = isActive
        center.changePlaybackPositionCommand.isEnabled = player != nil || bufferedPlayer != nil
        guard isActive else { MPNowPlayingInfoCenter.default().nowPlayingInfo = nil; return }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = [
            MPMediaItemPropertyTitle: title,
            MPMediaItemPropertyPlaybackDuration: duration,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: position,
            MPNowPlayingInfoPropertyPlaybackRate: isPlaying ? 1.0 : 0.0,
        ]
    }

    // The server's converted AAC/MP3 response has no byte ranges. Preserve the existing
    // in-memory path for those codecs, with a limit instead of unbounded allocation.
    private func playConverted(profile: ServerProfile, url: URL, token: String?, id: UUID) {
        bufferedRequest = Task { [weak self] in
            do {
                let session = try ConnectionFactory.makeSession(for: profile)
                defer { session.invalidateAndCancel() }
                var request = URLRequest(url: url)
                if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
                let (stream,response) = try await session.bytes(for: request)
                guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else { throw URLError(.badServerResponse) }
                var data = Data()
                for try await byte in stream {
                    guard data.count < 64 * 1024 * 1024 else { throw URLError(.dataLengthExceedsMaximum) }
                    data.append(byte)
                }
                try Task.checkCancellation()
                guard let self, self.requestID == id else { return }
                let audio = AVAudioSession.sharedInstance()
                try audio.setCategory(.playback, mode: .default, options: []); try audio.setActive(true)
                let player = try AVAudioPlayer(data: data)
                self.bufferedPlayer = player; player.delegate = self
                guard player.prepareToPlay(), player.play() else { throw URLError(.cannotDecodeContentData) }
                self.duration = player.duration; self.isPlaying = true; self.isLoading = false
                self.bufferedRequest = nil; self.startTimer(); self.updateNowPlaying()
            } catch {
                guard !Task.isCancelled, let self, self.requestID == id else { return }
                self.failPlayback()
            }
        }
    }
    nonisolated func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        Task { @MainActor [weak self] in
            guard let self, self.bufferedPlayer === player else { return }
            if flag { self.completed() } else { self.failPlayback() }
        }
    }
    nonisolated func audioPlayerDecodeErrorDidOccur(_ player: AVAudioPlayer, error: Error?) {
        Task { @MainActor [weak self] in
            guard let self, self.bufferedPlayer === player else { return }
            self.failPlayback()
        }
    }

}


/// Uses the documented resource-loader interface so authenticated requests retain the
/// same URL and redirect policy as other server requests. No media is saved to disk.
@MainActor
final class ServerAssetLoader: NSObject, AVAssetResourceLoaderDelegate {
    let asset: AVURLAsset
    private let url: URL
    private let token: String?
    private let session: URLSession
    private var tasks: [ObjectIdentifier: Task<Void, Never>] = [:]
    private var stopped = false

    init(profile: ServerProfile, url: URL, token: String?) throws {
        self.url = url; self.token = token
        session = try ConnectionFactory.makeSession(for: profile)
        var components = URLComponents(url: url, resolvingAgainstBaseURL: false)!
        components.scheme = "ichoi-stream"
        asset = AVURLAsset(url: components.url!)
        super.init()
        asset.resourceLoader.setDelegate(self, queue: .main)
    }

    nonisolated func resourceLoader(_ resourceLoader: AVAssetResourceLoader, shouldWaitForLoadingOfRequestedResource request: AVAssetResourceLoadingRequest) -> Bool {
        Task { @MainActor [weak self] in self?.begin(request) }
        return true
    }
    nonisolated func resourceLoader(_ resourceLoader: AVAssetResourceLoader, didCancel request: AVAssetResourceLoadingRequest) {
        Task { @MainActor [weak self] in self?.tasks.removeValue(forKey: ObjectIdentifier(request))?.cancel() }
    }
    private func begin(_ request: AVAssetResourceLoadingRequest) {
        guard !stopped, !request.isCancelled else { return }
        let key = ObjectIdentifier(request)
        tasks[key] = Task { [weak self] in
            guard let self else { return }
            defer { self.tasks[key] = nil }
            do { try await self.load(request) }
            catch { if !Task.isCancelled && !request.isCancelled { request.finishLoading(with: error) } }
        }
    }
    func cancel() {
        stopped = true
        tasks.values.forEach { $0.cancel() }; tasks.removeAll()
        session.invalidateAndCancel()
        asset.cancelLoading()
    }
    private func load(_ loading: AVAssetResourceLoadingRequest) async throws {
        let dataRequest = loading.dataRequest
        var offset = max(dataRequest?.requestedOffset ?? 0, dataRequest?.currentOffset ?? 0)
        guard offset >= 0 else { throw URLError(.badServerResponse) }
        let requestedEnd: Int64
        if let dataRequest, !dataRequest.requestsAllDataToEndOfResource {
            let sum = dataRequest.requestedOffset.addingReportingOverflow(Int64(dataRequest.requestedLength))
            guard !sum.overflow else { throw URLError(.badServerResponse) }
            requestedEnd = sum.partialValue
        } else { requestedEnd = dataRequest == nil ? 2 : Int64.max }
        var total = Int64.max
        while offset < min(requestedEnd, total) {
            try Task.checkCancellation()
            guard !loading.isCancelled else { return }
            let end = MediaByteRange.chunkEnd(start: offset, exclusiveEnd: min(requestedEnd, total))
            var request = URLRequest(url: url)
            request.setValue("bytes=\(offset)-\(end)", forHTTPHeaderField: "Range")
            if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
            let (stream, response) = try await session.bytes(for: request)
            guard let http = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
            if http.statusCode == 416, let header = http.value(forHTTPHeaderField: "Content-Range"),
               header.hasPrefix("bytes */"), let length = Int64(header.dropFirst(8)), offset >= length {
                break
            }
            guard http.statusCode == 206, let range = MediaByteRange.parse(http.value(forHTTPHeaderField: "Content-Range"), requestedStart: offset, requestedEnd: end) else {
                throw URLError(.badServerResponse)
            }
            total = range.total
            loading.contentInformationRequest?.contentLength = total
            loading.contentInformationRequest?.isByteRangeAccessSupported = true
            loading.contentInformationRequest?.contentType = UTType(mimeType: http.mimeType ?? "audio/mp4")?.identifier
            var buffer = Data()
            let expected = Int(range.end - range.start + 1)
            buffer.reserveCapacity(expected)
            for try await byte in stream {
                guard buffer.count < expected else { throw URLError(.badServerResponse) }
                buffer.append(byte)
            }
            try Task.checkCancellation()
            guard buffer.count == expected, !loading.isCancelled else { throw URLError(.badServerResponse) }
            dataRequest?.respond(with: buffer)
            offset = range.end + 1
        }
        try Task.checkCancellation()
        if !loading.isCancelled { loading.finishLoading() }
    }
}
