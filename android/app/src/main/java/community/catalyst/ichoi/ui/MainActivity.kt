package community.catalyst.ichoi.ui

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import community.catalyst.csilgen.generated.*
import community.catalyst.ichoi.api.IchoiApi
import community.catalyst.ichoi.api.LinkKeysLogin
import community.catalyst.ichoi.playback.PlaybackService
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

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        profiles = ServerProfileStore(this)
        sessions = ProfileSessions(SessionTokenStore(this))
        showWelcome()
        handleLoginCallback(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleLoginCallback(intent)
    }

    private fun showWelcome() {
        content = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(32, 40, 32, 24) }
        setContentView(ScrollView(this).apply { addView(content) })
        text("Ichoi", 26f)
        text("Ichoi is a native client for an Ichoi server. Your selected server operator controls the library and account data.")
        val terms = CheckBox(this).apply {
            text = "I accept the Terms of Use"
            isChecked = getPreferences(0).getBoolean(TERMS, false)
        }
        content.addView(terms)
        link("Read the Terms of Use", TERMS_URL)
        link("Read the Privacy Policy", PRIVACY_URL)
        val add = button("Add my server") { addServerDialog() }
        add.isEnabled = FirstRunPolicy.canConnect(terms.isChecked)
        content.addView(add)
        terms.setOnCheckedChangeListener { _, checked ->
            getPreferences(0).edit().putBoolean(TERMS, checked).apply()
            add.isEnabled = FirstRunPolicy.canConnect(checked)
        }
        profiles.all().forEach { saved ->
            val reconnect = button("Connect to ${saved.url}") { connect(saved) }
            reconnect.isEnabled = FirstRunPolicy.canConnect(terms.isChecked)
            content.addView(reconnect)
        }
        // There is no demo action until a real, reviewed demo URL is available.
    }

    private fun addServerDialog() {
        val fields = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(32, 8, 32, 0) }
        val url = EditText(this).apply { hint = "Server URL (https://...)"; inputType = 0x10 or 0x80000 }
        val pin = EditText(this).apply { hint = "Pinned SHA-256 fingerprints, separated by commas" }
        fields.addView(url)
        fields.addView(pin)
        AlertDialog.Builder(this).setTitle("Add my server").setView(fields).setNegativeButton("Cancel", null)
            .setPositiveButton("Connect") { _, _ -> validateAndConnect(url.text.toString(), pin.text.toString()) }.show()
    }

    private fun validateAndConnect(raw: String, pin: String) {
        when (val decision = ConnectionSecurity.check(raw)) {
            EndpointDecision.Secure -> {
                if (pin.isBlank()) showError("At least one TLS pin is required for HTTPS")
                else connect(profiles.put(raw.trim(), pin, false))
            }
            is EndpointDecision.PrivateHttpNeedsConsent -> AlertDialog.Builder(this)
                .setTitle("Use an unencrypted private connection?")
                .setMessage("Only use plain HTTP on a private network. Other people on that network can read this connection.")
                .setNegativeButton("Cancel", null)
                .setPositiveButton("I understand") { _, _ -> connect(profiles.put(raw.trim(), "", true)) }.show()
            is EndpointDecision.Rejected -> showError(decision.reason)
        }
    }

    private fun connect(selected: ServerProfile) {
        profile = selected
        val token = sessions.token(selected.id)
        val api = IchoiApi(selected, token)
        execute({
            val libraries = api.libraries()
            val restored = if (token == null) null else runCatching { api.whoAmI() }.getOrNull()
            libraries to restored
        }, { result, error ->
            if (error != null) showError("Could not connect to ${selected.url}: ${error.message ?: "connection failed"}")
            else {
                sessionInfo = result!!.second
                showLibrary(api, result.first)
            }
        })
    }

    private fun showLibrary(api: IchoiApi, libraries: LibrariesResponse) {
        content.removeAllViews()
        text("Your Ichoi library", 24f)
        text("Server: ${profile?.url}")
        libraries.libraries.forEach { info -> content.addView(button(info.kind.name) {
            execute({ api.albums(info.kind) }, { result, error ->
                if (error != null) showError(error.message ?: "Could not load albums") else showAlbums(api, result!!)
            })
        }) }
        content.addView(button("Playlists") {
            execute({ api.playlists() }, { result, error ->
                if (error != null) showError(error.message ?: "Could not load playlists") else showPlaylists(api, result!!)
            })
        })
        content.addView(button("Settings") { showSettings(api) })
    }

    private fun showAlbums(api: IchoiApi, albums: AlbumsResponse) {
        content.removeAllViews()
        text("Albums", 24f)
        albums.albums.forEach { album -> content.addView(button(album.title) {
            execute({ api.album(album.id) }, { detail, error ->
                if (error != null) showError(error.message ?: "Could not load album") else showTracks(detail!!.tracks)
            })
        }) }
        content.addView(button("Back") { connect(profile!!) })
    }

    private fun showPlaylists(api: IchoiApi, playlists: PlaylistsResponse) {
        content.removeAllViews()
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
            }
        }
        content.addView(button("Back") { connect(profile!!) })
    }

    private fun showTracks(tracks: List<Track>) {
        content.removeAllViews()
        text("Tracks", 24f)
        tracks.forEach { track -> content.addView(button("Play ${track.title}") {
            val selected = profile ?: return@button
            val uri = selected.url.trimEnd('/') + "/media/" + Uri.encode(track.id)
            startForegroundService(Intent(this, PlaybackService::class.java)
                .putExtra(PlaybackService.EXTRA_URL, uri)
                .putExtra(PlaybackService.EXTRA_TITLE, track.title)
                .putExtra(PlaybackService.EXTRA_TOKEN, sessions.token(selected.id))
                .putExtra(PlaybackService.EXTRA_PROFILE_ID, selected.id)
                .putExtra(PlaybackService.EXTRA_SERVER_URL, selected.url)
                .putExtra(PlaybackService.EXTRA_PIN, selected.pinnedSha256)
                .putExtra(PlaybackService.EXTRA_PRIVATE_HTTP, selected.allowPrivateHttp))
        }) }
        content.addView(button("Back") { connect(profile!!) })
    }

    private fun showSettings(api: IchoiApi) {
        content.removeAllViews()
        text("Settings", 24f)
        text("Server: ${profile?.url}")
        link("Privacy Policy", PRIVACY_URL)
        link("Terms of Use", TERMS_URL)
        val current = sessionInfo
        if (current != null) {
            text("Signed in as ${current.handle}")
            content.addView(button("Sign out") {
                execute({ runCatching { api.signOut() }; Unit }, { _, _ ->
                    sessions.signOut(profile!!.id)
                    sessionInfo = null
                    showWelcome()
                })
            })
            content.addView(button("Delete account") { deleteAccount(api, current.handle) })
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
        content.addView(button("Back to library") { connect(profile!!) })
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
            .setMessage("This removes your account data from ${profile?.url}.")
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
                    { output -> runOnUiThread { result(output, null) } },
                    { error -> runOnUiThread { result(null, error) } },
                )
            }
        }) }
    }

    private fun text(value: String, size: Float = 16f) {
        content.addView(TextView(this).apply { text = value; textSize = size; setPadding(0, 12, 0, 12) })
    }
    private fun button(label: String, action: () -> Unit) = Button(this).apply { text = label; setOnClickListener { action() } }
    private fun link(label: String, url: String) {
        content.addView(button(label) { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) })
    }
    private fun showError(message: String) {
        AlertDialog.Builder(this).setMessage(message).setPositiveButton("OK", null).show()
    }

    override fun onDestroy() { worker.shutdownNow(); super.onDestroy() }

    companion object {
        private const val TERMS = "termsAccepted"
        private const val PENDING_LOGIN_PROFILE = "pendingLoginProfile"
        private const val TERMS_URL = "https://ichoi.invalid/terms"
        private const val PRIVACY_URL = "https://ichoi.invalid/privacy"
    }
}
