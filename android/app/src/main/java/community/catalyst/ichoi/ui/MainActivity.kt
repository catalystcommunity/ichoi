package community.catalyst.ichoi.ui

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.ImageView
import community.catalyst.ichoi.R
import community.catalyst.csilgen.generated.*
import community.catalyst.ichoi.api.IchoiApi
import community.catalyst.ichoi.api.CsilServiceException
import community.catalyst.ichoi.api.LinkKeysLogin
import community.catalyst.ichoi.playback.PlaybackService
import community.catalyst.ichoi.playback.RemotePlayback
import community.catalyst.ichoi.playback.PlaybackQueue
import android.text.TextWatcher
import android.text.Editable
import community.catalyst.ichoi.security.ConnectionSecurity
import community.catalyst.ichoi.security.EndpointDecision
import community.catalyst.ichoi.security.SessionTokenStore
import community.catalyst.ichoi.security.ProfileSessions
import community.catalyst.ichoi.storage.ServerProfile
import community.catalyst.ichoi.storage.ServerProfileStore
import java.util.concurrent.Executors
import kotlin.coroutines.*

class MainActivity : Activity() {
    private lateinit var profiles: ServerProfileStore
    private lateinit var sessions: ProfileSessions
    private val worker = Executors.newSingleThreadExecutor()
    private var profile: ServerProfile? = null
    private var sessionInfo: SessionInfo? = null
    private lateinit var content: LinearLayout
    private lateinit var screenHost: FrameLayout
    private lateinit var messageBanner: MessageBanner
    private lateinit var playbackControls: PlaybackControls
    private lateinit var browseNavigation: LinearLayout
    private var queueScreen: LinearLayout? = null

