import SwiftUI

struct AppControls: ViewModifier {
    @EnvironmentObject private var model: AppModel
    @EnvironmentObject private var messages: AppMessages
    @EnvironmentObject private var playback: PlaybackManager

    func body(content: Content) -> some View {
        content
            .safeAreaInset(edge: .bottom) {
                if model.profile != nil && model.hasAcceptedTerms { PlaybackControls() }
            }
            .overlay(alignment: .top) {
                if let message = messages.current {
                    HStack(alignment: .top) {
                        Text(message.text).frame(maxWidth: .infinity, alignment: .leading)
                        Button("Dismiss") { messages.dismiss(id: message.id) }
                    }
                    .padding().background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
                    .padding().accessibilityElement(children: .contain)
                }
            }
    }
}

extension View {
    func appControls() -> some View { modifier(AppControls()) }
}

struct MessageDurationSetting: View {
    @EnvironmentObject private var messages: AppMessages
    var body: some View {
        Section("Messages") {
            Stepper("Message duration: \(messages.durationSeconds) seconds", value: $messages.durationSeconds, in: 1...60)
            Text("You can dismiss a message before this time ends.").font(.footnote)
        }
    }
}

/// Each control has a visible boundary and at least a 44-point touch target.
struct IchoiControlStyle: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    @Environment(\.colorScheme) private var scheme
    var primary = false
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.subheadline.weight(.semibold))
            .padding(.horizontal, 12).frame(minWidth: 44, minHeight: 44)
            .foregroundStyle(primary ? (scheme == .dark ? Color.black : Color.white) : Color.primary)
            .background(primary ? Color("AccentColor") : Color("BrandSurface"), in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(primary ? Color("AccentColor") : Color.secondary.opacity(0.6), lineWidth: 1))
            .opacity(enabled ? (configuration.isPressed ? 0.65 : 1) : 0.4)
    }
}

struct AppUpdatesView: View {
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            Form {
                Section("Installed version") {
                    Text("Ichoi \(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "Unknown") (\(Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "Unknown"))")
                }
                Section("Beta updates") {
                    Text("In TestFlight, select Ichoi and turn on Automatic Updates. New test builds then use the same installation. You do not need to join the test again.")
                    Text("TestFlight builds expire after 90 days. If an update fails, keep the error message and the installed version for the developer.")
                    Link("TestFlight update help", destination: URL(string: "https://testflight.apple.com/")!)
                }
                Section("App Store updates") { Text("For an App Store installation, use automatic app updates or open your account in the App Store to check for updates.") }
            }.navigationTitle("App and updates")
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
}

struct MusicTransportStyle: ButtonStyle {
    var primary = false
    @Environment(\.colorScheme) private var scheme
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.system(size: 21, weight: .semibold)).frame(minWidth: 44, minHeight: 44)
            .foregroundStyle(primary ? (scheme == .dark ? Color.black : Color.white) : Color("BrandText"))
            .background(primary ? Color("AccentColor") : Color.clear, in: Circle())
            .opacity(enabled ? (configuration.isPressed ? 0.5 : 1) : 0.3)
    }
}

@MainActor private enum PlayerArtMemory {
    static let images: NSCache<NSString, UIImage> = { let cache = NSCache<NSString, UIImage>(); cache.totalCostLimit = 24 * 1024 * 1024; return cache }()
}
struct PlayerArtwork: View {
    let albumID: String?
    var size: CGFloat
    @EnvironmentObject private var model: AppModel
    @State private var image: UIImage?
    var body: some View {
        ZStack {
            Color("BrandSurface")
            if let image { Image(uiImage: image).resizable().scaledToFill() }
            else { Image("BrandMark").resizable().scaledToFit().padding(size * 0.22) }
        }.frame(width: size, height: size).clipped().clipShape(RoundedRectangle(cornerRadius: size > 100 ? 14 : 6))
            .accessibilityHidden(true)
            .task(id: "\(model.profile?.id.uuidString ?? ""):\(albumID ?? "")") {
                image = nil
                guard let albumID, let profile = model.profile else { return }
                let key = "\(profile.id):\(albumID)" as NSString
                if let cached = PlayerArtMemory.images.object(forKey: key) { image = cached; return }
                do {
                    let api = LibraryAsyncClient(transport: try CSILHTTPTransport(profile: profile, token: model.token()))
                    let art = try await api.getCoverArt(CoverArtRequest(albumId: albumID, maxSize: 512))
                    guard !Task.isCancelled, let decoded = UIImage(data: Data(art.data)) else { return }
                    PlayerArtMemory.images.setObject(decoded, forKey: key, cost: Int(decoded.size.width * decoded.size.height * decoded.scale * decoded.scale * 4)); image = decoded
                } catch { }
            }
    }
}

