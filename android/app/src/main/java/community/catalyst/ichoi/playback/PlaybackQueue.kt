package community.catalyst.ichoi.playback

import android.content.Context
import android.content.Intent
import android.net.Uri
import community.catalyst.csilgen.generated.Track
import community.catalyst.ichoi.storage.ServerProfile

/** In-memory queue. Browsing views do not own playback state. */
object PlaybackQueue {
    data class Entry(val track: Track, val profile: ServerProfile, val token: String?, val id: String = java.util.UUID.randomUUID().toString())
    val entries = mutableListOf<Entry>()
    var currentId: String? = null
        private set
    private var undo: List<Entry>? = null
    fun enqueue(track: Track, profile: ServerProfile, token: String?) { undo = entries.toList(); entries.add(Entry(track, profile, token)) }
    fun remove(index: Int) { undo = entries.toList(); entries.removeAt(index) }
    fun move(index: Int, to: Int) { if (to !in entries.indices) return; undo = entries.toList(); entries.add(to, entries.removeAt(index)) }
    fun undo() { undo?.let { entries.clear(); entries.addAll(it) }; undo = null }
    fun play(context: Context, index: Int) {
        val entry = entries.getOrNull(index) ?: return
        currentId = entry.id
        context.startForegroundService(Intent(context, PlaybackService::class.java)
            .putExtra(PlaybackService.EXTRA_URL, "${entry.profile.url.trimEnd('/')}/media/${Uri.encode(entry.track.id)}")
            .putExtra(PlaybackService.EXTRA_DURATION, entry.track.durationMs.toLong())
            .putExtra(PlaybackService.EXTRA_TITLE, entry.track.title).putExtra(PlaybackService.EXTRA_TOKEN, entry.token)
            .putExtra(PlaybackService.EXTRA_PROFILE_ID, entry.profile.id).putExtra(PlaybackService.EXTRA_SERVER_URL, entry.profile.url)
            .putExtra(PlaybackService.EXTRA_PRIVATE_HTTP, entry.profile.allowPrivateHttp))
    }
    fun completed(context: Context) {
        val index = entries.indexOfFirst { it.id == currentId }
        if (index >= 0 && index + 1 < entries.size) play(context, index + 1)
    }
}
