package community.catalyst.ichoi.ui

import android.content.Context
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.util.LruCache
import android.widget.ImageView
import community.catalyst.csilgen.generated.ArtworkKind
import community.catalyst.ichoi.R
import community.catalyst.ichoi.api.IchoiApi
import java.util.concurrent.Executors
import kotlin.coroutines.*

/** Shared thumbnails avoid a new request for every occurrence of the same album. */
object MusicArtwork {
    private val cache = object : LruCache<String, Bitmap>(24 * 1024 * 1024) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount
    }
    private val main = Handler(Looper.getMainLooper())
    private val executor = Executors.newFixedThreadPool(2)
    private data class Request(val key: String, val api: IchoiApi, val kind: ArtworkKind, val id: String, val generation: Long)
    private val waiting = ArrayDeque<Request>()
    private val callbacks = mutableMapOf<String, MutableList<(Bitmap?) -> Unit>>()
    private var active = 0
    private var generation = 0L
    fun clear() { generation++; cache.evictAll() }
    fun load(api: IchoiApi, scope: String, kind: ArtworkKind, id: String, result: (Bitmap?) -> Unit) {
        val key = "$generation:$scope:$kind:$id"
        cache.get(key)?.let { result(it); return }
        callbacks[key]?.let { it.add(result); return }
        callbacks[key] = mutableListOf(result); waiting.add(Request(key,api,kind,id,generation)); drain()
    }
    private fun drain() {
        while (active < 2 && waiting.isNotEmpty()) {
            val request = waiting.removeFirst(); active++
            executor.execute {
                suspend { ArtworkImages.decode(request.api.artwork(request.kind, request.id).data) }.startCoroutine(object : Continuation<Bitmap> {
                    override val context = EmptyCoroutineContext
                    override fun resumeWith(result: Result<Bitmap>) { main.post {
                        val bitmap = result.getOrNull()
                        if (bitmap != null && request.generation == generation) cache.put(request.key,bitmap)
                        val listeners = callbacks.remove(request.key).orEmpty(); active--
                        listeners.forEach { it(bitmap) }; drain()
                    } }
                })
            }
        }
    }
}
class MusicCover(context: Context, size: Int) : ImageView(context) {
    init {
        layoutParams=android.widget.LinearLayout.LayoutParams(context.dp(size),context.dp(size))
        scaleType=ScaleType.CENTER_CROP; background=context.surface(R.color.ichoi_surface,6); clipToOutline=true
        placeholder()
    }
    fun placeholder() { setImageDrawable(MusicGlyph(MusicIcon.LIBRARY,context.getColor(R.color.ichoi_muted))); setPadding(context.dp(10),context.dp(10),context.dp(10),context.dp(10)) }
    fun show(bitmap: Bitmap?) { if(bitmap != null) { setPadding(0,0,0,0); setImageBitmap(bitmap) } else placeholder() }
}