/// A native slider that accepts a tap anywhere on its track, as well as a drag.
private struct MusicSeekSlider: UIViewRepresentable {
    var value: Double
    var duration: Double
    var identity: String
    var enabled: Bool
    var preview: (Double) -> Void
    var commit: (Double) -> Void
    var cancel: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeUIView(context: Context) -> TapSeekSlider {
        let slider = TapSeekSlider()
        slider.minimumValue = 0
        slider.accessibilityLabel = "Playback position"
        slider.minimumTrackTintColor = UIColor(named: "AccentColor")
        slider.addTarget(context.coordinator, action: #selector(Coordinator.began), for: .editingDidBegin)
        slider.addTarget(context.coordinator, action: #selector(Coordinator.changed), for: .valueChanged)
        slider.addTarget(context.coordinator, action: #selector(Coordinator.ended), for: .editingDidEnd)
        slider.addTarget(context.coordinator, action: #selector(Coordinator.cancelled), for: .touchCancel)
        return slider
    }
    func updateUIView(_ slider: TapSeekSlider, context: Context) {
        context.coordinator.parent = self
        slider.maximumValue = Float(max(1, duration))
        slider.isEnabled = enabled
        if !slider.scrubbing { slider.setValue(Float(value), animated: false) }
        slider.accessibilityValue = "\(Int(value) / 60):\(String(format: "%02d", Int(value) % 60)) of \(Int(duration) / 60):\(String(format: "%02d", Int(duration) % 60))"
    }
    final class Coordinator: NSObject {
        var parent: MusicSeekSlider
        var gestureIdentity: String?
        init(_ parent: MusicSeekSlider) { self.parent = parent }
        @objc func began(_ slider: TapSeekSlider) { gestureIdentity = parent.identity }
        @objc func changed(_ slider: TapSeekSlider) {
            guard parent.enabled, !slider.scrubbing || gestureIdentity == parent.identity else { return }
            parent.preview(Double(slider.value))
            // VoiceOver and keyboard adjustments do not have a touch gesture.
            if !slider.scrubbing { parent.commit(Double(slider.value)) }
        }
        @objc func ended(_ slider: TapSeekSlider) {
            if parent.enabled && gestureIdentity == parent.identity { parent.commit(Double(slider.value)) }
            else { parent.cancel() }
            gestureIdentity = nil
        }
        @objc func cancelled(_ slider: TapSeekSlider) { gestureIdentity = nil; parent.cancel() }
    }
}

private final class TapSeekSlider: UISlider {
    var scrubbing = false
    override func beginTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
        guard isEnabled else { return false }
        scrubbing = true
        sendActions(for: .editingDidBegin)
        update(touch)
        return true
    }
    override func continueTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
        update(touch)
        return true
    }
    override func endTracking(_ touch: UITouch?, with event: UIEvent?) {
        if let touch { update(touch) }
        scrubbing = false
        sendActions(for: .editingDidEnd)
    }
    override func cancelTracking(with event: UIEvent?) {
        scrubbing = false
        sendActions(for: .touchCancel)
        super.cancelTracking(with: event)
    }
    private func update(_ touch: UITouch) {
        let track = trackRect(forBounds: bounds)
        let left = thumbRect(forBounds: bounds, trackRect: track, value: minimumValue).midX
        let right = thumbRect(forBounds: bounds, trackRect: track, value: maximumValue).midX
        guard right != left else { return }
        let fraction = min(1, max(0, (touch.location(in: self).x - left) / (right - left)))
        setValue(minimumValue + Float(fraction) * (maximumValue - minimumValue), animated: false)
        sendActions(for: .valueChanged)
    }
}

struct PlaybackControls: View {
    var fullScreen = false
    @EnvironmentObject private var playback: PlaybackManager
    @EnvironmentObject private var remote: RemotePlayback
    @Environment(\.dismiss) private var dismiss
    @State private var expanded = false
    @State private var queueOpen = false
    @State private var devicesOpen = false
    @State private var seeking = false
    @State private var seekPosition = 0.0
    @State private var hasSeekPreview = false
    @State private var seekGeneration = 0
    private var displayedPosition: Double { hasSeekPreview ? seekPosition : position }
    private var playbackIdentity: String { "\(remote.target?.id ?? "local"):\(remoteItem?.queueItemId ?? playback.currentQueueID?.uuidString ?? ""):\(title)" }
    private var progressControl: some View {
        MusicSeekSlider(value: displayedPosition, duration: duration, identity: playbackIdentity,
            enabled: duration > 0 && (remote.target != nil || playback.isActive), preview: { value in
                seeking = true; hasSeekPreview = true; seekPosition = value
            }, commit: { value in
                seeking = false; hasSeekPreview = true; seekPosition = value; seekGeneration += 1
                if remote.target != nil { remote.send(.cmdSeek(CmdSeek(op: "seek", positionMs: UInt64(max(0, value * 1000))))) }
                else { playback.seek(to: value) }
            }, cancel: { seeking = false; hasSeekPreview = false })
            .frame(height: 44)
    }
    private var localItem: PlaybackManager.QueueEntry? { playback.queue.first(where: { $0.id == playback.currentQueueID }) ?? playback.queue.first }
    private var remoteItem: QueueItem? {
        guard let state = remote.state, let index = state.currentIndex, index < UInt64(state.queue.count) else { return nil }
        return state.queue[Int(index)]
    }
    private var title: String { remote.target == nil ? (playback.title.isEmpty ? localItem?.track.title ?? "Choose something to play" : playback.title) : remoteItem?.title ?? "Ready on this device" }
    private var artist: String { remote.target == nil ? localItem?.track.artistName ?? "" : remoteItem?.artist ?? "" }
    private var albumID: String? { remote.target == nil ? localItem?.track.albumId : nil }
    private var playing: Bool { remote.target == nil ? playback.isPlaying : remote.state?.status == .playing }
    private var position: Double { remote.target == nil ? playback.position : Double(remote.state?.positionMs ?? 0) / 1000 }
    private var duration: Double { remote.target == nil ? playback.duration : Double(remoteItem?.durationMs ?? 0) / 1000 }
    var body: some View {
        Group { if fullScreen { fullPlayer } else { miniPlayer } }
            .tint(Color("AccentColor"))
            .sheet(isPresented: $expanded) { PlaybackControls(fullScreen: true) }
            .sheet(isPresented: $queueOpen) { PlaybackQueueView().appControls() }
            .sheet(isPresented: $devicesOpen) { DevicePicker() }
            .onChange(of: playbackIdentity) { _ in seeking = false; hasSeekPreview = false; seekGeneration += 1 }
            .onChange(of: position) { value in if !seeking && abs(value - seekPosition) < 1 { hasSeekPreview = false } }
            .onChange(of: remote.error) { error in if error != nil && !seeking { hasSeekPreview = false } }
            .task(id: seekGeneration) {
                guard hasSeekPreview, !seeking else { return }
                do { try await Task.sleep(nanoseconds: 3_000_000_000) } catch { return }
                if !seeking { hasSeekPreview = false }
            }
    }
    private var miniPlayer: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                Button { expanded = true } label: {
                    HStack(spacing: 10) {
                        PlayerArtwork(albumID: albumID, size: 42)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(title).font(.subheadline.weight(.semibold)).lineLimit(1).foregroundStyle(Color("BrandText"))
                            Text(artist.isEmpty ? remote.target?.name ?? "This phone" : artist).font(.caption).lineLimit(1).foregroundStyle(Color("BrandMuted"))
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    }.contentShape(Rectangle())
                }.buttonStyle(.plain).accessibilityLabel("Open Now playing: \(title)")
                Button("Change playback device", systemImage: "hifispeaker") { devicesOpen = true }.labelStyle(.iconOnly).buttonStyle(MusicTransportStyle())
                Button(playing ? "Pause" : "Play", systemImage: playing ? "pause.fill" : "play.fill", action: toggle).labelStyle(.iconOnly).buttonStyle(MusicTransportStyle(primary: true))
                Button("Next", systemImage: "forward.end.fill", action: next).labelStyle(.iconOnly).buttonStyle(MusicTransportStyle())
            }.padding(8).background(Color("BrandSurface"), in: RoundedRectangle(cornerRadius: 12))
            HStack(spacing: 8) {
                Text(time(displayedPosition)).layoutPriority(1)
                progressControl
                Text(time(duration)).layoutPriority(1)
            }.font(.caption2.monospacedDigit()).foregroundStyle(Color("BrandMuted")).padding(.horizontal, 8)
        }.padding(.horizontal, 12).padding(.vertical, 4).background(Color("BrandBackground"))
    }
    private var fullPlayer: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 22) {
                    PlayerArtwork(albumID: albumID, size: min(380, UIScreen.main.bounds.width - 48)).frame(maxWidth: .infinity).padding(.vertical, 16)
                    VStack(alignment: .leading, spacing: 8) { Text(title).font(.title.bold()); Text(artist).font(.title3).foregroundStyle(Color("BrandMuted")) }
                    VStack(spacing: 4) {
                        progressControl
                        HStack { Text(time(displayedPosition)); Spacer(); Text(time(duration)) }.font(.caption.monospacedDigit()).foregroundStyle(Color("BrandMuted"))
                    }
                    HStack(spacing: 32) {
                        Spacer()
                        Button("Previous", systemImage: "backward.end.fill") { if remote.target != nil { remote.send(.cmdPrevious(CmdPrevious(op: "previous"))) } else { playback.previousQueued() } }.buttonStyle(MusicTransportStyle())
                        Button(action: toggle) { Image(systemName: playing ? "pause.fill" : "play.fill").frame(width: 76, height: 76) }.buttonStyle(MusicTransportStyle(primary: true)).accessibilityLabel(playing ? "Pause" : "Play")
                        Button("Next", systemImage: "forward.end.fill", action: next).buttonStyle(MusicTransportStyle())
                        Spacer()
                    }.labelStyle(.iconOnly)
                    HStack {
                        Button { devicesOpen = true } label: { Label(remote.target?.name ?? "This phone", systemImage: "hifispeaker").font(.subheadline) }.frame(minHeight: 44)
                        Spacer()
                        Button(remote.target == nil ? "Stop" : "Pause device", systemImage: "stop.fill") { if remote.target != nil { remote.send(.cmdPause(CmdPause(op: "pause"))) } else { playback.stop() } }.labelStyle(.iconOnly).buttonStyle(MusicTransportStyle())
                    }
                    if let error = remote.error, remote.target != nil { HStack { Text(error).font(.caption); Button("Retry") { Task { await remote.refresh() } } } }
                }.padding(.horizontal, 24).padding(.bottom, 24)
            }.background(Color("BrandBackground")).navigationTitle("Now playing").navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button("Close Now playing", systemImage: "chevron.down") { dismiss() } }
                    ToolbarItem(placement: .primaryAction) { Button("Queue", systemImage: "list.bullet") { queueOpen = true } }
                }
        }
    }
    private func toggle() {
        if remote.target != nil { remote.send(playing ? .cmdPause(CmdPause(op: "pause")) : .cmdPlay(CmdPlay(op: "play"))) }
        else if playback.isPlaying { playback.pause() }
        else if !playback.isActive, let entry = playback.queue.first { playback.playQueued(entry) }
        else { playback.resume() }
    }
    private func next() { if remote.target != nil { remote.send(.cmdNext(CmdNext(op: "next"))) } else { playback.nextQueued() } }
    private func time(_ seconds: TimeInterval) -> String { let value = max(0, Int(seconds)); return String(format: "%d:%02d", value / 60, value % 60) }
}