    private data class CoverChoice(val api: IchoiApi, val kind: ArtworkKind, val id: String, val title: String,
        val profileId: String, val screen: LinearLayout, val row: ArtworkRow)
    private var coverChoice: CoverChoice? = null
    private val pendingCovers = ArrayDeque<CoverChoice>()
    private var activeCovers = 0
    private val history = ArrayDeque<Pair<LinearLayout, ScrollView>>()
    private fun backToBrowse() {
        if (history.isEmpty()) { profile?.let { connect(it) } ?: showWelcome(); return }
        val previous = history.removeLast()
        content = previous.first
        screenHost.removeAllViews()
        screenHost.addView(previous.second)
    }
    @Deprecated("Deprecated in Java")
    override fun onBackPressed() { if (history.isNotEmpty()) backToBrowse() else super.onBackPressed() }


    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        profiles = ServerProfileStore(this)
        sessions = ProfileSessions(SessionTokenStore(this))
        screenHost = FrameLayout(this)
        messageBanner = MessageBanner(this)
        playbackControls = PlaybackControls(this).apply { openQueue = { showQueue() }; chooseTarget = { showDevicePicker() } }
        browseNavigation = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            addView(button("Library") { profile?.let { connect(it) } }, LinearLayout.LayoutParams(0, -2, 1f))
            addView(button("Search") { profile?.let { showSearch(IchoiApi(it, sessions.token(it.id))) } }, LinearLayout.LayoutParams(0, -2, 1f))
        }
        setContentView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            addView(messageBanner)
            addView(screenHost, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(browseNavigation)
            addView(playbackControls)
        })
        showWelcome()
        handleLoginCallback(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleLoginCallback(intent)
    }

    private fun showWelcome() {
        beginScreen(reset = true)
        browseNavigation.visibility = android.view.View.GONE
        content.addView(ImageView(this).apply { setImageResource(R.mipmap.ic_launcher); contentDescription = "Ichoi" }, LinearLayout.LayoutParams(dp(80), dp(80)))
        text("Ichoi", 30f)
        text("Ichoi is a native client for an Ichoi server. Your selected server operator controls the library and account data.")
        val terms = CheckBox(this).apply {
            text = "I accept the Terms of Use"
            isChecked = getPreferences(0).getBoolean(TERMS, false)
        }
        content.addView(terms)
        link("Read the Terms of Use", TERMS_URL)
        link("Read the Privacy Policy", PRIVACY_URL)
        val add = button("Add my server") { addServerDialog() }
        val demo = button("Connect to demo server") { connect(profiles.put(DEMO_URL)) }
        add.isEnabled = FirstRunPolicy.canConnect(terms.isChecked)
        demo.isEnabled = add.isEnabled
        content.addView(add)
        content.addView(demo)
        content.addView(button("App settings") {
            beginScreen()
            text("App settings", 24f)
            messageDurationSetting()
            content.addView(button("Back") { showWelcome() })
        })
        terms.setOnCheckedChangeListener { _, checked ->
            getPreferences(0).edit().putBoolean(TERMS, checked).apply()
            add.isEnabled = FirstRunPolicy.canConnect(checked)
            demo.isEnabled = add.isEnabled
        }
        profiles.all().forEach { saved ->
            val reconnect = button("Connect to ${saved.url}") { connect(saved) }
            reconnect.isEnabled = FirstRunPolicy.canConnect(terms.isChecked)
            content.addView(reconnect)
        }
    }

    private fun addServerDialog() {
        val fields = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(32, 8, 32, 0) }
        val url = EditText(this).apply { hint = "Server address (server.example)"; inputType = 0x10 or 0x80000 }
        fields.addView(url)
        AlertDialog.Builder(this).setTitle("Add my server").setView(fields).setNegativeButton("Cancel", null)
            .setPositiveButton("Connect") { _, _ -> validateAndConnect(url.text.toString()) }.show()
    }

    private fun validateAndConnect(raw: String) {
        when (val decision = ConnectionSecurity.check(raw)) {
            EndpointDecision.Secure -> connect(profiles.put(raw.trim(), false))
            is EndpointDecision.PrivateHttpNeedsConsent -> AlertDialog.Builder(this)
                .setTitle("Use an unencrypted private connection?")
                .setMessage("Only use plain HTTP on a private network. Other people on that network can read this connection.")
                .setNegativeButton("Cancel", null)
                .setPositiveButton("I understand") { _, _ -> connect(profiles.put(raw.trim(), true)) }.show()
            is EndpointDecision.Rejected -> showError(decision.reason)
        }
    }

    private fun connect(selected: ServerProfile) {
        if (profile?.id != selected.id) sessionInfo = null
        profile = selected
        beginScreen(reset = true)
        val loadingScreen = content
        text("Your Ichoi library", 24f)
        text("Server: ${selected.url}")
        text("Loading library…")
        content.addView(button("Settings") { showSettings(IchoiApi(selected, sessions.token(selected.id))) })
        val token = sessions.token(selected.id)
        val api = IchoiApi(selected, token)
        RemotePlayback.configure(selected, token)
        execute({
            val libraries = api.libraries()
            val restored = if (token == null) null else runCatching { api.whoAmI() }.getOrNull()
            libraries to restored
        }, { result, error ->
            if (content !== loadingScreen || profile?.id != selected.id) return@execute
            if (error != null) showConnectionFailure(selected, api, error)
            else {
                sessionInfo = result!!.second
                showLibrary(api, result.first)
            }
        })
    }

    private fun showConnectionFailure(selected: ServerProfile, api: IchoiApi, error: Throwable) {
        beginScreen(reset = true)
        val needsSignIn = error is CsilServiceException && error.code == 401L
        text(if (needsSignIn) "Sign in to your library" else "Could not load your library", 28f)
        text(selected.url.removePrefix("https://"))
        if (needsSignIn) {
            sessionInfo = null
            text("This server requires sign-in. Use your LinkKeys identity to continue.")
            val identity = EditText(this).apply {
                hint = "handle@domain"
                inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
                isSingleLine = true
            }
            content.addView(identity)
            content.addView(button("Sign in with LinkKeys") { startLogin(identity.text.toString()) })
        } else {
            text(error.message ?: "Check your connection and try again.")
            content.addView(button("Retry") { connect(selected) })
        }
        content.addView(button("Settings") { showSettings(api) })
        content.addView(button("Choose another server") { showWelcome() })
    }

    private fun showLibrary(api: IchoiApi, libraries: LibrariesResponse) {
        beginScreen(reset = true)
        text("Your Ichoi library", 24f)
        val screen = content
        val search = button("Search artists, albums, and tracks") { showSearch(api) }
        content.addView(search)
        val categories = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        libraries.libraries.forEach { info -> categories.addView(button(if (info.kind == Library.Music) "Music" else "Audiobooks") {
            execute({ api.albums(info.kind) }, { result, error ->
                if (content !== screen) return@execute
                if (error != null) showError(error.message ?: "Could not load albums") else showAlbums(api, result!!, info.kind)
            })
        }, LinearLayout.LayoutParams(0, -2, 1f)) }
        content.addView(categories)
        val sections = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        sections.addView(button("Artists") {
            val kinds = libraries.libraries.map { it.kind }
            fun open(kind: Library) { execute({ api.artists(kind) }, { result, error ->
                if (content !== screen) return@execute
                if (error != null) showError(error.message ?: "Could not load artists") else showArtists(api, result!!, kind)
            }) }
            if (kinds.size > 1) AlertDialog.Builder(this).setTitle("Browse artists").setItems(kinds.map { it.name }.toTypedArray()) { _, index -> open(kinds[index]) }.show()
            else kinds.firstOrNull()?.let { open(it) }
        }, LinearLayout.LayoutParams(0, -2, 1f))
        sections.addView(button("Playlists") {
            execute({ api.playlists() }, { result, error ->
                if (content !== screen) return@execute
                if (error != null) showError(error.message ?: "Could not load playlists") else showPlaylists(api, result!!)
            })
        }, LinearLayout.LayoutParams(0, -2, 1f))
        sections.addView(button("Settings") { showSettings(api) }, LinearLayout.LayoutParams(0, -2, 1f))
        content.addView(sections)
        val kind = libraries.libraries.firstOrNull()?.kind
        if (kind != null) {
            text("In your library", 22f)
            val covers = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
            content.addView(covers)
            val loading = TextView(this).apply { text = "Loading covers…" }; covers.addView(loading)
            fun load() {
                execute({ api.albums(kind) }, { result, error ->
                    if (content !== screen) return@execute
                    covers.removeAllViews()
                    if (error != null) {
                        covers.addView(TextView(this).apply { text = "Could not load albums." })
                        covers.addView(button("Retry") { load() })
                    } else {
                        result!!.albums.take(12).chunked(2).forEach { pair ->
                            val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }; covers.addView(row)
                            pair.forEach { album -> addArtworkRow(api, ArtworkKind.Album, album.id, album.title, album.artistName ?: "", row, true) {
                                execute({ api.album(album.id) }, { detail, failure -> if (content === screen) { if (failure != null) showError(failure.message ?: "Could not load album") else showTracks(detail!!.tracks) } })
                            } }
                            if (pair.size == 1) row.addView(android.view.View(this), LinearLayout.LayoutParams(0, 1, 1f))
                        }
                        covers.addView(button("Browse all albums") { showAlbums(api, result, kind) })
                    }
                })
            }
            load()
        }
    }

    private fun showAlbums(api: IchoiApi, albums: AlbumsResponse, libraryKind: Library = Library.Music) {
        beginScreen()
        text(if (libraryKind == Library.Audiobook) "Audiobooks" else "Albums", 24f)
        val screen = content
        content.addView(button("Search this library") { showSearch(api) })
        albums.albums.chunked(2).forEach { pair ->
            val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = android.view.Gravity.TOP }
            content.addView(row)
            pair.forEach { album ->
                addArtworkRow(api, ArtworkKind.Album, album.id, album.title, album.artistName ?: "", row, true) {
                    execute({ api.album(album.id) }, { detail, error ->
                        if (content !== screen) return@execute
                        if (error != null) showError(error.message ?: "Could not load album") else showTracks(detail!!.tracks)
                    })
                }
            }
            if (pair.size == 1) row.addView(android.view.View(this), LinearLayout.LayoutParams(0, 1, 1f))
        }
        if (albums.albums.size.toULong() < albums.total) content.addView(button("Load more albums") {
            execute({ api.albums(libraryKind, albums.albums.size.toULong()) }, { more, error ->
                if (content !== screen) return@execute
                if (error != null) showError(error.message ?: "Could not load albums")
                else showAlbums(api, AlbumsResponse(albums.albums + more!!.albums, more.total), libraryKind)
            })
        })
        content.addView(button("Back") { backToBrowse() })
    }

    private fun showArtists(api: IchoiApi, artists: ArtistsResponse, libraryKind: Library) {
        beginScreen()
        text("Artists", 24f)
        val screen = content
        artists.artists.forEach { artist ->
            addArtworkRow(api, ArtworkKind.Artist, artist.id, artist.name, "${artist.albumCount} albums") {
                execute({ api.artist(artist.id, libraryKind) }, { detail, error ->
                    if (content !== screen) return@execute
                    if (error != null) showError(error.message ?: "Could not load artist")
                    else showAlbums(api, AlbumsResponse(detail!!.albums, detail.albums.size.toULong()), libraryKind)
                })
            }
        }
        if (artists.artists.size.toULong() < artists.total) content.addView(button("Load more artists") {
            execute({ api.artists(libraryKind, artists.artists.size.toULong()) }, { more, error ->
                if (content !== screen) return@execute
                if (error != null) showError(error.message ?: "Could not load artists")
                else showArtists(api, ArtistsResponse(artists.artists + more!!.artists, more.total), libraryKind)
            })
        })
        content.addView(button("Back") { backToBrowse() })
    }

    private fun addArtworkRow(api: IchoiApi, kind: ArtworkKind, id: String, title: String, subtitle: String, container: LinearLayout = content, tile: Boolean = false, open: () -> Unit) {
        val selected = profile ?: return
        val screen = content
        val row = ArtworkRow(this, title, subtitle, tile)
        val choice = CoverChoice(api, kind, id, title, selected.id, screen, row)
        row.setOnClickListener { open() }
        row.setOnLongClickListener {
            // Refresh permission before offering a shared-library change.
            execute({ api.whoAmI() }, { session, error ->
                if (profile?.id != selected.id || content !== screen) return@execute
                if (error != null || (session?.canEditLibrary != true && session?.role != Role.Admin)) {
                    showError("An administrator must grant you Edit library artwork permission.")
                } else {
                    sessionInfo = session
                    AlertDialog.Builder(this).setTitle("Cover art: $title")
                        .setItems(arrayOf("Choose library image", "Upload image", "Use automatic cover")) { _, which ->
                            if (which == 0) showLibraryImages(choice)
                            else if (which == 1) {
                                coverChoice = choice
                                startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                                    type = "image/*"; addCategory(Intent.CATEGORY_OPENABLE)
                                }, PICK_COVER)
                            } else saveCover(choice, null)
                        }.setNegativeButton("Cancel", null).show()
                }
            })
            true
        }
        container.addView(row, if (tile) LinearLayout.LayoutParams(0, -2, 1f).apply { marginEnd = dp(8) } else LinearLayout.LayoutParams(-1, -2))
        row.whenVisible { loadCover(choice) }
    }

    private fun showLibraryImages(choice: CoverChoice, offset: ULong = 0uL) {
        execute({ choice.api.artworkCandidates(choice.kind, choice.id, offset) }, { page, error ->
            if (profile?.id != choice.profileId || content !== choice.screen) return@execute
            if (error != null) { showError(error.message ?: "Could not load library images"); return@execute }
            val rows = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(20, 12, 20, 12) }
            val dialog = AlertDialog.Builder(this).setTitle("Images in this library")
                .setView(ScrollView(this).apply { addView(rows) }).setNegativeButton("Cancel", null).create()
            if (page!!.candidates.isEmpty()) rows.addView(TextView(this).apply { text = "No local images found. You can upload a cover instead." })
            page.candidates.forEach { candidate ->
                val subtitle = "${candidate.width} × ${candidate.height} · ${candidate.role}" + if (candidate.suggested) " · Automatic choice" else ""
                val row = ArtworkRow(this, candidate.label, subtitle)
                rows.addView(row)
                row.setOnClickListener {
                    if (profile?.id != choice.profileId || content !== choice.screen) { dialog.dismiss(); return@setOnClickListener }
                    // A separate Save action prevents an accidental tap from changing the shared cover.
                    AlertDialog.Builder(this).setTitle("Use this cover?").setMessage(candidate.label)
                        .setPositiveButton("Save cover") { _, _ ->
                            if (profile?.id == choice.profileId && choice.screen.isAttachedToWindow) saveCover(choice, null, candidate.id)
                            dialog.dismiss()
                        }.setNegativeButton("Cancel", null).show()
                }
                execute({ ArtworkImages.decode(choice.api.artworkCandidate(choice.kind, choice.id, candidate.id).data) }, { bitmap, _ ->
                    if (dialog.isShowing && profile?.id == choice.profileId) row.showCover(bitmap) else bitmap?.recycle()
                })
            }
            if (offset > 0uL) rows.addView(button("Previous images") { dialog.dismiss(); showLibraryImages(choice, offset - 12uL) })
            if (offset + page.candidates.size.toULong() < page.total) rows.addView(button("More images") { dialog.dismiss(); showLibraryImages(choice, offset + 12uL) })
            dialog.show()
        })
    }

    private fun loadCover(choice: CoverChoice) {
        pendingCovers.addLast(choice)
        drainCovers()
    }

    private fun drainCovers() {
        while (activeCovers < 2 && pendingCovers.isNotEmpty()) {
            val choice = pendingCovers.removeFirst()
            if (isDestroyed || profile?.id != choice.profileId) continue
            if (!choice.screen.isAttachedToWindow) {
                choice.row.whenVisible { loadCover(choice) }
                continue
            }
            activeCovers++
            execute({ ArtworkImages.decode(choice.api.artwork(choice.kind, choice.id).data) }, { bitmap, _ ->
                activeCovers--
                if (!isDestroyed && profile?.id == choice.profileId) choice.row.showCover(bitmap)
                else bitmap?.recycle()
                drainCovers()
            })
        }
    }

    private fun saveCover(choice: CoverChoice, data: ByteArray?, candidateId: String? = null) {
        execute({ choice.api.setArtwork(choice.kind, choice.id, data, candidateId) }, { _, error ->
            if (profile?.id != choice.profileId || content !== choice.screen) return@execute
            if (error != null) showError(error.message ?: "Cover art could not be saved") else loadCover(choice)
        })
    }

    @Deprecated("Uses the platform document picker without storage permissions")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != PICK_COVER) return
        val choice = coverChoice ?: return
        coverChoice = null
        val uri = data?.data
        if (resultCode != RESULT_OK || uri == null || profile?.id != choice.profileId || content !== choice.screen) return
        execute({
            val bytes = contentResolver.openInputStream(uri)?.use { input ->
                val output = java.io.ByteArrayOutputStream()
                val buffer = ByteArray(8192)
                while (true) {
                    val count = input.read(buffer)
                    if (count < 0) break
                    require(output.size() + count <= ArtworkImages.MAX_BYTES) { "Choose an image no larger than 8 MB." }
                    output.write(buffer, 0, count)
                }
                output.toByteArray()
            } ?: error("This image could not be opened.")
            ArtworkImages.upload(bytes)
        }, { bytes, error ->
            if (profile?.id != choice.profileId || content !== choice.screen) return@execute
            if (error != null) showError(error.message ?: "This image could not be read") else saveCover(choice, bytes!!)
        })
    }

    private fun showPlaylists(api: IchoiApi, playlists: PlaylistsResponse) {
        beginScreen()
        text("Playlists", 24f)
        playlists.playlists.forEach { playlist ->
            content.addView(button(playlist.name) {
                execute({ api.playlist(playlist.id) }, { detail, error ->
                    if (error != null) showError(error.message ?: "Could not load playlist") else showTracks(detail!!.tracks)
                })
            })
            if (sessionInfo != null) {
                content.addView(button("Report playlist: ${playlist.name}") {
                    report(api, ContentReportTargetType.Playlist, playlist.id)
                })
                playlist.owner?.let { owner -> content.addView(button("Report playlist owner") {
                    report(api, ContentReportTargetType.Account, owner)
                }) }
                playlist.owner?.takeIf { it != sessionInfo?.accountId }?.let { owner ->
                    content.addView(button("Block user") {
                        val selected = profile ?: return@button
                        execute({
                            AccountBlockAction.change(
                                selected.id,
                                owner,
                                { _, accountId -> api.blockAccount(accountId); Unit },
                                { _ -> api.playlists() },
                            )
                        }, { result, error ->
                            if (profile?.id != selected.id) return@execute
                            if (error != null) showError(error.message ?: "The account was not blocked")
                            else showPlaylists(api, result!!)
                        })
                    })
                }
            }
        }
        content.addView(button("Back") { backToBrowse() })
    }

    private fun addTrack(track: Track) {
        text(track.title, 18f)
        val actions = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        actions.addView(button("Play") {
            val selected = profile ?: return@button
            if (RemotePlayback.target != null) RemotePlayback.play(track)
            else { PlaybackQueue.enqueue(track, selected, sessions.token(selected.id)); PlaybackQueue.play(this, PlaybackQueue.entries.lastIndex) }
        }.apply { contentDescription = "Play ${track.title}" }, LinearLayout.LayoutParams(0, -2, 1f))
        actions.addView(button("Add to queue") {
            val selected = profile ?: return@button
            if (RemotePlayback.target != null) RemotePlayback.enqueue(track)
            else PlaybackQueue.enqueue(track, selected, sessions.token(selected.id))
        }.apply { contentDescription = "Add ${track.title} to queue" }, LinearLayout.LayoutParams(0, -2, 1f))
        content.addView(actions)
    }
    private fun showTracks(tracks: List<Track>) {
        beginScreen()
        text("Tracks", 24f)
        content.addView(button("Back") { backToBrowse() })
        content.addView(button("Now playing queue") { showQueue() })
        tracks.forEach { addTrack(it) }
    }
    private fun showQueue(refresh: Boolean = false) {
        if (!refresh && content === queueScreen) return
        if (!refresh) beginScreen() else content.removeAllViews()
        queueScreen = content
        text("Now playing", 24f)
        content.addView(button("Back to browsing") { backToBrowse() })
        if (RemotePlayback.target != null) {
            text("Playing on ${RemotePlayback.target!!.name}", 14f)
            content.addView(button("Undo queue change") { RemotePlayback.undo() }.apply { isEnabled = RemotePlayback.state?.canUndo == true })
            RemotePlayback.error?.let { text(it); content.addView(button("Retry") { RemotePlayback.refresh() }) }
            val entries = RemotePlayback.state?.queue.orEmpty()
            if (entries.isEmpty()) text(if (RemotePlayback.state == null) "Loading device queue…" else "This device’s queue is empty.")
            entries.forEachIndexed { index, item ->
                queueRow(item.title ?: item.trackId, index, entries.size,
                    { RemotePlayback.playItem(item) }, { RemotePlayback.move(index, index - 1) },
                    { RemotePlayback.move(index, index + 1) }, { RemotePlayback.remove(item) })
            }
        } else {
            content.addView(button("Undo queue change") { PlaybackQueue.undo(); showQueue(true) })
            if (PlaybackQueue.entries.isEmpty()) text("Your queue is empty. Add music or an audiobook from the library.")
            PlaybackQueue.entries.forEachIndexed { index, entry ->
                queueRow((if (entry.id == PlaybackQueue.currentId) "Playing · " else "") + entry.track.title, index, PlaybackQueue.entries.size,
                    { PlaybackQueue.play(this, index); showQueue(true) }, { PlaybackQueue.move(index, index - 1); showQueue(true) },
                    { PlaybackQueue.move(index, index + 1); showQueue(true) }, { PlaybackQueue.remove(index); showQueue(true) })
            }
        }
    }
    private fun queueRow(title: String, index: Int, count: Int, play: () -> Unit, up: () -> Unit, down: () -> Unit, remove: () -> Unit) {
        text(title, 17f)
        content.addView(LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            listOf(button("Play", play), button("↑", up).apply { contentDescription = "Move $title up"; isEnabled = index > 0 },
                button("↓", down).apply { contentDescription = "Move $title down"; isEnabled = index + 1 < count },
                button("Remove", remove).apply { contentDescription = "Remove $title" }).forEach { addView(it, LinearLayout.LayoutParams(0, -2, 1f)) }
        })
    }
    private var lastRemoteQueue = ""
    private val remoteChanged: () -> Unit = {
        val signature = "${RemotePlayback.target?.id}|${RemotePlayback.state?.revision}|${RemotePlayback.error}"
        if (signature != lastRemoteQueue) {
            lastRemoteQueue = signature
            if (::content.isInitialized && content === queueScreen) showQueue(true)
        }
    }
    private fun showDevicePicker() {
        val selected = profile ?: return
        RemotePlayback.configure(selected, sessions.token(selected.id))
        val dialog = AlertDialog.Builder(this).setTitle("Play on a device").setMessage("Loading devices…").setNegativeButton("Cancel", null).show()
        RemotePlayback.list { players, error ->
            if (!dialog.isShowing || profile?.id != selected.id) return@list
            dialog.dismiss()
            if (error != null) {
                AlertDialog.Builder(this).setTitle("Could not load devices").setMessage(error.message)
                    .setPositiveButton("Retry") { _, _ -> showDevicePicker() }.setNegativeButton("Cancel", null).show()
            } else {
                val choices = listOf("This phone") + players.orEmpty().map { it.name + if (it.audioBlocked == true) " · Enable audio on device" else "" }
                AlertDialog.Builder(this).setTitle("Play on a device").setSingleChoiceItems(choices.toTypedArray(), players.orEmpty().indexOfFirst { it.id == RemotePlayback.target?.id } + 1) { picker, index ->
                    RemotePlayback.select(this, if (index == 0) null else players!![index - 1]); picker.dismiss()
                }.setNegativeButton("Cancel", null).show()
            }
        }
    }

    private fun showSearch(api: IchoiApi) {
        beginScreen()
        val screen = content
        text("Search", 24f)
        content.addView(button("Back") { backToBrowse() })
        content.addView(button("Now playing queue") { showQueue() })
        val input = EditText(this).apply { hint = "Albums, artists, tracks, and books"; inputType = android.text.InputType.TYPE_CLASS_TEXT }
        content.addView(input)
        val results = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        content.addView(results)
        var revision = 0
        val handler = android.os.Handler(mainLooper)
        input.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                val query = s.toString().trim(); val version = ++revision
                results.removeAllViews()
                if (query.isEmpty()) return
                handler.postDelayed({
                    if (version != revision) return@postDelayed
                    execute({ listOf(api.search(query, Library.Music), api.search(query, Library.Audiobook)) }, { pages, error ->
                        if (version != revision || (content !== screen && history.none { it.first === screen })) return@execute
                        if (error != null) { showError("Search failed. Try again."); return@execute }
                        results.removeAllViews()
                        val original = content; content = results
                        pages!!.forEachIndexed { pageIndex, page ->
                            val kind = if (pageIndex == 0) Library.Music else Library.Audiobook
                            page.albums.forEach { album -> addArtworkRow(api, ArtworkKind.Album, album.id, album.title, album.artistName ?: "") {
                                execute({ api.album(album.id) }, { detail, failure -> if (content === screen && failure == null) showTracks(detail!!.tracks) })
                            } }
                            page.artists.forEach { artist -> content.addView(button(artist.name) {
                                execute({ api.artist(artist.id, kind) }, { detail, failure -> if (content === screen && failure == null) showAlbums(api, AlbumsResponse(detail!!.albums, detail.albums.size.toULong()), kind) })
                            }) }
                            page.tracks.forEach { addTrack(it) }
                        }
                        content = original
                    })
                }, 250)
            }
            override fun afterTextChanged(s: Editable?) {}
        })
    }

    private fun showSettings(api: IchoiApi, blocked: BlockedAccountsResponse? = null) {
        beginScreen()
        text("Settings", 24f)
        text("Server: ${profile?.url}")
        content.addView(button("Back to library") { connect(profile!!) })
        messageDurationSetting()
        link("Privacy Policy", PRIVACY_URL)
        link("Terms of Use", TERMS_URL)
        val current = sessionInfo
        if (current != null) {
            val settingsProfileId = profile?.id
            val settingsScreen = content
            text("Signed in as ${current.handle}")
            content.addView(button("Sign out") {
                execute({ runCatching { api.signOut() }; Unit }, { _, _ ->
                    sessions.signOut(profile!!.id)
                    sessionInfo = null
                    showWelcome()
                })
            })
            content.addView(button("Delete account") { deleteAccount(api, current.handle) })
            execute({ blocked ?: api.blockedAccounts() }, { result, error ->
                if (isDestroyed || content !== settingsScreen || profile?.id != settingsProfileId || sessionInfo == null) return@execute
                if (error != null) {
                    showError("Could not load blocked accounts: ${error.message ?: "service failure"}")
                } else {
                    text("Blocked accounts", 20f)
                    text("A block hides public playlists from that account on this selected server.")
                    if (result!!.accounts.isEmpty()) text("You have not blocked an account on this server.")
                    result.accounts.forEach { account ->
                        content.addView(button("Unblock ${account.displayName ?: account.handle}") {
                            val selected = profile ?: return@button
                            execute({
                                AccountBlockAction.change(
                                    selected.id,
                                    account.accountId,
                                    { _, accountId -> api.unblockAccount(accountId); Unit },
                                    { _ -> api.blockedAccounts() },
                                )
                            }, { refreshed, unblockError ->
                                if (isDestroyed || content !== settingsScreen || profile?.id != selected.id) return@execute
                                if (unblockError != null) {
                                    showError(unblockError.message ?: "The account was not unblocked")
                                } else showSettings(api, refreshed)
                            })
                        })
                    }
                }
            })
        } else {
            val identity = EditText(this).apply { hint = "handle@domain" }
            content.addView(identity)
            content.addView(button("Sign in with LinkKeys") { startLogin(identity.text.toString()) })
        }
        content.addView(button("Remove selected server") {
            val selected = profile ?: return@button
            sessions.signOut(selected.id)
            profiles.remove(selected.id)
            profile = null
            sessionInfo = null
            showWelcome()
        })
    }

    private fun startLogin(identity: String) {
        val selected = profile ?: return
        getPreferences(0).edit().putString(PENDING_LOGIN_PROFILE, selected.id).apply()
        execute({ LinkKeysLogin.start(selected, identity) }, { redirect, error ->
            if (error != null) {
                getPreferences(0).edit().remove(PENDING_LOGIN_PROFILE).apply()
                showError(error.message ?: "LinkKeys login could not start")
            }
            else startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(redirect)))
        })
    }

    private fun handleLoginCallback(intent: Intent?) {
        val data = intent?.data ?: return
        if (data.scheme != "ichoi" || data.host != "linkkeys") return
        val exchange = LinkKeysCallback.exchangeCode(data.toString()) ?: run {
                showError("The LinkKeys callback did not include an exchange code")
                return
            }
        val pendingProfile = getPreferences(0).getString(PENDING_LOGIN_PROFILE, null)
        val selected = profiles.all().firstOrNull { it.id == pendingProfile } ?: run {
            showError("Add the server again before sign-in")
            return
        }
        profile = selected
        val api = IchoiApi(selected, null)
        execute({ api.authenticate(AuthRequest(linkkeysExchangeCode = exchange)) }, { info, error ->
            if (error != null || info?.token.isNullOrEmpty()) {
                getPreferences(0).edit().remove(PENDING_LOGIN_PROFILE).apply()
                showError("LinkKeys sign-in did not complete")
            }
            else {
                sessions.replace(selected.id, info!!.token!!)
                getPreferences(0).edit().remove(PENDING_LOGIN_PROFILE).apply()
                sessionInfo = info
                connect(selected)
            }
        })
    }

    private fun deleteAccount(api: IchoiApi, currentHandle: String) {
        val handle = EditText(this).apply { hint = "Enter your current handle exactly" }
        AlertDialog.Builder(this).setTitle("Delete account")
            .setMessage("This removes your account and every public and private playlist that you own from ${profile?.url}.")
            .setView(handle).setNegativeButton("Cancel", null).setPositiveButton("Delete permanently") { _, _ ->
                if (runCatching { AccountValidation.requireExactHandle(handle.text.toString(), currentHandle) }.isFailure) {
                    showError("Enter the current handle exactly. Your session is kept.")
                    return@setPositiveButton
                }
                execute({
                    sessions.deleteAfterConfirmation(
                        profile!!.id,
                        currentHandle,
                        handle.text.toString(),
                    ) { api.deleteAccount(it).ok }
                }, { ok, error ->
                    if (error != null || ok != true) {
                        showError("Deletion failed. Your session is kept. Contact the administrator of ${profile?.url}.")
                    } else {
                        sessionInfo = null
                        showWelcome()
                    }
                })
            }.show()
    }

    private fun report(api: IchoiApi, target: ContentReportTargetType, targetId: String) {
        ReportDialog.show(this, if (target == ContentReportTargetType.Playlist) "Report playlist" else "Report user") { reason, details ->
            execute({ api.report(target, targetId, reason, details) }, { _, error ->
                if (error != null) showError(error.message ?: "The report was not sent") else showError("Report sent")
            })
        }
    }

    private fun <T> execute(action: suspend () -> T, result: (T?, Throwable?) -> Unit) {
        worker.execute { action.startCoroutine(object : Continuation<T> {
            override val context: CoroutineContext = EmptyCoroutineContext
            override fun resumeWith(value: Result<T>) {
                value.fold(
                    { output -> runOnUiThread { if (!isDestroyed) result(output, null) } },
                    { error -> runOnUiThread { if (!isDestroyed) result(null, error) } },
                )
            }
        }) }
    }

    private fun dp(value: Int) = (value * resources.displayMetrics.density).toInt()
    private fun beginScreen(reset: Boolean = false) {
        browseNavigation.visibility = if (profile == null) android.view.View.GONE else android.view.View.VISIBLE
        if (reset) history.clear()
        else if (::content.isInitialized && screenHost.childCount > 0) {
            history.addLast(content to (screenHost.getChildAt(0) as ScrollView))
            while (history.size > 12) history.removeFirst()
        }
        content = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(dp(20), dp(24), dp(20), dp(24)) }
        screenHost.removeAllViews()
        screenHost.addView(ScrollView(this).apply { isFillViewport = true; addView(content) })
    }
    private fun text(value: String, size: Float = 16f) {
        content.addView(TextView(this).apply {
            text = value; textSize = size; setTextColor(getColor(R.color.ichoi_text)); setPadding(0, dp(8), 0, dp(8))
            if (size >= 24f) setTypeface(typeface, android.graphics.Typeface.BOLD)
        })
    }
    private fun button(label: String, action: () -> Unit) = Button(this).apply {
        text = label; isAllCaps = false; minHeight = dp(48); gravity = android.view.Gravity.CENTER_VERTICAL
        setPadding(dp(16), dp(10), dp(16), dp(10))
        backgroundTintList = android.content.res.ColorStateList.valueOf(getColor(R.color.ichoi_surface))
        layoutParams = LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(4); bottomMargin = dp(4) }
        setOnClickListener { action() }
    }
    private fun link(label: String, url: String) {
        content.addView(button(label) { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) })
    }
    private fun showError(message: String) {
        AppMessages.show(message)
    }

    private fun messageDurationSetting() {
        val preferences = MessagePreferences(this)
        val setting = Button(this)
        fun updateLabel() { setting.text = "Message duration: ${preferences.durationSeconds} seconds" }
        updateLabel()
        setting.setOnClickListener {
            val input = EditText(this).apply {
                inputType = android.text.InputType.TYPE_CLASS_NUMBER
                setText(preferences.durationSeconds.toString())
                selectAll()
            }
            val dialog = AlertDialog.Builder(this).setTitle("Message duration (1–60 seconds)")
                .setView(input).setNegativeButton("Cancel", null).setPositiveButton("Save", null).create()
            dialog.setOnShowListener {
                dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener {
                    val seconds = input.text.toString().toIntOrNull()
                    if (seconds == null || seconds !in 1..60) input.error = "Enter a number from 1 to 60."
                    else { preferences.durationSeconds = seconds; updateLabel(); dialog.dismiss() }
                }
            }
            dialog.show()
        }
        content.addView(setting)
    }

    override fun onStart() {
        super.onStart()
        messageBanner.start()
        playbackControls.start()
        RemotePlayback.observe(remoteChanged); RemotePlayback.start()
    }

    override fun onStop() {
        messageBanner.stop()
        playbackControls.stop()
        RemotePlayback.removeObserver(remoteChanged); RemotePlayback.stop()
        super.onStop()
    }

    override fun onDestroy() { messageBanner.stop(); playbackControls.stop(); worker.shutdownNow(); super.onDestroy() }

    companion object {
        private const val PICK_COVER = 101
        private const val TERMS = "termsAccepted"
        private const val PENDING_LOGIN_PROFILE = "pendingLoginProfile"
        private const val TERMS_URL = "https://catalystichoi.com/terms.html"
        // The public demo. Guests browse and play without a login.
        private const val DEMO_URL = "https://ichoi.catalystsquad.com"
        private const val PRIVACY_URL = "https://catalystichoi.com/privacy.html"
    }
}
