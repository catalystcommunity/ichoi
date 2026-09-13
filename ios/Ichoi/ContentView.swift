import SwiftUI

struct ContentView: View {
    @EnvironmentObject private var model: AppModel
    var body: some View {
        Group {
            if !model.hasAcceptedTerms { TermsView() }
            else if model.profile == nil { ServerConnectionView() }
            else { LibraryView() }
        }
        .alert("Ichoi", isPresented: Binding(get: { model.message != nil }, set: { if !$0 { model.message = nil } })) {
            Button("OK", role: .cancel) { model.message = nil }
        } message: { Text(model.message ?? "") }
        .task { await model.restoreSession() }
    }
}

struct TermsView: View {
    @EnvironmentObject private var model: AppModel
    var body: some View {
        VStack(spacing: 20) {
            Text("Welcome to Ichoi").font(.largeTitle.bold())
            Text("Ichoi is a free, open-source player for a server that you choose. The server operator controls its library and account data.").multilineTextAlignment(.center)
            HStack { Link("Privacy policy", destination: IchoiConfig.privacyURL); Link("Terms", destination: IchoiConfig.termsURL) }
            Button("Accept terms and continue") { model.acceptTerms() }.buttonStyle(.borderedProminent)
        }.padding()
    }
}

struct ServerConnectionView: View {
    @EnvironmentObject private var model: AppModel
    @State private var name = ""
    @State private var url = "https://"
    @State private var pins = ""
    @State private var privateHTTP = false
    var body: some View {
        Form {
            Section("Connect to an Ichoi server") {
                TextField("Name (optional)", text: $name)
                TextField("https://server.example", text: $url).textInputAutocapitalization(.never).autocorrectionDisabled()
                TextField("HTTPS TLS SHA-256 fingerprints (optional)", text: $pins).textInputAutocapitalization(.never).autocorrectionDisabled()
                Toggle("Allow HTTP on my private network", isOn: $privateHTTP)
                Text("Use plain HTTP only for a private address, after you confirm this choice. Public HTTP is rejected.").font(.footnote)
                Button("Add my server") { model.addProfile(name: name, urlString: url, allowPrivateHTTP: privateHTTP, pins: pins.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }) }.buttonStyle(.borderedProminent)
            }
        }.navigationTitle("Ichoi")
    }
}

struct LibraryView: View {
    @EnvironmentObject private var model: AppModel
    @State private var albums: [Album] = []
    @State private var playlists: [Playlist] = []
    @State private var tracks: [Track] = []
    @State private var showSettings = false
    @State private var showReport = false
    @State private var reportID = ""
    @State private var reportTarget: ContentReportTargetType = .playlist
    @State private var error: String?
    var body: some View {
        NavigationStack {
            List {
                Section("Music") {
                    if albums.isEmpty { Text("No albums loaded. Pull to refresh.").foregroundStyle(.secondary) }
                    ForEach(albums, id: \.id) { album in
                        Button(album.title) { Task { await openAlbum(album.id) } }
                    }
                }
                Section("Playlists") {
                    if playlists.isEmpty { Text("No playlists are available.").foregroundStyle(.secondary) }
                    ForEach(playlists, id: \.id) { playlist in
                        HStack {
                            Button(playlist.name) { Task { await openPlaylist(playlist.id) } }
                            Spacer()
                            if model.sessionManager?.session != nil {
                                Menu("Report") {
                                    Button("Report playlist") { beginReport(.playlist, id: playlist.id) }
                                    if let owner = playlist.owner {
                                        Button("Report user") { beginReport(.account, id: owner) }
                                    }
                                }
                            }
                        }
                    }
                }
                Section("Tracks") {
                    if tracks.isEmpty { Text("Select an album or playlist.").foregroundStyle(.secondary) }
                    ForEach(tracks, id: \.id) { track in
                        TrackRow(track: track, profile: model.profile, bearerToken: model.token())
                    }
                }
            }
            .refreshable { await load() }
            .navigationTitle(model.profile?.name ?? "Ichoi")
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Menu {
                        Button("Settings", systemImage: "gear") { showSettings = true }
                    } label: { Image(systemName: "ellipsis.circle") }
                }
            }
            .task { await load() }
            .sheet(isPresented: $showSettings) { SettingsView() }
            .sheet(isPresented: $showReport) { ReportView(targetType: reportTarget, targetID: reportID) }
            .alert("Could not connect", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) { Button("OK", role: .cancel) {} } message: { Text(error ?? "") }
        }
    }
    private func beginReport(_ target: ContentReportTargetType, id: String) {
        reportTarget = target
        reportID = id
        showReport = true
    }
    private func client() throws -> LibraryAsyncClient {
        guard let profile = model.profile else { throw ServerURLPolicyError.invalidHost }
        let transport = try CSILHTTPTransport(profile: profile, token: model.token())
        return LibraryAsyncClient(transport: transport)
    }
    private func load() async {
        do {
            let api = try client()
            albums = (try await api.listAlbums(BrowseRequest(library: .music))).albums
            playlists = (try await api.listPlaylists(BrowseRequest(library: .music))).playlists
        } catch { error = "Check the selected server URL and try again." }
    }
    private func openAlbum(_ id: String) async {
        do { tracks = (try await client().getAlbum(AlbumRequest(albumId: id))).tracks }
        catch { error = "The album could not be loaded from the selected server." }
    }
    private func openPlaylist(_ id: String) async {
        do { tracks = (try await client().getPlaylist(PlaylistRequest(playlistId: id))).tracks }
        catch { error = "The playlist could not be loaded from the selected server." }
    }
}

struct TrackRow: View {
    let track: Track
    let profile: ServerProfile?
    let bearerToken: String?
    @EnvironmentObject private var playback: PlaybackManager
    var body: some View {
        HStack {
            Image(systemName: "music.note"); Text(track.title); Spacer()
            Button("Play") {
                guard let profile else { return }
                // Streaming is transient. The app has no persistent media-file action.
                let mediaURL = profile.url.appendingPathComponent("media").appendingPathComponent(track.id)
                Task { try? await playback.play(profile: profile, url: mediaURL, title: track.title, bearerToken: bearerToken) }
            }.foregroundStyle(.tint)
        }.accessibilityElement(children: .combine).accessibilityLabel("Play \(track.title)")
    }
}