struct PlaybackQueueView: View {
    var embedded = false
    @EnvironmentObject private var remote: RemotePlayback
    @EnvironmentObject private var playback: PlaybackManager
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                Section("\(remote.target?.name ?? "This phone") · \(remote.target == nil ? playback.queue.count : remote.state?.queue.count ?? 0) tracks") {
                    if remote.target != nil {
                        if let error = remote.error { Text(error); Button("Retry") { Task { await remote.refresh() } } }
                        ForEach(Array((remote.state?.queue ?? []).enumerated()), id: \.element.queueItemId) { index, item in
                            queueRow(title: item.title ?? item.trackId, artist: item.artist ?? "", album: nil, current: remote.state?.currentIndex == UInt64(index), play: { remote.send(.cmdPlay(CmdPlay(op: "play", queueItemId: item.queueItemId))) }, up: { remote.move(index, to: index - 1) }, down: { remote.move(index, to: index + 1) }, remove: { remote.send(.cmdRemoveItem(CmdRemoveItem(op: "remove-item", queueItemId: item.queueItemId))) }, first: index == 0, last: index + 1 == remote.state?.queue.count)
                        }.onMove { indices, destination in if let index = indices.first { remote.move(index, to: destination > index ? destination - 1 : destination) } }
                         .onDelete { indices in for index in indices { if let item = remote.state?.queue[index] { remote.send(.cmdRemoveItem(CmdRemoveItem(op: "remove-item", queueItemId: item.queueItemId))) } } }
                    } else {
                        if playback.queue.isEmpty { Text("Add music or an audiobook to start your queue.").foregroundStyle(.secondary) }
                        ForEach(Array(playback.queue.enumerated()), id: \.element.id) { index, entry in
                            queueRow(title: entry.track.title, artist: entry.track.artistName ?? "", album: entry.track.albumId, current: entry.id == playback.currentQueueID, play: { playback.playQueued(entry) }, up: { playback.moveQueued(from: IndexSet(integer: index), to: index - 1) }, down: { playback.moveQueued(from: IndexSet(integer: index), to: index + 2) }, remove: { playback.removeQueued(index) }, first: index == 0, last: index + 1 == playback.queue.count)
                        }.onDelete { indices in for index in indices.sorted(by: >) { playback.removeQueued(index) } }
                         .onMove { playback.moveQueued(from: $0, to: $1) }
                    }
                }
            }.listStyle(.plain).scrollContentBackground(.hidden).background(Color("BrandBackground"))
                .navigationTitle("Your queue")
                .toolbar {
                    if !embedded { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } } }
                    ToolbarItem(placement: .primaryAction) { EditButton() }
                    ToolbarItem(placement: .primaryAction) { Button("Undo queue change", systemImage: "arrow.uturn.backward") { if remote.target != nil { remote.send(.cmdUndo(CmdUndo(op: "undo"))) } else { playback.undoQueueChange() } } }
                }
        }.tint(Color("AccentColor"))
    }
    private func queueRow(title: String, artist: String, album: String?, current: Bool, play: @escaping () -> Void, up: @escaping () -> Void, down: @escaping () -> Void, remove: @escaping () -> Void, first: Bool, last: Bool) -> some View {
        HStack(spacing: 12) {
            Button(action: play) {
                HStack(spacing: 12) {
                    PlayerArtwork(albumID: album, size: 48)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(title).font(.subheadline.weight(.semibold)).lineLimit(1).foregroundStyle(current ? Color("AccentColor") : Color("BrandText"))
                        Text(artist).font(.caption).lineLimit(1).foregroundStyle(Color("BrandMuted"))
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }.contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityLabel("Play \(title)")
            Menu {
                Button("Move up", systemImage: "arrow.up", action: up).disabled(first)
                Button("Move down", systemImage: "arrow.down", action: down).disabled(last)
                Button("Remove from queue", systemImage: "minus.circle", role: .destructive, action: remove)
            } label: { Image(systemName: "ellipsis").frame(width: 44, height: 44) }.accessibilityLabel("More for \(title)")
        }.padding(.vertical, 4).listRowBackground(Color.clear).listRowSeparator(.hidden)
    }
}

