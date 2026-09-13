package community.catalyst.ichoi.playback

import android.media.MediaDataSource
import community.catalyst.ichoi.security.PinnedNetwork
import community.catalyst.ichoi.storage.ServerProfile
import java.io.IOException
import okhttp3.Request

/** Reads media ranges through the selected pinned connection without a disk cache. */
class PinnedMediaDataSource(
    private val profile: ServerProfile,
    private val url: String,
    private val token: String?,
) : MediaDataSource() {
    private val client = PinnedNetwork.client(profile)
    @Volatile private var closed = false
    @Volatile private var knownSize = -1L

    override fun readAt(position: Long, buffer: ByteArray, offset: Int, size: Int): Int {
        if (closed) throw IOException("Media source is closed")
        if (knownSize >= 0 && position >= knownSize) return -1
        val end = position + size - 1
        val request = Request.Builder().url(url).header("Range", "bytes=$position-$end").apply {
            token?.let { header("Authorization", "Bearer $it") }
        }.build()
        return client.newCall(request).execute().use { response ->
            if (response.code !in listOf(200, 206)) throw IOException("Media request failed (${response.code})")
            updateSize(response.header("Content-Range"), response.header("Content-Length"), position)
            val bytes = response.body?.bytes() ?: return@use -1
            if (response.code == 200 && position != 0L) throw IOException("Server did not honor the media range")
            val count = minOf(size, bytes.size)
            if (count == 0) -1 else {
                bytes.copyInto(buffer, offset, 0, count)
                count
            }
        }
    }

    override fun getSize(): Long = knownSize
    override fun close() { closed = true; client.dispatcher.cancelAll() }

    private fun updateSize(contentRange: String?, contentLength: String?, position: Long) {
        val total = contentRange?.substringAfterLast('/')?.toLongOrNull()
        if (total != null) knownSize = total
        else if (position == 0L) contentLength?.toLongOrNull()?.let { knownSize = it }
    }
}
