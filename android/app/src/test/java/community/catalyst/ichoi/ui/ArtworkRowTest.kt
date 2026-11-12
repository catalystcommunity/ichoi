package community.catalyst.ichoi.ui

import android.graphics.Bitmap
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.TextView
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class ArtworkRowTest {
    private fun descendants(view: View): List<View> = listOf(view) +
        if (view is ViewGroup) (0 until view.childCount).flatMap { descendants(view.getChildAt(it)) } else emptyList()

    @Test fun placeholderUsesTheAlbumNameIncludingParentheses() {
        val row = ArtworkRow(RuntimeEnvironment.getApplication(), "(Selections) Night Music", "The Night Orchestra")
        val labels = descendants(row).filterIsInstance<TextView>().map { it.text.toString() }
        assertTrue(labels.contains("(Selections) Night Music"))
        assertFalse(labels.contains("("))
        assertTrue(labels.any { it.contains("The Night Orchestra") })
    }

    @Test fun resetRestoresTheNamedPlaceholder() {
        val row = ArtworkRow(RuntimeEnvironment.getApplication(), "Night Music")
        val image = descendants(row).filterIsInstance<ImageView>().single()
        row.showCover(Bitmap.createBitmap(10, 10, Bitmap.Config.ARGB_8888))
        assertEquals(View.VISIBLE, image.visibility)
        row.showCover(null)
        assertEquals(View.GONE, image.visibility)
        assertTrue(descendants(row).filterIsInstance<TextView>().any { it.visibility == View.VISIBLE && it.text == "Night Music" })
    }

    @Test @org.robolectric.annotation.GraphicsMode(org.robolectric.annotation.GraphicsMode.Mode.NATIVE)
    fun validArtworkDecodesWithoutAZeroSamplingDivisor() {
        val source = Bitmap.createBitmap(64,64,Bitmap.Config.ARGB_8888)
        val bytes = java.io.ByteArrayOutputStream().use { output -> source.compress(Bitmap.CompressFormat.PNG,100,output);output.toByteArray() }
        val decoded = ArtworkImages.decode(bytes)
        assertEquals(64,decoded.width)
        assertEquals(64,decoded.height)
        source.recycle();decoded.recycle()
    }

    @Test fun oversizedImagesAreRejectedBeforeDecode() {
        val result = runCatching { ArtworkImages.upload(ByteArray(ArtworkImages.MAX_BYTES + 1)) }
        assertTrue(result.exceptionOrNull() is IllegalArgumentException)
        assertTrue(result.exceptionOrNull()!!.message!!.contains("8 MB"))
    }
}