/// Keeps a shared device queue independent of the current library screen.
@MainActor
final class RemotePlayback: ObservableObject {
    @Published private(set) var target: Player?
    @Published private(set) var state: PlayerState?
    @Published private(set) var error: String?
    private var client: PlayerAsyncClient?
    private var profileID: UUID?
    private var token: String?
    private var generation = UUID()
    private var pollTask: Task<Void, Never>?
    private var commandTask: Task<Void, Never>?
    private var active = true

    func configure(profile: ServerProfile?, token: String?) {
        guard profileID != profile?.id || self.token != token else { return }
        profileID = profile?.id; self.token = token
        generation = UUID(); pollTask?.cancel(); commandTask?.cancel()
        target = nil; state = nil; error = nil
        client = profile.flatMap { try? PlayerAsyncClient(transport: CSILHTTPTransport(profile: $0, token: token)) }
    }
    func devices() async throws -> [Player] {
        guard let client else { throw URLError(.userAuthenticationRequired) }
        return try await client.listPlayers(ListPlayersRequest(kind: .shared)).players
    }
    func select(_ player: Player?) {
        generation = UUID(); pollTask?.cancel(); commandTask?.cancel()
        target = player; state = nil; error = nil; startPolling()
    }
    func setActive(_ value: Bool) { active = value; if value { startPolling() } else { pollTask?.cancel(); pollTask = nil } }
    private func startPolling() {
        pollTask?.cancel()
        guard active, target != nil else { return }
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                await self?.refresh()
                do { try await Task.sleep(nanoseconds: 2_000_000_000) } catch { return }
            }
        }
    }
    func refresh() async {
        guard let target, let client else { return }
        let epoch = generation
        do {
            let value = try await client.getState(SubscribeRequest(playerId: target.id))
            guard !Task.isCancelled, generation == epoch else { return }
            if value.revision >= (state?.revision ?? 0) { state = value }
            error = nil
        } catch {
            guard !Task.isCancelled, generation == epoch else { return }
            self.error = error.localizedDescription
        }
    }
    func send(_ command: PlayerCommand) {
        guard let target, let client else { return }
        let epoch = generation, previous = commandTask
        commandTask = Task { [weak self] in
            await previous?.value
            guard let self, !Task.isCancelled, self.generation == epoch else { return }
            do {
                let result = try await client.control(CommandRequest(playerId: target.id, command: command))
                guard self.generation == epoch else { return }
                if result.revision >= (self.state?.revision ?? 0) { self.state = result }
                self.error = nil
            } catch { if self.generation == epoch { self.error = error.localizedDescription } }
        }
    }
    func enqueue(_ track: Track) { send(.cmdEnqueue(CmdEnqueue(op: "enqueue", trackIds: [track.id]))) }
    func play(_ track: Track) { send(.cmdReplaceAndPlay(CmdReplaceAndPlay(op: "replace-and-play", trackIds: [track.id]))) }
    func move(_ index: Int, to: Int) {
        guard let queue = state?.queue, queue.indices.contains(index), queue.indices.contains(to) else { return }
        let item = queue[index], remaining = queue.filter { $0.queueItemId != item.queueItemId }
        send(.cmdMoveItem(CmdMoveItem(op: "move-item", queueItemId: item.queueItemId, beforeQueueItemId: to < remaining.count ? remaining[to].queueItemId : nil)))
    }
}

