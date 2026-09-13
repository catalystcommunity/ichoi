import AVFoundation
import Combine
import MediaPlayer
import UIKit

/// Playback uses transient in-memory media data. No media data is written to
/// shared or app-private persistent storage.
@MainActor
final class PlaybackManager: NSObject, ObservableObject {
    @Published private(set) var isPlaying = false
    private var player: AVAudioPlayer?

    override init() {
        super.init()
        configureAudioSession()
        UIApplication.shared.beginReceivingRemoteControlEvents()
        let commandCenter = MPRemoteCommandCenter.shared()
        commandCenter.playCommand.addTarget { [weak self] _ in self?.player?.play(); self?.isPlaying = true; return .success }
        commandCenter.pauseCommand.addTarget { [weak self] _ in self?.player?.pause(); self?.isPlaying = false; return .success }
    }

    func play(profile: ServerProfile, url: URL, title: String, bearerToken: String?) async throws {
        var request = URLRequest(url: url)
        if let bearerToken { request.setValue("Bearer \(bearerToken)", forHTTPHeaderField: "Authorization") }
        let session = try ConnectionFactory.makeSession(for: profile)
        let (data, response) = try await session.data(for: request)
        guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode) else {
            throw URLError(.badServerResponse)
        }
        player = try AVAudioPlayer(data: data)
        player?.prepareToPlay()
        player?.play()
        isPlaying = true
        MPNowPlayingInfoCenter.default().nowPlayingInfo = [MPMediaItemPropertyTitle: title]
    }
    func pause() { player?.pause(); isPlaying = false }
    private func configureAudioSession() {
        let audio = AVAudioSession.sharedInstance()
        try? audio.setCategory(.playback, mode: .default, options: [])
        try? audio.setActive(true)
    }
}
