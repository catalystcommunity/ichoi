package community.catalyst.ichoi.ui

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import java.io.ByteArrayOutputStream

/** A named placeholder stays visible until a valid cover is available. */
class ArtworkRow(context: Context, title: String, subtitle: String = "", tile: Boolean = false) : LinearLayout(context) {
    private val fallback = TextView(context).apply {
        text = title.trim().ifBlank { "Untitled album" }
        textSize = 13f
        gravity = Gravity.CENTER
        setPadding(8, 8, 8, 8)
        setBackgroundColor(context.getColor(community.catalyst.ichoi.R.color.ichoi_surface))
    }
    private val image = ImageView(context).apply {
        scaleType = ImageView.ScaleType.CENTER_CROP
        contentDescription = "Cover art for $title"
        visibility = View.GONE
    }
    init {
        orientation = if (tile) VERTICAL else HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(0, 0, 0, (16 * resources.displayMetrics.density).toInt())
        isFocusable = true
        val density = resources.displayMetrics.density
        val size = ((if (tile) (resources.configuration.screenWidthDp - 56) / 2 else 72) * density).toInt()
        addView(FrameLayout(context).apply {
            background = android.graphics.drawable.GradientDrawable().apply { cornerRadius = 10 * density; setColor(context.getColor(community.catalyst.ichoi.R.color.ichoi_surface)) }
            clipToOutline = true
            addView(fallback, FrameLayout.LayoutParams(-1, -1))
            addView(image, FrameLayout.LayoutParams(-1, -1))
        }, LayoutParams(size, size))
        val copy = LinearLayout(context).apply {
            orientation = VERTICAL
            setPadding(if (tile) 0 else (14 * density).toInt(), if (tile) (9 * density).toInt() else 0, 0, 0)
            addView(TextView(context).apply {
                text = title; textSize = if (tile) 14f else 16f; maxLines = 2
                ellipsize = android.text.TextUtils.TruncateAt.END
                setTextColor(context.getColor(community.catalyst.ichoi.R.color.ichoi_text))
                setTypeface(typeface, android.graphics.Typeface.BOLD)
            })
            if (subtitle.isNotBlank()) addView(TextView(context).apply {
                text = subtitle; textSize = 12f; maxLines = 1; ellipsize = android.text.TextUtils.TruncateAt.END
                setTextColor(context.getColor(community.catalyst.ichoi.R.color.ichoi_muted))
            })
        }
        addView(copy, if (tile) LayoutParams(size, -2) else LayoutParams(0, -2, 1f))
    }

    fun showCover(bitmap: Bitmap?) {
        image.setImageBitmap(bitmap)
        image.visibility = if (bitmap == null) View.GONE else View.VISIBLE
        fallback.visibility = if (bitmap == null) View.VISIBLE else View.GONE
    }

    fun whenVisible(load: () -> Unit) {
        var loaded = false
        var observer: android.view.ViewTreeObserver? = null
        val bounds = android.graphics.Rect()
        fun check() {
            if (!loaded && isAttachedToWindow && getGlobalVisibleRect(bounds) && !bounds.isEmpty) {
                loaded = true
                load()
            }
        }
        val scroll = android.view.ViewTreeObserver.OnScrollChangedListener { check() }
        val layout = android.view.ViewTreeObserver.OnGlobalLayoutListener { check() }
        addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
            override fun onViewAttachedToWindow(view: View) {
                if (loaded) return
                observer = viewTreeObserver
                observer?.addOnScrollChangedListener(scroll)
                observer?.addOnGlobalLayoutListener(layout)
                post { check() }
            }
            override fun onViewDetachedFromWindow(view: View) {
                observer?.takeIf { it.isAlive }?.let {
                    it.removeOnScrollChangedListener(scroll)
                    it.removeOnGlobalLayoutListener(layout)
                }
                observer = null
            }
        })
        if (isAttachedToWindow) {
            observer = viewTreeObserver
            observer?.addOnScrollChangedListener(scroll)
            observer?.addOnGlobalLayoutListener(layout)
            post { check() }
        }
    }
}

object ArtworkImages {
    const val MAX_BYTES = 8 * 1024 * 1024
    fun decode(bytes: ByteArray): Bitmap {
        require(bytes.size <= MAX_BYTES) { "Choose an image no larger than 8 MB." }
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        require(bounds.outWidth in 1..8192 && bounds.outHeight in 1..8192) { "Choose a valid image no larger than 8192 pixels per side." }
        val options = BitmapFactory.Options().apply {
            inSampleSize = 1
            while (maxOf(bounds.outWidth, bounds.outHeight) / inSampleSize > 1600) inSampleSize *= 2
        }
        return requireNotNull(BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)) { "This image could not be read." }
    }
    fun upload(bytes: ByteArray): ByteArray {
        val bitmap = decode(bytes)
        return try { ByteArrayOutputStream().use { output -> bitmap.compress(Bitmap.CompressFormat.JPEG, 88, output); output.toByteArray() } }
        finally { bitmap.recycle() }
    }
}