struct DevicePicker: View {
    @EnvironmentObject private var remote: RemotePlayback
    @EnvironmentObject private var playback: PlaybackManager
    @Environment(\.dismiss) private var dismiss
    @State private var devices: [Player] = []
    @State private var loading = true
    @State private var error: String?
    var body: some View {
        NavigationStack {
            List {
                Button { remote.select(nil); dismiss() } label: {
                    Label("This phone", systemImage: remote.target == nil ? "checkmark.circle.fill" : "iphone")
                }
                if loading { ProgressView("Loading devices…") }
                if let error { Section { Text(error); Button("Retry") { Task { await load() } } } }
                ForEach(devices, id: \.id) { device in
                    Button {
                        playback.pause(); remote.select(device); dismiss()
                    } label: {
                        VStack(alignment: .leading, spacing: 5) {
                            Label(device.name, systemImage: device.id == remote.target?.id ? "checkmark.circle.fill" : "hifispeaker")
                            if device.audioBlocked == true { Text("Enable audio on this device first").font(.caption) }
                        }
                    }
                }
                if !loading && error == nil && devices.isEmpty { Text("No shared devices. Share a browser or connect a satellite.").foregroundStyle(.secondary) }
            }.navigationTitle("Play on a device")
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } } }
        }.task { await load() }
    }
    private func load() async {
        loading = true; error = nil
        do { devices = try await remote.devices() } catch { self.error = error.localizedDescription }
        loading = false
    }
}
