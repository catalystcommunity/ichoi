package community.catalyst.ichoi.playback

import android.media.MediaDataSource
import community.catalyst.ichoi.security.ServerNetwork
import community.catalyst.ichoi.storage.ServerProfile
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import java.util.concurrent.FutureTask
import java.util.concurrent.atomic.AtomicBoolean
import okhttp3.Request

/** Transient range cache. Prefetch ten minutes by container size and track duration. */
class ServerMediaDataSource(
    profile: ServerProfile,
    private val url: String,
    private val token: String?,
    private val durationMs: Long = 0,
) : MediaDataSource() {
    private val client = ServerNetwork.client(profile)
    @Volatile private var closed = false
    @Volatile private var knownSize = -1L
    private val cache = LinkedHashMap<Long, ByteArray>()
    private val pending = ConcurrentHashMap<Long, FutureTask<ByteArray>>()
    private val worker = Executors.newSingleThreadExecutor()
    private val warming = AtomicBoolean(false)
    @Volatile private var requestedBlock = 0L
    private var cachedBytes = 0

    private fun block(index: Long): ByteArray {
        synchronized(cache) { cache[index]?.let { return it } }
        val task = FutureTask {
            if (closed) throw IOException("Media source is closed")
            val start = index * BLOCK
            val request = Request.Builder().url(url).header("Range", "bytes=$start-${start + BLOCK - 1}").apply {
                token?.let { header("Authorization", "Bearer $it") }
            }.build()
            val data = client.newCall(request).execute().use { response ->
                val range = Regex("bytes (\\d+)-(\\d+)/(\\d+)").matchEntire(response.header("Content-Range") ?: "")
                if (response.code != 206 || range == null || range.groupValues[1].toLong() != start) throw IOException("Server did not honor the media range")
                val end = range.groupValues[2].toLong()
                if (end < start || end >= start + BLOCK) throw IOException("Invalid media range")
                knownSize = range.groupValues[3].toLong()
                val body = response.body ?: throw IOException("Empty media response")
                val bytes = ByteArray((end - start + 1).toInt())
                body.byteStream().use { stream ->
                    var read = 0
                    while (read < bytes.size) { val count = stream.read(bytes, read, bytes.size - read); if (count < 0) throw IOException("Incomplete media range"); read += count }
                }
                if (bytes.size.toLong() != end - start + 1) throw IOException("Incomplete media range")
                bytes
            }
            synchronized(cache) {
                if (!closed) {
                    while (cachedBytes + data.size > LIMIT && cache.isNotEmpty()) {
                        val first = cache.entries.iterator(); val entry = first.next(); cachedBytes -= entry.value.size; first.remove()
                    }
                    cache.put(index, data)?.let { cachedBytes -= it.size }; cachedBytes += data.size
                }
            }
            data
        }
        val existing = pending.putIfAbsent(index, task)
        return try { if (existing == null) task.run(); (existing ?: task).get() }
        catch (e: Exception) { throw IOException("Media range failed", e) }
        finally { if (existing == null) pending.remove(index, task) }
    }
    private fun prefetch(index: Long) {
        requestedBlock = index
        if (durationMs <= 0 || closed || !warming.compareAndSet(false, true)) return
        worker.execute {
            try {
                val start = requestedBlock
                val budget = minOf(LIMIT.toLong() - BLOCK, knownSize * minOf(durationMs, 600_000) / durationMs)
                val end = minOf(knownSize, start * BLOCK + budget)
                var next = start + 1
                while (!closed && requestedBlock == start && next * BLOCK < end) { block(next); next++ }
            } catch (_: Exception) { /* A playback read reports a persistent connection failure. */ }
            finally { warming.set(false) }
        }
    }
    override fun readAt(position: Long, buffer: ByteArray, offset: Int, size: Int): Int {
        if (closed) throw IOException("Media source is closed")
        if (size == 0) return 0
        if (position < 0 || offset < 0 || size < 0 || offset + size > buffer.size) throw IOException("Invalid media read")
        if (knownSize >= 0 && position >= knownSize) return -1
        val index = position / BLOCK
        val bytes = block(index)
        val from = (position % BLOCK).toInt()
        val count = minOf(size, bytes.size - from)
        if (count <= 0) return -1
        bytes.copyInto(buffer, offset, from, from + count)
        prefetch(index)
        return count
    }
    override fun getSize(): Long = knownSize
    override fun close() {
        closed = true; client.dispatcher.cancelAll(); worker.shutdownNow()
        synchronized(cache) { cache.clear(); cachedBytes = 0 }
    }
    companion object { private const val BLOCK = 512 * 1024L; private const val LIMIT = 128 * 1024 * 1024 }
}
