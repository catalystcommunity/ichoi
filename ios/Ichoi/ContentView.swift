import SwiftUI
import PhotosUI
import ImageIO

struct ContentView: View {
    @EnvironmentObject private var model: AppModel
    @EnvironmentObject private var remote: RemotePlayback
    @Environment(\.scenePhase) private var scenePhase
    @State private var destination = 0
    var body: some View {
        Group {
            if !model.hasAcceptedTerms { TermsView().appControls() }
            else if model.profile == nil { ServerConnectionView().appControls() }
            else {
                TabView(selection: $destination) {
                    LibraryView(onSearch: { destination = 1 }).appControls().tabItem { Label("Library", systemImage: "square.stack.fill") }.tag(0)
                    LibraryView(searchOnly: true).appControls().tabItem { Label("Search", systemImage: "magnifyingglass") }.tag(1)
                    PlaybackQueueView(embedded: true).appControls().tabItem { Label("Queue", systemImage: "list.bullet") }.tag(2)
                }
            }
        }
        .tint(Color("AccentColor"))
        .background(Color("BrandBackground"))
        .onChange(of: scenePhase) { model.messages.setActive($0 == .active); remote.setActive($0 == .active) }
        .task(id: [model.profile?.id.uuidString, model.sessionManager?.session?.accountId]) { remote.configure(profile: model.profile, token: model.token()) }
        .task { await model.restoreSession() }
    }
}

