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

struct PlaybackControls: View {
    @EnvironmentObject private var playback: PlaybackManager
    @EnvironmentObject private var remote: RemotePlayback
    @State private var queueOpen = false
    @State private var devicesOpen = false
    @State private var seeking = false
    @State private var seekPosition = 0.0
    private var remoteItem: QueueItem? {
        guard let state = remote.state, let index = state.currentIndex, index < UInt64(state.queue.count) else { return nil }
        return state.queue[Int(index)]
    }
    private var playing: Bool { remote.target == nil ? playback.isPlaying : remote.state?.status == .playing }
    private var position: Double { remote.target == nil ? playback.position : Double(remote.state?.positionMs ?? 0) / 1000 }
    private var duration: Double { remote.target == nil ? playback.duration : Double(remoteItem?.durationMs ?? 0) / 1000 }
    var body: some View {
        VStack(spacing: 4) {
            HStack(spacing: 12) {
                Image(systemName: "music.note").frame(width: 32, height: 32).background(Color.accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 7))
                Text(remote.target == nil ? (playback.title.isEmpty ? "Ready when you are" : playback.title) : (remoteItem?.title ?? "Ready on this device"))
                    .font(.subheadline.weight(.semibold)).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                Button(playing ? "Pause" : "Play", systemImage: playing ? "pause.fill" : "play.fill") {
                    if remote.target != nil { remote.send(playing ? .cmdPause(CmdPause(op: "pause")) : .cmdPlay(CmdPlay(op: "play"))) }
                    else if playback.isPlaying { playback.pause() }
                    else if !playback.isActive, let entry = playback.queue.first { playback.playQueued(entry) }
                    else { playback.resume() }
                }.labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44)
                Button("Next", systemImage: "forward.end.fill") {
                    if remote.target != nil { remote.send(.cmdNext(CmdNext(op: "next"))) } else { playback.nextQueued() }
                }.labelStyle(.iconOnly).frame(minWidth: 44, minHeight: 44)
            }
            HStack(spacing: 8) {
                Text(time(position)).font(.caption2.monospacedDigit())
                Slider(value: Binding(get: { seeking ? seekPosition : position }, set: { seekPosition = $0 }), in: 0...max(1, duration), onEditingChanged: { editing in
                    if editing { seekPosition = position; seeking = true }
                    else { seeking = false; if remote.target != nil { remote.send(.cmdSeek(CmdSeek(op: "seek", positionMs: UInt64(max(0, seekPosition * 1000))))) } else { playback.seek(to: seekPosition) } }
                }).accessibilityLabel("Playback position").disabled(duration <= 0)
                Text(time(duration)).font(.caption2.monospacedDigit())
            }.frame(height: 22)
            HStack {
                Button { devicesOpen = true } label: { Label(remote.target?.name ?? "This phone", systemImage: "hifispeaker").lineLimit(1) }
                    .accessibilityLabel("Playback device: \(remote.target?.name ?? "This phone"). Change device")
                Spacer()
                Button("Queue", systemImage: "list.bullet") { queueOpen = true }
                Menu("More", systemImage: "ellipsis") {
                    Button("Previous") { if remote.target != nil { remote.send(.cmdPrevious(CmdPrevious(op: "previous"))) } else { playback.previousQueued() } }
                    Button(remote.target == nil ? "Stop" : "Pause device") { if remote.target != nil { remote.send(.cmdPause(CmdPause(op: "pause"))) } else { playback.stop() } }
                }.labelStyle(.iconOnly)
            }.font(.caption).frame(minHeight: 36)
            if let error = remote.error, remote.target != nil {
                HStack { Text(error).font(.caption); Button("Retry") { Task { await remote.refresh() } } }
            }
        }.padding(.horizontal, 16).padding(.top, 6).background(Color("BrandSurface")).tint(Color("AccentColor"))
        .sheet(isPresented: $queueOpen) { PlaybackQueueView() }
        .sheet(isPresented: $devicesOpen) { DevicePicker() }
    }
    private func time(_ seconds: TimeInterval) -> String { let value = max(0, Int(seconds)); return String(format: "%d:%02d", value / 60, value % 60) }
}


struct PlaybackQueueView: View {
    @EnvironmentObject private var remote: RemotePlayback
    @EnvironmentObject private var playback: PlaybackManager
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
                if let target = remote.target {
                    Section(target.name) {
                        if let error = remote.error { Text(error); Button("Retry") { Task { await remote.refresh() } } }
                        ForEach(Array((remote.state?.queue ?? []).enumerated()), id: \.element.queueItemId) { index, item in
                            HStack {
                                Text(item.title ?? item.trackId).lineLimit(2)
                                Spacer()
                                Button("Play", systemImage: "play.fill") { remote.send(.cmdPlay(CmdPlay(op: "play", queueItemId: item.queueItemId))) }.labelStyle(.iconOnly)
                                Button("Move up", systemImage: "arrow.up") { remote.move(index, to: index - 1) }.labelStyle(.iconOnly).disabled(index == 0)
                                Button("Move down", systemImage: "arrow.down") { remote.move(index, to: index + 1) }.labelStyle(.iconOnly).disabled(index + 1 == remote.state?.queue.count)
                                Button("Remove", systemImage: "xmark") { remote.send(.cmdRemoveItem(CmdRemoveItem(op: "remove-item", queueItemId: item.queueItemId))) }.labelStyle(.iconOnly)
                            }.buttonStyle(.borderless)
                        }
                    }
                } else {
                if playback.queue.isEmpty { Text("Your queue is empty. Add music or an audiobook from the library.") }
                ForEach(playback.queue) { entry in
                    HStack {
                        if entry.id == playback.currentQueueID { Image(systemName: "waveform").foregroundStyle(Color("BrandSignal")) }
                        Text(entry.track.title)
                        Spacer()
                        Button("Play") { playback.playQueued(entry) }
                    }
                }.onDelete { indices in for index in indices.sorted(by: >) { playback.removeQueued(index) } }
                 .onMove { playback.moveQueued(from: $0, to: $1) }
                }
            }.scrollContentBackground(.hidden).background(Color("BrandBackground"))
            .navigationTitle("Your queue")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Back to browsing") { dismiss() } }
                ToolbarItem(placement: .primaryAction) { EditButton() }
                ToolbarItem(placement: .bottomBar) { Button("Undo queue change") { if remote.target != nil { remote.send(.cmdUndo(CmdUndo(op: "undo"))) } else { playback.undoQueueChange() } } }
            }
        }.tint(Color("AccentColor"))
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
