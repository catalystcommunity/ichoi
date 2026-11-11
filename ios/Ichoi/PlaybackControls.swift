import SwiftUI

struct AppControls: ViewModifier {
    @EnvironmentObject private var messages: AppMessages
    @EnvironmentObject private var playback: PlaybackManager

    func body(content: Content) -> some View {
        content
            .safeAreaInset(edge: .bottom) {
                if playback.isActive { PlaybackControls() }
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
    @State private var queueOpen = false
    @State private var seeking = false
    @State private var seekPosition = 0.0
    var body: some View {
        VStack(spacing: 8) {
            Text(playback.title).font(.headline).lineLimit(2)
            Text(playback.server).font(.caption).lineLimit(1)
            if playback.isLoading {
                ProgressView("Loading…")
            } else {
                Slider(value: Binding(
                    get: { seeking ? seekPosition : playback.position },
                    set: { seekPosition = $0 }
                ), in: 0...max(1, playback.duration), onEditingChanged: { editing in
                    if editing { seekPosition = playback.position; seeking = true }
                    else { seeking = false; playback.seek(to: seekPosition) }
                }).accessibilityLabel("Playback position")
                Text("\(time(playback.position)) / \(time(playback.duration))").font(.caption.monospacedDigit())
            }
            HStack {
                Button("Queue", systemImage: "list.bullet") { queueOpen = true }
                Spacer()
                Button(playback.isPlaying ? "Pause" : "Resume") {
                    if playback.isPlaying { playback.pause() } else { playback.resume() }
                }.disabled(playback.isLoading)
                Spacer()
                Button("Stop") { playback.stop() }
            }.buttonStyle(.bordered)
        }.padding().background(Color("BrandSurface"))
        .sheet(isPresented: $queueOpen) { PlaybackQueueView() }
    }

    private func time(_ seconds: TimeInterval) -> String {
        let value = max(0, Int(seconds))
        return String(format: "%d:%02d", value / 60, value % 60)
    }
}


struct PlaybackQueueView: View {
    @EnvironmentObject private var playback: PlaybackManager
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            List {
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
            }.scrollContentBackground(.hidden).background(Color("BrandBackground"))
            .navigationTitle("Now playing")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Back to browsing") { dismiss() } }
                ToolbarItem(placement: .primaryAction) { EditButton() }
                ToolbarItem(placement: .bottomBar) { Button("Undo queue change") { playback.undoQueueChange() } }
            }
        }.tint(Color("AccentColor"))
    }
}