struct TermsView: View {
    @EnvironmentObject private var model: AppModel
    var body: some View {
        VStack(spacing: 20) {
            Image("BrandMark").resizable().scaledToFit().frame(width: 88, height: 88).accessibilityHidden(true)
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
    @State private var url = ""
    @State private var privateHTTP = false
    var body: some View {
        Form {
            Section("Connect to an Ichoi server") {
                TextField("Name (optional)", text: $name)
                TextField("server.example", text: $url).textInputAutocapitalization(.never).autocorrectionDisabled()
                Toggle("Allow HTTP on my private network", isOn: $privateHTTP)
                Text("Use plain HTTP only for a private address, after you confirm this choice. Public HTTP is rejected.").font(.footnote)
                Button("Add my server") { model.addProfile(name: name, urlString: url, allowPrivateHTTP: privateHTTP) }.buttonStyle(.borderedProminent)
            }
            Section("Try Ichoi") {
                Text("The public demo needs no login for browsing and playback.").font(.footnote)
                Button("Connect to demo server") { model.connectDemo() }
            }
            MessageDurationSetting()
        }.scrollContentBackground(.hidden).background(Color("BrandBackground")).navigationTitle("Ichoi")
    }
}

struct LibraryView: View {
    var searchOnly = false
    var onSearch: () -> Void = {}
    @Environment(\.colorScheme) private var colorScheme
    @EnvironmentObject private var model: AppModel
    @State private var browse = "Albums"
    @State private var loadGeneration = UUID()
    @State private var query = ""
    @State private var searchResults: SearchResponse?
    @State private var searchLoading = false
    @State private var showQueue = false
    @State private var showDevices = false
    @State private var showUpdates = false
    @State private var showTracks = false
    @State private var detailAlbums: [Album] = []
    @State private var detailAlbum: Album?
    @EnvironmentObject private var playback: PlaybackManager
    @EnvironmentObject private var remote: RemotePlayback
    @State private var albums: [Album] = []
    @State private var playlists: [Playlist] = []
    @State private var artists: [Artist] = []
    @State private var library: Library = .music
    @State private var albumTotal: UInt64 = 0
    @State private var artistTotal: UInt64 = 0
    @State private var tracks: [Track] = []
    @State private var showSettings = false
    @State private var showReport = false
    @State private var libraryLoading = false
    @State private var libraryError: String?
    @State private var signInRequired = false
    @State private var reportID = ""
    @State private var reportTarget: ContentReportTargetType = .playlist
    var body: some View {
        NavigationStack {
            List {
                if let libraryError {
                    Section(signInRequired ? "Sign in to your library" : "Could not load your library") {
                        Text(libraryError)
                        if signInRequired {
                            Button("Sign in with LinkKeys") { showSettings = true }.buttonStyle(.borderedProminent)
                        } else {
                            Button("Retry") { Task { await load() } }
                        }
                    }
                }
                if libraryLoading { ProgressView("Loading your library…") }
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        libraryChip("Music", selected: library == .music && browse == "Albums") { library = .music; browse = "Albums" }
                        libraryChip("Audiobooks", selected: library == .audiobook && browse == "Albums") { library = .audiobook; browse = "Albums" }
                        if !searchOnly {
                            libraryChip("Artists", selected: browse == "Artists") { browse = "Artists" }
                            libraryChip("Playlists", selected: browse == "Playlists") { browse = "Playlists" }
                        }
                    }.padding(.vertical, 4)
                }.listRowBackground(Color.clear).listRowSeparator(.hidden)
                if !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    Section("Search results") {
                        if searchLoading { ProgressView("Searching…") }
                        if let result = searchResults {
                            ForEach(result.albums, id: \.id) { album in
                                ArtworkRow(kind: .album, id: album.id, title: album.title, subtitle: album.artistName ?? "") { Task { await openAlbum(album.id) } }
                            }
                            ForEach(result.artists, id: \.id) { artist in
                                Button(artist.name) { Task { await openArtist(artist.id) } }
                            }
                            ForEach(result.tracks, id: \.id) { track in TrackRow(track: track, profile: model.profile, bearerToken: model.token()) }
                            if result.albums.isEmpty && result.artists.isEmpty && result.tracks.isEmpty { Text("No results. Try another search.") }
                        }
                    }
                } else if searchOnly {
                    Text("Find your next listen").font(.title2.weight(.semibold)).padding(.top, 30).listRowBackground(Color.clear).listRowSeparator(.hidden)
                } else {
                if browse == "Albums" {
                Section(library == .music ? "Albums" : "Audiobooks") {
                    if albums.isEmpty && !libraryLoading && libraryError == nil { Text("No albums in this library.").foregroundStyle(.secondary) }
                    LazyVGrid(columns: [GridItem(.flexible(), alignment: .top), GridItem(.flexible(), alignment: .top)], alignment: .leading, spacing: 22) {
                        ForEach(albums, id: \.id) { album in
                            ArtworkRow(kind: .album, id: album.id, title: album.title, subtitle: album.artistName ?? "", tile: true) { Task { await openAlbum(album.id) } }
                        }
                    }.listRowBackground(Color.clear).listRowSeparator(.hidden)
                    if UInt64(albums.count) < albumTotal {
                        Button("Load more albums") { Task { await loadMoreAlbums() } }
                    }
                }
                }
                if browse == "Artists" {
                Section("Artists") {
                    ForEach(artists, id: \.id) { artist in
                        ArtworkRow(kind: .artist, id: artist.id, title: artist.name, subtitle: "\(artist.albumCount) albums") {
                            Task { await openArtist(artist.id) }
                        }
                    }
                    if UInt64(artists.count) < artistTotal {
                        Button("Load more artists") { Task { await loadMoreArtists() } }
                    }
                }
                }
                if browse == "Playlists" {
                Section("Playlists") {
                    if playlists.isEmpty { Text("No playlists are available.").foregroundStyle(.secondary) }
                    ForEach(playlists, id: \.id) { playlist in
                        HStack {
                            Button(playlist.name) { Task { await openPlaylist(playlist.id) } }
                            Spacer()
                            if model.sessionManager?.session != nil {
                                Menu("Actions") {
                                    Button("Report playlist") { beginReport(.playlist, id: playlist.id) }
                                    if let owner = playlist.owner {
                                        Button("Report user") { beginReport(.account, id: owner) }
                                        if owner != model.sessionManager?.session?.accountId {
                                            Button("Block user", role: .destructive) {
                                                Task { await block(owner) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                }
                }
            }
            .listStyle(.plain)
            .modifier(MusicSearch(enabled: searchOnly, query: $query))
            .task(id: query) { await search() }
            .onChange(of: library) { _ in query = ""; searchResults = nil }
            .scrollContentBackground(.hidden)
            .background(Color("BrandBackground"))
            .refreshable { await load() }
            .navigationTitle(searchOnly ? "Search" : "Your library")
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Menu {
                        Button("Music", systemImage: "music.note.list") { library = .music; browse = "Albums"; query = "" }
                        Button("Audiobooks", systemImage: "books.vertical") { library = .audiobook; browse = "Albums"; query = "" }
                        Button("Artists", systemImage: "person.2") { browse = "Artists"; query = "" }
                        Button("Playlists", systemImage: "music.note.list") { browse = "Playlists"; query = "" }
                        Divider()
                        Button("Queue", systemImage: "list.bullet") { showQueue = true }
                        Button("Playback devices", systemImage: "hifispeaker") { showDevices = true }
                        Button("Settings and servers", systemImage: "gear") { showSettings = true }
                        Button("App and updates", systemImage: "arrow.down.circle") { showUpdates = true }
                    } label: { HStack(spacing: 6) { Image(systemName: "line.3.horizontal"); Text("Menu") } }
                }
                if !searchOnly { ToolbarItem(placement: .navigationBarTrailing) { Button("Search", systemImage: "magnifyingglass", action: onSearch) } }
            }
            .task(id: [model.profile?.id.uuidString, model.sessionManager?.session?.accountId, library.rawValue]) {
                albums = []
                playlists = []
                artists = []
                tracks = []
                if !searchOnly { await load() }
            }
            .sheet(isPresented: $showQueue) { PlaybackQueueView().appControls() }
            .sheet(isPresented: $showDevices) { DevicePicker() }
            .sheet(isPresented: $showUpdates) { AppUpdatesView() }
            .sheet(isPresented: $showTracks) {
                NavigationStack {
                    List {
                        if let album = detailAlbum {
                            VStack(alignment: .leading, spacing: 12) {
                                PlayerArtwork(albumID: album.id, size: 210).frame(maxWidth: .infinity).padding(.bottom, 12)
                                Text(album.title).font(.title.bold()).foregroundStyle(Color("BrandText"))
                                Text(album.artistName ?? "").font(.subheadline).foregroundStyle(Color("BrandMuted"))
                                HStack {
                                    Text("\(tracks.count) tracks").font(.caption).foregroundStyle(Color("BrandMuted"))
                                    Spacer()
                                    Button { playAlbum() } label: { Image(systemName: "play.fill").frame(width: 56, height: 56) }
                                        .buttonStyle(MusicTransportStyle(primary: true)).accessibilityLabel("Play album")
                                }
                            }.padding(.vertical, 12).listRowBackground(Color.clear).listRowSeparator(.hidden)
                        }
                        ForEach(detailAlbums, id: \.id) { album in
                            ArtworkRow(kind: .album, id: album.id, title: album.title, subtitle: album.artistName ?? "") { Task { await openAlbum(album.id) } }
                        }
                        ForEach(tracks, id: \.id) { track in TrackRow(track: track, profile: model.profile, bearerToken: model.token()) }
                    }
                    .listStyle(.plain).scrollContentBackground(.hidden).background(Color("BrandBackground"))
                    .navigationTitle(detailAlbum == nil ? "Tracks" : "Album").navigationBarTitleDisplayMode(.inline)
                    .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close album", systemImage: "chevron.down") { showTracks = false }.labelStyle(.iconOnly) } }
                }.appControls()
            }
            .sheet(isPresented: $showSettings, onDismiss: { Task { await load() } }) { SettingsView() }
            .sheet(isPresented: $showReport) { ReportView(targetType: reportTarget, targetID: reportID) }
        }
    }
    private func libraryChip(_ title: String, selected: Bool, action: @escaping () -> Void) -> some View {
        Button(title, action: action).font(.subheadline.weight(.medium)).padding(.horizontal, 18).frame(minHeight: 44)
            .background(selected ? Color("AccentColor") : Color("BrandSurface"), in: Capsule())
            .foregroundStyle(selected ? (colorScheme == .dark ? Color.black : Color.white) : Color("BrandText"))
            .buttonStyle(.plain)
    }
    private func search() async {
        let text = query.trimmingCharacters(in: .whitespacesAndNewlines)
        searchResults = nil
        guard !text.isEmpty else { searchLoading = false; return }
        searchLoading = true
        let profileID = model.profile?.id
        do {
            try await Task.sleep(nanoseconds: 250_000_000)
            let result = try await client().search(SearchRequest(query: text, library: library))
            guard !Task.isCancelled, model.profile?.id == profileID, query.trimmingCharacters(in: .whitespacesAndNewlines) == text else { return }
            searchResults = result; searchLoading = false
        } catch { if !Task.isCancelled { searchLoading = false; model.message = "Search failed. Try again." } }
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
        let profileID = model.profile?.id
        let accountID = model.sessionManager?.session?.accountId
        let generation = UUID(); loadGeneration = generation
        libraryLoading = true
        libraryError = nil
        signInRequired = false
        defer {
            if loadGeneration == generation && model.profile?.id == profileID && model.sessionManager?.session?.accountId == accountID {
                libraryLoading = false
            }
        }
        do {
            let api = try client()
            let selectedLibrary = library
            let loadedAlbums = try await api.listAlbums(BrowseRequest(library: selectedLibrary))
            let loadedArtists = try await api.listArtists(BrowseRequest(library: selectedLibrary))
            let loadedPlaylists = try await allPlaylists(api)
            guard !Task.isCancelled, loadGeneration == generation, model.profile?.id == profileID,
                  model.sessionManager?.session?.accountId == accountID else { return }
            guard library == selectedLibrary else { return }
            albums = loadedAlbums.albums
            albumTotal = loadedAlbums.total
            artists = loadedArtists.artists
            artistTotal = loadedArtists.total
            playlists = loadedPlaylists
        } catch {
            guard !Task.isCancelled, loadGeneration == generation, model.profile?.id == profileID,
                  model.sessionManager?.session?.accountId == accountID else { return }
            signInRequired = (error as? CsilClientError)?.code == 401
            libraryError = signInRequired
                ? "This server requires sign-in. Use your LinkKeys identity to continue."
                : "The library could not be loaded. Check your connection and try again."
        }
    }
    private func loadMoreAlbums() async {
        let profileID = model.profile?.id
        let selectedLibrary = library
        do {
            let page = try await client().listAlbums(BrowseRequest(library: selectedLibrary, offset: UInt64(albums.count)))
            guard model.profile?.id == profileID, library == selectedLibrary else { return }
            let known = Set(albums.map(\.id))
            albums += page.albums.filter { !known.contains($0.id) }
            albumTotal = page.total
        } catch { model.message = "Could not load more albums." }
    }
    private func loadMoreArtists() async {
        let profileID = model.profile?.id
        let selectedLibrary = library
        do {
            let page = try await client().listArtists(BrowseRequest(library: selectedLibrary, offset: UInt64(artists.count)))
            guard model.profile?.id == profileID, library == selectedLibrary else { return }
            let known = Set(artists.map(\.id))
            artists += page.artists.filter { !known.contains($0.id) }
            artistTotal = page.total
        } catch { model.message = "Could not load more artists." }
    }
    private func openArtist(_ id: String) async {
        let profileID = model.profile?.id
        let selectedLibrary = library
        do {
            let detail = try await client().getArtist(ArtistRequest(artistId: id, library: selectedLibrary))
            guard model.profile?.id == profileID, library == selectedLibrary else { return }
            detailAlbum = nil
            detailAlbums = detail.albums
            tracks = []
            showTracks = true
        } catch { model.message = "Could not load this artist." }
    }

    /// Loads every page, so no visible playlist is cut off.
    private func allPlaylists(_ api: LibraryAsyncClient) async throws -> [Playlist] {
        var playlists: [Playlist] = []
        while true {
            let page = try await api.listPlaylists(
                BrowseRequest(library: .music, offset: UInt64(playlists.count), limit: 1_000))
            playlists += page.playlists
            if page.playlists.isEmpty || UInt64(playlists.count) >= page.total { return playlists }
        }
    }
    private func openAlbum(_ id: String) async {
        do {
            let profileID = model.profile?.id
            let result = try await client().getAlbum(AlbumRequest(albumId: id))
            guard model.profile?.id == profileID else { return }
            tracks = result.tracks.map { track in
                Track(id: track.id, library: track.library, title: track.title,
                    artistId: track.artistId ?? result.album.artistId, artistName: track.artistName ?? result.album.artistName,
                    albumId: track.albumId ?? result.album.id, albumTitle: track.albumTitle ?? result.album.title,
                    trackNo: track.trackNo, discNo: track.discNo, durationMs: track.durationMs, codec: track.codec,
                    bitrateKbps: track.bitrateKbps, sampleRate: track.sampleRate, channels: track.channels,
                    bitDepth: track.bitDepth, rootRelativePath: track.rootRelativePath, contentHash: track.contentHash)
            }; detailAlbum = result.album; detailAlbums = []; showTracks = true
        }
        catch { model.message = "The album could not be loaded from the selected server." }
    }
    private func playAlbum() {
        guard let first = tracks.first, let profile = model.profile else { return }
        if remote.target != nil {
            remote.play(first)
            for track in tracks.dropFirst() { remote.enqueue(track) }
        } else {
            let start = playback.queue.count
            for track in tracks { playback.enqueue(track, profile: profile, token: model.token()) }
            playback.playQueued(playback.queue[start])
        }
    }
    private func openPlaylist(_ id: String) async {
        do {
            let profileID = model.profile?.id
            let result = try await client().getPlaylist(PlaylistRequest(playlistId: id))
            guard model.profile?.id == profileID else { return }
            tracks = result.tracks; detailAlbum = nil; detailAlbums = []; showTracks = true
        }
        catch { model.message = "The playlist could not be loaded from the selected server." }
    }
    private func block(_ accountID: String) async {
        guard let profile = model.profile else { return }
        do {
            try await ComplianceValidation.changeAccountBlock(
                profileID: profile.id.uuidString,
                accountID: accountID,
                request: { _, target in
                    _ = try await client().blockAccount(BlockAccountRequest(accountId: target))
                },
                refresh: { selectedProfileID in
                    guard model.profile?.id.uuidString == selectedProfileID else { return }
                    playlists = try await allPlaylists(client())
                }
            )
            guard model.profile?.id == profile.id else { return }
            tracks = []
        } catch {
            guard model.profile?.id == profile.id else { return }
            model.message = "The account was not blocked on the selected server."
        }
    }
}

struct MusicSearch: ViewModifier {
    let enabled: Bool
    @Binding var query: String
    @ViewBuilder func body(content: Content) -> some View {
        if enabled { content.searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Artists, albums, and tracks") }
        else { content }
    }
}

struct TrackRow: View {
    let track: Track
    let profile: ServerProfile?
    let bearerToken: String?
    @EnvironmentObject private var playback: PlaybackManager
    @EnvironmentObject private var remote: RemotePlayback
    var body: some View {
        HStack(spacing: 10) {
            Button(action: play) {
                HStack(spacing: 12) {
                    PlayerArtwork(albumID: track.albumId, size: 48)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(track.title).font(.subheadline.weight(.semibold)).lineLimit(1).foregroundStyle(Color("BrandText"))
                        Text(track.artistName ?? track.albumTitle ?? "").font(.caption).lineLimit(1).foregroundStyle(Color("BrandMuted"))
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }.contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityLabel("Play \(track.title)")
            Button("Add \(track.title) to queue", systemImage: "plus.circle", action: enqueue).labelStyle(.iconOnly).buttonStyle(MusicTransportStyle())
            Menu {
                Button("Play now", systemImage: "play.fill", action: play)
                Button("Add to queue", systemImage: "text.badge.plus", action: enqueue)
            } label: { Image(systemName: "ellipsis").frame(width: 44, height: 44) }.accessibilityLabel("More for \(track.title)")
        }.padding(.vertical, 4).listRowBackground(Color.clear).listRowSeparator(.hidden)
    }
    private func play() {
        guard let profile else { return }
        if remote.target != nil { remote.play(track) }
        else { playback.enqueue(track, profile: profile, token: bearerToken); if let entry = playback.queue.last { playback.playQueued(entry) } }
    }
    private func enqueue() {
        guard let profile else { return }
        if remote.target != nil { remote.enqueue(track) } else { playback.enqueue(track, profile: profile, token: bearerToken) }
    }
}

/// The same cover and picker serve albums, books, and artists.
struct ArtworkRow: View {
    @EnvironmentObject private var model: AppModel
    let kind: ArtworkKind
    let id: String
    let title: String
    let subtitle: String
    var tile = false
    let open: () -> Void
    @State private var image: UIImage?
    @State private var libraryPickerOpen = false
    @State private var pickerOpen = false
    @State private var selection: PhotosPickerItem?
    @State private var busy = false
    @State private var revision = 0
    @State private var pickerProfileID: UUID?

    var body: some View {
        Button(action: open) {
            let layout = tile ? AnyLayout(VStackLayout(alignment: .leading, spacing: 9)) : AnyLayout(HStackLayout(spacing: 14))
            layout {
                ZStack {
                    RoundedRectangle(cornerRadius: 8).fill(Color.accentColor.opacity(0.12))
                    if let image {
                        Image(uiImage: image).resizable().scaledToFill()
                    } else {
                        VStack(spacing: 5) {
                            Image(systemName: "music.note")
                            Text(title).font(.caption).multilineTextAlignment(.center).lineLimit(4)
                        }.padding(6)
                    }
                }.frame(width: tile ? min(220, (UIScreen.main.bounds.width - 72) / 2) : 72, height: tile ? min(220, (UIScreen.main.bounds.width - 72) / 2) : 72).clipped().clipShape(RoundedRectangle(cornerRadius: 9)).accessibilityLabel("Cover art for \(title)")
                VStack(alignment: .leading) {
                    Text(title).font(tile ? .subheadline.weight(.semibold) : .body).lineLimit(2).foregroundStyle(Color("BrandText"))
                    if !subtitle.isEmpty { Text(subtitle).font(.caption).lineLimit(1).foregroundStyle(Color("BrandMuted")) }
                }
                if busy { ProgressView() }
            }
        }
        .buttonStyle(.plain)
        .listRowBackground(Color.clear)
        .contextMenu {
            Button("Choose library image", systemImage: "photo.on.rectangle") { libraryPickerOpen = true }.disabled(busy)
            Button("Upload cover image", systemImage: "photo") { Task { await beginPicker() } }.disabled(busy)
            Button("Use automatic cover", systemImage: "arrow.counterclockwise") { Task { await save(nil) } }.disabled(busy)
        }
        .sheet(isPresented: $libraryPickerOpen) {
            LibraryArtworkPicker(kind: kind, targetID: id, title: title) { revision += 1 }
                .environmentObject(model).appControls()
        }
        .photosPicker(isPresented: $pickerOpen, selection: $selection, matching: .images)
        .onChange(of: selection) { item in
            guard let item else { return }
            Task { await upload(item) }
        }
        .task(id: "\(model.profile?.id.uuidString ?? ""):\(id):\(revision)") {
            image = nil
            guard let profile = model.profile else { return }
            do {
                let api = LibraryAsyncClient(transport: try CSILHTTPTransport(profile: profile, token: model.token()))
                let art: CoverArt
                if kind == .album { art = try await api.getCoverArt(CoverArtRequest(albumId: id, maxSize: 256)) }
                else { art = try await api.getArtwork(ArtworkRequest(kind: kind, id: id, maxSize: 256)) }
                guard !Task.isCancelled, model.profile?.id == profile.id else { return }
                image = UIImage(data: Data(art.data))
            } catch { /* A named placeholder remains visible when no cover is available. */ }
        }
    }

    private func beginPicker() async {
        guard let profile = model.profile else { return }
        do {
            let api = SessionAsyncClient(transport: try CSILHTTPTransport(profile: profile, token: model.token()))
            let session = try await api.whoami(Page())
            guard model.profile?.id == profile.id else { return }
            guard session.canEditLibrary == true || session.role == .admin else {
                model.message = "An administrator must grant you Edit library artwork permission."
                return
            }
            pickerProfileID = profile.id
            selection = nil
            pickerOpen = true
        } catch { model.message = "Sign in with permission to edit library artwork." }
    }

    private func upload(_ item: PhotosPickerItem) async {
        guard model.profile?.id == pickerProfileID else { return }
        busy = true
        do {
            guard let data = try await item.loadTransferable(type: Data.self), data.count <= 8 * 1024 * 1024,
                  let source = CGImageSourceCreateWithData(data as CFData, nil),
                  let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
                  let width = properties[kCGImagePropertyPixelWidth] as? Int,
                  let height = properties[kCGImagePropertyPixelHeight] as? Int,
                  (1...8192).contains(width), (1...8192).contains(height),
                  let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceCreateThumbnailWithTransform: true,
                    kCGImageSourceThumbnailMaxPixelSize: 1600,
                  ] as CFDictionary),
                  let jpeg = UIImage(cgImage: thumbnail).jpegData(compressionQuality: 0.88)
            else { throw ArtworkError.invalidImage }
            guard model.profile?.id == pickerProfileID else { busy = false; return }
            await save(Array(jpeg))
        } catch { model.message = "Choose an image up to 8 MB and 8192 pixels per side." }
        busy = false
    }

    private func save(_ data: [UInt8]?) async {
        guard let profile = model.profile else { return }
        busy = true
        defer { busy = false }
        do {
            let api = LibraryAsyncClient(transport: try CSILHTTPTransport(profile: profile, token: model.token()))
            _ = try await api.setArtwork(SetArtworkRequest(kind: kind, id: id, data: data))
            guard model.profile?.id == profile.id else { return }
            revision += 1
        } catch {
            guard model.profile?.id == profile.id else { return }
            model.message = "Cover art could not be saved. Check your Edit library artwork permission and server connection."
        }
    }
    private enum ArtworkError: Error { case invalidImage }
}


struct LibraryArtworkPicker: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    let kind: ArtworkKind
    let targetID: String
    let title: String
    let didSave: () -> Void
    @State private var candidates: [ArtworkCandidate] = []
    @State private var total: UInt64 = 0
    @State private var offset: UInt64 = 0
    @State private var selected: String?
    @State private var busy = false
    @State private var loading = true
    @State private var error: String?
    @State private var profileID: UUID?

    var body: some View {
        NavigationStack {
            List {
                Text("Choose a shared cover for \(title). Library scans will keep this choice.")
                if loading { ProgressView("Loading images…") }
                if let error { Text(error).foregroundStyle(.red); Button("Retry") { Task { await load() } } }
                if !loading && error == nil && candidates.isEmpty { Text("No local images found. You can upload a cover instead.") }
                ForEach(candidates, id: \.id) { candidate in
                    Button { selected = candidate.id } label: {
                        HStack {
                            LibraryArtworkPreview(kind: kind, targetID: targetID, candidateID: candidate.id)
                            VStack(alignment: .leading) {
                                Text(candidate.label).lineLimit(3)
                                Text("\(candidate.width) × \(candidate.height) · \(candidate.role)").font(.caption)
                                if candidate.suggested { Text("Automatic choice").font(.caption) }
                            }
                            if selected == candidate.id { Image(systemName: "checkmark.circle.fill") }
                        }
                    }.disabled(busy)
                }
                HStack {
                    if offset > 0 { Button("Previous") { offset -= 12 }.disabled(busy) }
                    Spacer()
                    if offset + UInt64(candidates.count) < total { Button("More images") { offset += 12 }.disabled(busy) }
                }
            }
            .navigationTitle("Library images")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(busy) }
                ToolbarItem(placement: .confirmationAction) { Button("Save cover") { Task { await save() } }.disabled(selected == nil || busy) }
            }
            .task(id: "\(model.profile?.id.uuidString ?? ""):\(offset)") { await load() }
        }
    }
    private func load() async {
        guard let profile = model.profile else { return }
        if profileID != nil && profileID != profile.id { dismiss(); return }
        profileID = profile.id; loading = true; error = nil
        defer { loading = false }
        do {
            let api = LibraryAsyncClient(transport: try CSILHTTPTransport(profile: profile, token: model.token()))
            let page = try await api.listArtworkCandidates(ArtworkCandidatesRequest(kind: kind, id: targetID, offset: offset, limit: 12))
            guard !Task.isCancelled, model.profile?.id == profile.id else { return }
            candidates = page.candidates; total = page.total
        } catch { if !Task.isCancelled { self.error = "Could not load library images. Check the server connection." } }
    }
    private func save() async {
        guard let selected, let profile = model.profile, profile.id == profileID else { return }
        busy = true; defer { busy = false }
        do {
            let api = LibraryAsyncClient(transport: try CSILHTTPTransport(profile: profile, token: model.token()))
            _ = try await api.setArtwork(SetArtworkRequest(kind: kind, id: targetID, candidateId: selected))
            guard model.profile?.id == profile.id else { return }
            didSave(); dismiss()
        } catch { model.message = "Could not save this cover. Check your artwork permission or rescan if the image changed." }
    }
}

private struct LibraryArtworkPreview: View {
    @EnvironmentObject private var model: AppModel
    let kind: ArtworkKind
    let targetID: String
    let candidateID: String
    @State private var image: UIImage?
    var body: some View {
        Group {
            if let image { Image(uiImage: image).resizable().scaledToFit() }
            else { Image(systemName: "photo").accessibilityLabel("Preview unavailable") }
        }.frame(width: 80, height: 80)
        .task(id: "\(model.profile?.id.uuidString ?? ""):\(candidateID)") {
            guard let profile = model.profile else { return }
            do {
                let api = LibraryAsyncClient(transport: try CSILHTTPTransport(profile: profile, token: model.token()))
                let art = try await api.getArtworkCandidate(ArtworkCandidateRequest(kind: kind, id: targetID, candidateId: candidateID, maxSize: 160))
                guard !Task.isCancelled, model.profile?.id == profile.id else { return }
                image = UIImage(data: Data(art.data))
            } catch { image = nil }
        }
    }
}
