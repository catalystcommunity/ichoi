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
class ArtworkRow(context: Context, title: String, subtitle: String = "") : LinearLayout(context) {
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
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(0, 12, 0, 12)
        isFocusable = true
        val size = (96 * resources.displayMetrics.density).toInt()
        addView(FrameLayout(context).apply {
            addView(fallback, FrameLayout.LayoutParams(size, size))
            addView(image, FrameLayout.LayoutParams(size, size))
        }, LayoutParams(size, size))
        addView(TextView(context).apply {
            text = if (subtitle.isBlank()) title else "$title\n$subtitle"
            textSize = 17f
            setPadding(20, 0, 0, 0)
        }, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
    }
    fun showCover(bitmap: Bitmap?) {
        image.setImageBitmap(bitmap)
        image.visibility = if (bitmap == null) View.GONE else View.VISIBLE
        fallback.visibility = if (bitmap == null) View.VISIBLE else View.GONE
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
