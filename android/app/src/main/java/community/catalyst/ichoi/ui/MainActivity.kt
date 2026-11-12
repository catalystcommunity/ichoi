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
    private lateinit var updates: PlayUpdates
    private lateinit var updateNotice: Button
    private var updateDetails: TextView? = null
    private lateinit var browseNavigation: LinearLayout
    private var queueScreen: LinearLayout? = null
    private lateinit var backControl: android.widget.ImageButton
    private lateinit var bottomNavigation: LinearLayout
    private val navItems = mutableMapOf<String, LinearLayout>()
    private var activeTab = "Library"
    private data class BrowseTab(val content: LinearLayout, val scroll: ScrollView, val history: List<Pair<LinearLayout, ScrollView>>)
    private val browseTabs = mutableMapOf<String, BrowseTab>()
    private var queueEditing = false
    private var queueGeneration = 0
    private data class QueueDrag(val index: Int, val generation: Int)

    private data class CoverChoice(val api: IchoiApi, val kind: ArtworkKind, val id: String, val title: String,
        val profileId: String, val screen: LinearLayout, val row: ArtworkRow)
    private var coverChoice: CoverChoice? = null
    private val history = ArrayDeque<Pair<LinearLayout, ScrollView>>()
    private fun backToBrowse() {
        if (history.isEmpty()) { profile?.let { connect(it) } ?: showWelcome(); return }
        val previous = history.removeLast()
        content = previous.first
        screenHost.removeAllViews()
        screenHost.addView(previous.second)
        updateNavigation()
    }
    @Deprecated("Deprecated in Java")
    override fun onBackPressed() { if (history.isNotEmpty()) backToBrowse() else super.onBackPressed() }


    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        profiles = ServerProfileStore(this)
        sessions = ProfileSessions(SessionTokenStore(this))
        screenHost = FrameLayout(this)
        messageBanner = MessageBanner(this)
        playbackControls = PlaybackControls(this).apply {
            openQueue = { openTab("Queue") }; chooseTarget = { showDevicePicker() }
            artwork = { id, result -> val selected = profile; if (selected != null && id != null) MusicArtwork.load(IchoiApi(selected,sessions.token(selected.id)), artworkScope(), ArtworkKind.Album,id,result) else result(null) }
        }
        backControl = iconButton(this, MusicIcon.BACK, "Back") { backToBrowse() }
        browseNavigation = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL; gravity = android.view.Gravity.CENTER_VERTICAL
            setPadding(dp(8),dp(4),dp(8),dp(4))
            addView(backControl,LinearLayout.LayoutParams(dp(48),dp(48)))
            addView(musicText(this@MainActivity,"ichoi",25f).apply { setTypeface(typeface,android.graphics.Typeface.BOLD);setPadding(dp(12),0,0,0) },LinearLayout.LayoutParams(0,-2,1f))
            addView(iconButton(this@MainActivity,MusicIcon.MENU,"Menu") { showNavigationMenu(getChildAt(childCount-1)) },LinearLayout.LayoutParams(dp(48),dp(48)))
        }
        bottomNavigation=LinearLayout(this).apply {
            gravity=android.view.Gravity.CENTER;setPadding(dp(8),dp(4),dp(8),dp(4))
            for((name,icon) in listOf("Library" to MusicIcon.LIBRARY,"Search" to MusicIcon.SEARCH,"Queue" to MusicIcon.QUEUE)) {
                val item=LinearLayout(this@MainActivity).apply {
                    orientation=LinearLayout.VERTICAL;gravity=android.view.Gravity.CENTER;isFocusable=true;contentDescription=name
                    background=touchSurface();minimumHeight=dp(58)
                    addView(ImageView(this@MainActivity).apply { setImageDrawable(MusicGlyph(icon,getColor(R.color.ichoi_text)));tag=icon },LinearLayout.LayoutParams(dp(23),dp(23)))
                    addView(musicText(this@MainActivity,name,11f).apply { gravity=android.view.Gravity.CENTER;setPadding(0,dp(4),0,0) })
                    setOnClickListener { openTab(name) }
                }
                navItems[name]=item;addView(item,LinearLayout.LayoutParams(0,dp(58),1f))
            }
        }
        updateNotice = appButton(this, "Update available", primary = true) { handleUpdateAction() }.apply { visibility = android.view.View.GONE }
        updates = PlayUpdates(this) { state ->
            updateDetails?.text = updateMessage(state)
            updateNotice.visibility = if (state == PlayUpdates.State.AVAILABLE || state == PlayUpdates.State.READY || state == PlayUpdates.State.DOWNLOADING) android.view.View.VISIBLE else android.view.View.GONE
            updateNotice.text = updateMessage(state)
            updateNotice.isEnabled = state != PlayUpdates.State.DOWNLOADING
        }
        setContentView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            addView(browseNavigation)
            addView(messageBanner)
            addView(updateNotice)
            addView(screenHost, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(playbackControls)
            addView(bottomNavigation)
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
        browseTabs.clear(); activeTab="Library"
        beginScreen(reset = true)
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
        browseTabs.clear(); activeTab="Library"
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
        text("Your Ichoi library", 28f)
        val screen=content
        val chips=LinearLayout(this).apply { gravity=android.view.Gravity.CENTER_VERTICAL;setPadding(0,dp(12),0,dp(20)) }
        fun openAlbums(kind:Library) { execute({ api.albums(kind) }, { result,error -> if(content===screen) { if(error!=null) showError(error.message?:"Could not load albums") else showAlbums(api,result!!,kind) } }) }
        chips.addView(musicChip(this,"Music",true) { openAlbums(Library.Music) })
        chips.addView(musicChip(this,"Audiobooks") { openAlbums(Library.Audiobook) })
        chips.addView(musicChip(this,"Playlists") { execute({ api.playlists() }, { result,error -> if(content===screen) { if(error!=null) showError(error.message?:"Could not load playlists") else showPlaylists(api,result!!) } }) })
        content.addView(android.widget.HorizontalScrollView(this).apply { isHorizontalScrollBarEnabled=false;addView(chips) })
        val kind = libraries.libraries.firstOrNull()?.kind
        if (kind != null) {
            val covers = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
            content.addView(covers)
            val loading = TextView(this).apply { text = "Loading covers…" }; covers.addView(loading)
            fun load() {
                execute({ api.albums(kind) }, { result, error ->
                    if (!retainsScreen(screen)) return@execute
                    val visibleScreen = content
                    content = screen
                    covers.removeAllViews()
                    if (error != null) {
                        covers.addView(TextView(this).apply { text = "Could not load albums." })
                        covers.addView(button("Retry") { load() })
                    } else {
                        result!!.albums.take(24).chunked(2).forEach { pair ->
                            val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }; covers.addView(row)
                            pair.forEach { album -> addArtworkRow(api, ArtworkKind.Album, album.id, album.title, album.artistName ?: "", row, true) {
                                execute({ api.album(album.id) }, { detail, failure -> if (content === screen) { if (failure != null) showError(failure.message ?: "Could not load album") else showTracks(detail!!.tracks, detail.album) } })
                            } }
                            if (pair.size == 1) row.addView(android.view.View(this), LinearLayout.LayoutParams(0, 1, 1f))
                        }
                        covers.addView(button("Browse all albums") { showAlbums(api, result, kind) })
                    }
                    content = visibleScreen
                })
            }
            load()
        }
    }

    private fun showAlbums(api: IchoiApi, albums: AlbumsResponse, libraryKind: Library = Library.Music) {
        beginScreen()
        text(if (libraryKind == Library.Audiobook) "Audiobooks" else "Albums", 24f)
        val screen = content
        albums.albums.chunked(2).forEach { pair ->
            val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = android.view.Gravity.TOP }
            content.addView(row)
            pair.forEach { album ->
                addArtworkRow(api, ArtworkKind.Album, album.id, album.title, album.artistName ?: "", row, true) {
                    execute({ api.album(album.id) }, { detail, error ->
                        if (content !== screen) return@execute
                        if (error != null) showError(error.message ?: "Could not load album") else showTracks(detail!!.tracks, detail.album)
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
            if (profile?.id != choice.profileId || !choice.row.isAttachedToWindow) return@execute
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

    private fun artworkScope() = "${profile?.id}:${sessions.token(profile?.id ?: "")}"
    private fun loadCover(choice: CoverChoice) {
        MusicArtwork.load(choice.api,artworkScope(),choice.kind,choice.id) { bitmap ->
            if(!isDestroyed && profile?.id==choice.profileId) choice.row.showCover(bitmap)
        }
    }
    private fun trackCover(id:String?, title:String): MusicCover = MusicCover(this,48).apply {
        contentDescription="Cover art for $title"
        val selected=profile
        if(id!=null && selected!=null) MusicArtwork.load(IchoiApi(selected,sessions.token(selected.id)),artworkScope(),ArtworkKind.Album,id) { bitmap -> if(!isDestroyed && profile?.id==selected.id) show(bitmap) }
    }
    private fun saveCover(choice: CoverChoice, data: ByteArray?, candidateId: String? = null) {
        execute({ choice.api.setArtwork(choice.kind, choice.id, data, candidateId) }, { _, error ->
            if (profile?.id != choice.profileId || !choice.row.isAttachedToWindow) return@execute
            if (error != null) showError(error.message ?: "Cover art could not be saved") else { MusicArtwork.clear(); loadCover(choice) }
        })
    }

    @Deprecated("Uses the platform document picker without storage permissions")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == PlayUpdates.REQUEST_CODE) { updates.result(resultCode); return }
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
            if (profile?.id != choice.profileId || !choice.row.isAttachedToWindow) return@execute
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
    }

    private fun playTrack(track: Track) {
        val selected=profile?:return
        if(RemotePlayback.target!=null) RemotePlayback.play(track)
        else { PlaybackQueue.enqueue(track,selected,sessions.token(selected.id));PlaybackQueue.play(this,PlaybackQueue.entries.lastIndex) }
    }
    private fun enqueueTrack(track: Track) {
        val selected=profile?:return
        if(RemotePlayback.target!=null) RemotePlayback.enqueue(track) else PlaybackQueue.enqueue(track,selected,sessions.token(selected.id))
        playbackControls.refresh();AppMessages.show("Added ${track.title} to queue")
    }
    private fun addTrack(track: Track) {
        val row=LinearLayout(this).apply { gravity=android.view.Gravity.CENTER_VERTICAL;minimumHeight=dp(72);background=touchSurface();setPadding(0,dp(8),0,dp(8));isFocusable=true;contentDescription="Play ${track.title}";setOnClickListener { playTrack(track) } }
        row.addView(trackCover(track.albumId,track.title))
        row.addView(LinearLayout(this).apply {
            orientation=LinearLayout.VERTICAL;setPadding(dp(12),0,dp(4),0)
            addView(musicText(this@MainActivity,track.title,15f).apply { setTypeface(typeface,android.graphics.Typeface.BOLD) })
            addView(musicText(this@MainActivity,listOfNotNull(track.artistName,formatTrackTime(track.durationMs)).joinToString(" · "),12f,true))
        },LinearLayout.LayoutParams(0,-2,1f))
        row.addView(iconButton(this,MusicIcon.PLUS,"Add ${track.title} to queue") { enqueueTrack(track) },LinearLayout.LayoutParams(dp(44),dp(48)))
        val more=iconButton(this,MusicIcon.MORE,"More for ${track.title}") {}
        more.setOnClickListener {
            android.widget.PopupMenu(this,more).apply {
                menu.add("Play now").setOnMenuItemClickListener { playTrack(track);true }
                menu.add("Add to queue").setOnMenuItemClickListener { enqueueTrack(track);true }
                show()
            }
        }
        row.addView(more,LinearLayout.LayoutParams(dp(44),dp(48)));content.addView(row)
    }
    private fun formatTrackTime(ms:ULong)="${ms/60000uL}:${(ms/1000uL%60uL).toString().padStart(2,'0')}"
    private fun showTracks(items: List<Track>, album: Album? = null) {
        val tracks = if (album == null) items else items.map { it.copy(albumTitle = it.albumTitle ?: album.title,
            artistName = it.artistName ?: album.artistName, albumId = it.albumId ?: album.id) }
        beginScreen()
        val first=tracks.firstOrNull()
        if(first?.albumId!=null) {
            val art=trackCover(first.albumId,first.albumTitle?:"Album")
            content.addView(art,LinearLayout.LayoutParams(dp(190),dp(190)).apply { gravity=android.view.Gravity.CENTER_HORIZONTAL;topMargin=dp(12);bottomMargin=dp(24) })
        }
        text(first?.albumTitle?:"Tracks",28f)
        if(!first?.artistName.isNullOrBlank()) text(first!!.artistName!!,15f)
        val heading=LinearLayout(this).apply { gravity=android.view.Gravity.CENTER_VERTICAL;setPadding(0,dp(8),0,dp(16)) }
        heading.addView(musicText(this,"${tracks.size} tracks",13f,true),LinearLayout.LayoutParams(0,-2,1f))
        heading.addView(iconButton(this,MusicIcon.PLAY,"Play album",true) {
            val selected=profile?:return@iconButton
            if(RemotePlayback.target!=null) { tracks.firstOrNull()?.let { RemotePlayback.play(it) };tracks.drop(1).forEach { RemotePlayback.enqueue(it) } }
            else { val start=PlaybackQueue.entries.size;tracks.forEach { PlaybackQueue.enqueue(it,selected,sessions.token(selected.id)) };if(tracks.isNotEmpty()) PlaybackQueue.play(this,start) }
        },LinearLayout.LayoutParams(dp(56),dp(56)))
        content.addView(heading);tracks.forEach { addTrack(it) }
    }
    private fun showQueue(refresh: Boolean = false) {
        if(!refresh && content===queueScreen) return
        val scroll=screenHost.getChildAt(0) as? ScrollView
        val offset=if(refresh) scroll?.scrollY?:0 else 0
        if(!refresh) beginScreen() else content.removeAllViews()
        queueScreen=content;queueGeneration++
        content.addView(LinearLayout(this).apply {
            gravity=android.view.Gravity.CENTER_VERTICAL
            addView(musicText(this@MainActivity,"Your queue",28f).apply { setTypeface(typeface,android.graphics.Typeface.BOLD) },LinearLayout.LayoutParams(0,-2,1f))
            addView(musicChip(this@MainActivity,if(queueEditing) "Done" else "Edit",queueEditing) { queueEditing=!queueEditing;showQueue(true) })
        })
        val count=if(RemotePlayback.target!=null) RemotePlayback.state?.queue?.size?:0 else PlaybackQueue.entries.size
        content.addView(LinearLayout(this).apply {
            gravity=android.view.Gravity.CENTER_VERTICAL;setPadding(0,dp(4),0,dp(16))
            addView(musicText(this@MainActivity,"$count tracks · ${RemotePlayback.target?.name?:"This phone"}",13f,true),LinearLayout.LayoutParams(0,-2,1f))
            addView(musicText(this@MainActivity,"Undo",14f).apply { minHeight=dp(48);gravity=android.view.Gravity.CENTER;setPadding(dp(12),0,dp(12),0);setTextColor(getColor(R.color.ichoi_accent));background=touchSurface();isFocusable=true;setOnClickListener { if(RemotePlayback.target!=null) RemotePlayback.undo() else { PlaybackQueue.undo();showQueue(true) };playbackControls.refresh() } })
        })
        if(RemotePlayback.target!=null) {
            RemotePlayback.error?.let { text(it);content.addView(button("Retry") { RemotePlayback.refresh() }) }
            val entries=RemotePlayback.state?.queue.orEmpty()
            if(entries.isEmpty()) text(if(RemotePlayback.state==null) "Loading device queue…" else "This device’s queue is empty.")
            entries.forEachIndexed { index,item -> queueRow(item.title?:item.trackId,item.artist?:"",null,index,entries.size,
                { RemotePlayback.playItem(item) },{ RemotePlayback.move(index,index-1) },{ RemotePlayback.move(index,index+1) },{ RemotePlayback.remove(item) }) }
        } else {
            if(PlaybackQueue.entries.isEmpty()) text("Add music or an audiobook to start your queue.")
            PlaybackQueue.entries.forEachIndexed { index,entry -> queueRow(entry.track.title,entry.track.artistName?:formatTrackTime(entry.track.durationMs),entry.track.albumId,index,PlaybackQueue.entries.size,
                { PlaybackQueue.play(this,index);showQueue(true) },{ PlaybackQueue.move(index,index-1);showQueue(true) },{ PlaybackQueue.move(index,index+1);showQueue(true) },{ PlaybackQueue.remove(index);showQueue(true);playbackControls.refresh() }) }
        }
        if(refresh) content.post { scroll?.scrollTo(0,offset) }
    }
    private fun queueRow(title:String,subtitle:String,album:String?,index:Int,count:Int,play:()->Unit,up:()->Unit,down:()->Unit,remove:()->Unit) {
        val row=LinearLayout(this).apply { gravity=android.view.Gravity.CENTER_VERTICAL;minimumHeight=dp(72);setPadding(0,dp(8),0,dp(8));background=touchSurface();isFocusable=true;contentDescription="Play $title";setOnClickListener { play() } }
        if(!queueEditing) row.addView(trackCover(album,title))
        row.addView(LinearLayout(this).apply { orientation=LinearLayout.VERTICAL;setPadding(dp(12),0,dp(6),0);addView(musicText(this@MainActivity,title,15f).apply { setTypeface(typeface,android.graphics.Typeface.BOLD) });addView(musicText(this@MainActivity,subtitle,12f,true)) },LinearLayout.LayoutParams(0,-2,1f))
        if(queueEditing) {
            row.addView(iconButton(this,MusicIcon.UP,"Move $title up",action=up).apply { isEnabled=index>0;alpha=if(isEnabled) 1f else .3f },LinearLayout.LayoutParams(dp(44),dp(48)))
            row.addView(iconButton(this,MusicIcon.DOWN,"Move $title down",action=down).apply { isEnabled=index+1<count;alpha=if(isEnabled) 1f else .3f },LinearLayout.LayoutParams(dp(44),dp(48)))
            row.addView(iconButton(this,MusicIcon.REMOVE,"Remove $title",action=remove),LinearLayout.LayoutParams(dp(44),dp(48)))
        } else {
            val more=iconButton(this,MusicIcon.MORE,"More for $title") {}
            more.setOnClickListener { android.widget.PopupMenu(this,more).apply {
                menu.add("Move up").apply { isEnabled=index>0;setOnMenuItemClickListener { up();true } }
                menu.add("Move down").apply { isEnabled=index+1<count;setOnMenuItemClickListener { down();true } }
                menu.add("Remove from queue").setOnMenuItemClickListener { remove();true };show()
            } }
            row.addView(more,LinearLayout.LayoutParams(dp(44),dp(48)))
            val grip=iconButton(this,MusicIcon.GRIP,"Hold and drag $title to reorder") {}
            grip.setOnLongClickListener { row.startDragAndDrop(android.content.ClipData.newPlainText("queue",index.toString()),android.view.View.DragShadowBuilder(row),QueueDrag(index,queueGeneration),0) }
            row.addView(grip,LinearLayout.LayoutParams(dp(44),dp(48)))
        }
        val generation=queueGeneration
        row.setOnDragListener { view,event ->
            val drag=event.localState as? QueueDrag
            if(drag==null || drag.generation!=generation) false else {
                when(event.action) {
                    android.view.DragEvent.ACTION_DRAG_ENTERED -> view.alpha=.55f
                    android.view.DragEvent.ACTION_DRAG_EXITED,android.view.DragEvent.ACTION_DRAG_ENDED -> view.alpha=1f
                    android.view.DragEvent.ACTION_DROP -> { view.alpha=1f;if(RemotePlayback.target!=null) RemotePlayback.move(drag.index,index) else { PlaybackQueue.move(drag.index,index);showQueue(true) } }
                };true
            }
        }
        content.addView(row)
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
        val input = EditText(this).apply {
            hint="Artists, albums, and tracks";inputType=android.text.InputType.TYPE_CLASS_TEXT;isSingleLine=true;textSize=16f
            background=surface();setPadding(dp(16),dp(12),dp(16),dp(12));minHeight=dp(52)
            layoutParams=LinearLayout.LayoutParams(-1,-2).apply { topMargin=dp(12);bottomMargin=dp(18) }
            setCompoundDrawablesWithIntrinsicBounds(null,null,null,null)
        }
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
                results.addView(musicText(this@MainActivity, "Searching…", 14f, true))
                handler.postDelayed({
                    if (version != revision) return@postDelayed
                    execute({ listOf(api.search(query, Library.Music), api.search(query, Library.Audiobook)) }, { pages, error ->
                        if (version != revision || !retainsScreen(screen)) return@execute
                        results.removeAllViews()
                        if (error != null) {
                            results.addView(musicText(this@MainActivity, "Search failed. Try again.", 14f, true))
                            results.addView(button("Retry search") { input.setText(query); input.setSelection(query.length) })
                            return@execute
                        }
                        if (pages!!.all { it.albums.isEmpty() && it.artists.isEmpty() && it.tracks.isEmpty() }) {
                            results.addView(musicText(this@MainActivity, "No results. Try another search.", 14f, true))
                        }
                        val original = content; content = results
                        pages!!.forEachIndexed { pageIndex, page ->
                            val kind = if (pageIndex == 0) Library.Music else Library.Audiobook
                            if(page.albums.isNotEmpty()) text(if(kind==Library.Music) "Albums" else "Audiobooks",20f)
                            page.albums.forEach { album -> addArtworkRow(api, ArtworkKind.Album, album.id, album.title, album.artistName ?: "") {
                                execute({ api.album(album.id) }, { detail, failure -> if (content === screen && failure == null) showTracks(detail!!.tracks, detail.album) })
                            } }
                            if(page.artists.isNotEmpty()) text("Artists",20f)
                            page.artists.forEach { artist -> content.addView(musicChip(this@MainActivity,artist.name) {
                                execute({ api.artist(artist.id, kind) }, { detail, failure -> if (content === screen && failure == null) showAlbums(api, AlbumsResponse(detail!!.albums, detail.albums.size.toULong()), kind) })
                            }) }
                            if(page.tracks.isNotEmpty()) text("Songs",20f)
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

    private fun updateNavigation() {
        if(::backControl.isInitialized) backControl.visibility=if(history.isEmpty()) android.view.View.GONE else android.view.View.VISIBLE
        if(::bottomNavigation.isInitialized) bottomNavigation.visibility=if(profile==null) android.view.View.GONE else android.view.View.VISIBLE
        for((name,item) in navItems) {
            val color=getColor(if(name==activeTab) R.color.ichoi_accent else R.color.ichoi_muted)
            (item.getChildAt(0) as ImageView).setImageDrawable(MusicGlyph(item.getChildAt(0).tag as MusicIcon,color))
            (item.getChildAt(1) as TextView).setTextColor(color);item.isSelected=name==activeTab
        }
    }
    private fun retainsScreen(screen: LinearLayout) = content === screen ||
        history.any { it.first === screen } ||
        browseTabs.values.any { it.content === screen || it.history.any { entry -> entry.first === screen } }

    private fun openTab(name:String) {
        val selected=profile?:return
        if(activeTab==name && name!="Queue" && screenHost.childCount>0) {
            if(history.isNotEmpty()) {
                val first=history.first();history.clear();content=first.first;screenHost.removeAllViews();screenHost.addView(first.second);updateNavigation()
            }
            return
        }
        if(::content.isInitialized && screenHost.childCount>0) browseTabs[activeTab]=BrowseTab(content,screenHost.getChildAt(0) as ScrollView,history.toList())
        activeTab=name
        val saved=browseTabs[name]
        if(saved!=null) {
            content=saved.content;history.clear();history.addAll(saved.history);screenHost.removeAllViews();screenHost.addView(saved.scroll)
            if(name=="Queue") showQueue(true)
        } else {
            history.clear();screenHost.removeAllViews()
            when(name) { "Library" -> connect(selected); "Search" -> showSearch(IchoiApi(selected,sessions.token(selected.id))); "Queue" -> showQueue() }
        }
        updateNavigation()
    }
    private fun dp(value: Int) = (value * resources.displayMetrics.density).toInt()
    private fun beginScreen(reset: Boolean = false) {
        if (reset) history.clear()
        else if (::content.isInitialized && screenHost.childCount > 0) {
            history.addLast(content to (screenHost.getChildAt(0) as ScrollView))
            while (history.size > 12) history.removeFirst()
        }
        content = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(dp(20), dp(12), dp(20), dp(20)) }
        screenHost.removeAllViews()
        screenHost.addView(ScrollView(this).apply { isFillViewport = true; addView(content) })
        updateNavigation()
    }
    private fun text(value: String, size: Float = 16f) {
        content.addView(TextView(this).apply {
            text = value; textSize = size; setTextColor(getColor(R.color.ichoi_text)); setPadding(0, dp(8), 0, dp(8))
            if (size >= 24f) setTypeface(typeface, android.graphics.Typeface.BOLD)
        })
    }
    private fun button(label: String, action: () -> Unit) = appButton(this, label, action = action)

    private fun showNavigationMenu(anchor: android.view.View) {
        val popup = android.widget.PopupMenu(this, anchor)
        fun item(label: String, action: () -> Unit) { popup.menu.add(label).setOnMenuItemClickListener { action(); true } }
        profile?.let { selected ->
            val api = IchoiApi(selected, sessions.token(selected.id))
            item("Library") { openTab("Library") }
            item("Search") { openTab("Search") }
            fun albums(kind: Library) { execute({ api.albums(kind) }, { result, error ->
                if (profile?.id != selected.id) return@execute
                if (error != null) showError(error.message ?: "Could not load albums") else showAlbums(api, result!!, kind)
            }) }
            item("Music") { albums(Library.Music) }
            item("Audiobooks") { albums(Library.Audiobook) }
            fun artists(kind: Library) { execute({ api.artists(kind) }, { result, error ->
                if (profile?.id != selected.id) return@execute
                if (error != null) showError(error.message ?: "Could not load artists") else showArtists(api, result!!, kind)
            }) }
            item("Artists") { artists(Library.Music) }
            item("Authors") { artists(Library.Audiobook) }
            item("Playlists") { execute({ api.playlists() }, { result, error ->
                if (profile?.id != selected.id) return@execute
                if (error != null) showError(error.message ?: "Could not load playlists") else showPlaylists(api, result!!)
            }) }
            item("Playback devices") { showDevicePicker() }
            item("Queue") { openTab("Queue") }
            item("Settings") { showSettings(api) }
        }
        item("Servers") { showWelcome() }
        item("App and updates") { showAppUpdates() }
        popup.show()
    }
    private fun link(label: String, url: String) {
        content.addView(button(label) { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) })
    }
    private fun showError(message: String) {
        AppMessages.show(message)
    }

    @Suppress("DEPRECATION")
    private fun installerName(): String? = packageManager.getInstallerPackageName(packageName)

    private fun updateMessage(state: PlayUpdates.State): String = when (state) {
        PlayUpdates.State.IDLE -> "Check for updates in Google Play."
        PlayUpdates.State.CHECKING -> "Checking Google Play…"
        PlayUpdates.State.CURRENT -> "Google Play has no update for this account and device."
        PlayUpdates.State.AVAILABLE -> "Update available · Download"
        PlayUpdates.State.DOWNLOADING -> "Google Play is preparing the update…"
        PlayUpdates.State.READY -> "Update ready · Restart to install"
        PlayUpdates.State.FAILED -> "Could not update. Try again or open Google Play."
        PlayUpdates.State.UNAVAILABLE -> "Open Google Play to check this installation."
    }
    private fun handleUpdateAction() {
        if (updates.state == PlayUpdates.State.READY) {
            AlertDialog.Builder(this).setTitle("Restart to install?")
                .setMessage("This closes Ichoi and interrupts playback on this phone. You can install later.")
                .setNegativeButton("Later", null).setPositiveButton("Restart") { _, _ -> updates.install() }.show()
        } else if (installerName() == "com.android.vending") updates.check(requestInstall = true)
        else openPlayStore()
    }
    private fun openPlayStore() {
        try { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=com.catalystsquad.ichoi"))) }
        catch (_: android.content.ActivityNotFoundException) { showError("Open Google Play and select Ichoi to check for updates.") }
    }
    private fun showAppUpdates() {
        beginScreen()
        text("App and updates", 24f)
        text("Ichoi ${community.catalyst.ichoi.BuildConfig.VERSION_NAME} (${community.catalyst.ichoi.BuildConfig.VERSION_CODE})")
        text("Application: $packageName")
        text("Installed by: ${installerName() ?: "Unknown or direct install"}")
        @Suppress("DEPRECATION")
        val info = packageManager.getPackageInfo(packageName, android.content.pm.PackageManager.GET_SIGNING_CERTIFICATES)
        val fingerprint = info.signingInfo?.apkContentsSigners?.firstOrNull()?.toByteArray()?.let {
            java.security.MessageDigest.getInstance("SHA-256").digest(it).joinToString(":") { byte -> "%02X".format(byte) }
        }
        if (fingerprint != null) text("Signing certificate (SHA-256):\n$fingerprint", 12f)
        updateDetails = TextView(this).apply { text = updateMessage(updates.state); setPadding(dp(4), dp(12), dp(4), dp(12)) }
        content.addView(updateDetails)
        content.addView(button("Check for updates") { handleUpdateAction() })
        content.addView(button("Open Google Play") { openPlayStore() })
        text("Join the test once with your Play account. Keep automatic updates enabled for Ichoi in Google Play. Later releases use the same store listing.")
        text("If an update fails, keep the error message and the version details above. An app cannot repair a signing or test-track mismatch.")
    }

    private fun messageDurationSetting() {
        val preferences = MessagePreferences(this)
        val setting = button("Message duration") {}
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
        if (installerName() == "com.android.vending") updates.start() else updates.unavailable()
        playbackControls.start()
        RemotePlayback.observe(remoteChanged); RemotePlayback.start()
    }

    override fun onStop() {
        messageBanner.stop()
        updates.stop()
        playbackControls.stop()
        RemotePlayback.removeObserver(remoteChanged); RemotePlayback.stop()
        super.onStop()
    }

    override fun onDestroy() { updates.close(); messageBanner.stop(); playbackControls.stop(); worker.shutdownNow(); super.onDestroy() }

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
