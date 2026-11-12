package community.catalyst.ichoi.ui

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.*
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.view.Gravity
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import community.catalyst.ichoi.R

fun Context.dp(value: Int) = (value * resources.displayMetrics.density).toInt()
fun Context.surface(color: Int = R.color.ichoi_surface, radius: Int = 12) = GradientDrawable().apply {
    setColor(getColor(color)); cornerRadius = dp(radius).toFloat()
}
fun Context.touchSurface(color: Int = android.R.color.transparent, radius: Int = 10) = RippleDrawable(
    ColorStateList.valueOf((getColor(R.color.ichoi_accent) and 0x00ffffff) or 0x22000000), surface(color, radius), surface(android.R.color.white, radius))
fun musicText(context: Context, value: String, size: Float = 15f, muted: Boolean = false) = TextView(context).apply {
    text = value; textSize = size; setTextColor(context.getColor(if (muted) R.color.ichoi_muted else R.color.ichoi_text))
    maxLines = 1; ellipsize = android.text.TextUtils.TruncateAt.END
}
enum class MusicIcon { PLAY, PAUSE, NEXT, PREVIOUS, QUEUE, DEVICE, MORE, MENU, SEARCH, LIBRARY, BACK, DOWN, PLUS, REMOVE, UP, STOP, GRIP }
class MusicGlyph(private val icon: MusicIcon, private val color: Int) : Drawable() {
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = this@MusicGlyph.color; strokeWidth = 1.8f; strokeCap = Paint.Cap.ROUND; strokeJoin = Paint.Join.ROUND }
    override fun draw(canvas: Canvas) {
        canvas.save(); canvas.translate(bounds.left.toFloat(), bounds.top.toFloat()); canvas.scale(bounds.width() / 24f, bounds.height() / 24f)
        paint.style = Paint.Style.STROKE
        fun line(x: Float, y: Float, a: Float, b: Float) = canvas.drawLine(x,y,a,b,paint)
        fun play(reverse: Boolean = false) { paint.style = Paint.Style.FILL; canvas.drawPath(Path().apply { moveTo(if(reverse) 17f else 7f,5f); lineTo(if(reverse) 6f else 18f,12f); lineTo(if(reverse) 17f else 7f,19f); close() },paint) }
        when(icon) {
            MusicIcon.PLAY -> play()
            MusicIcon.PAUSE -> { paint.style=Paint.Style.FILL; canvas.drawRoundRect(6f,5f,10f,19f,1f,1f,paint); canvas.drawRoundRect(14f,5f,18f,19f,1f,1f,paint) }
            MusicIcon.NEXT, MusicIcon.PREVIOUS -> { val back=icon==MusicIcon.PREVIOUS; play(back); canvas.drawRect(if(back) 4f else 19f,5f,if(back) 6f else 21f,19f,paint) }
            MusicIcon.STOP -> { paint.style=Paint.Style.FILL; canvas.drawRoundRect(6f,6f,18f,18f,2f,2f,paint) }
            MusicIcon.SEARCH -> { canvas.drawCircle(10f,10f,6f,paint); line(15f,15f,21f,21f) }
            MusicIcon.MORE -> { paint.style=Paint.Style.FILL; for(x in listOf(5f,12f,19f)) canvas.drawCircle(x,12f,1.6f,paint) }
            MusicIcon.MENU -> { for(y in listOf(6f,12f,18f)) line(4f,y,20f,y) }
            MusicIcon.QUEUE -> { for(y in listOf(6f,12f,18f)) { line(9f,y,20f,y); canvas.drawCircle(4f,y,0.7f,paint) } }
            MusicIcon.LIBRARY -> { canvas.drawRoundRect(3f,4f,8f,20f,1f,1f,paint); canvas.drawRoundRect(11f,4f,16f,20f,1f,1f,paint); line(19f,4f,22f,20f) }
            MusicIcon.DEVICE -> { canvas.drawRoundRect(3f,4f,16f,15f,2f,2f,paint); line(7f,19f,12f,19f); line(10f,15f,10f,19f); canvas.drawRoundRect(18f,9f,22f,21f,1f,1f,paint) }
            MusicIcon.BACK -> { line(17f,4f,9f,12f); line(9f,12f,17f,20f) }
            MusicIcon.DOWN -> { line(5f,9f,12f,16f); line(12f,16f,19f,9f) }
            MusicIcon.UP -> { line(5f,15f,12f,8f); line(12f,8f,19f,15f) }
            MusicIcon.PLUS -> { canvas.drawCircle(12f,12f,9f,paint); line(8f,12f,16f,12f); line(12f,8f,12f,16f) }
            MusicIcon.REMOVE -> { line(6f,6f,18f,18f); line(6f,18f,18f,6f) }
            MusicIcon.GRIP -> { line(6f,9f,18f,9f); line(6f,15f,18f,15f) }
        }
        canvas.restore()
    }
    override fun setAlpha(alpha: Int) { paint.alpha=alpha }
    override fun setColorFilter(filter: ColorFilter?) { paint.colorFilter=filter }
    @Deprecated("Deprecated in Java") override fun getOpacity() = PixelFormat.TRANSLUCENT
}
fun iconButton(context: Context, icon: MusicIcon, label: String, primary: Boolean = false, action: () -> Unit) = ImageButton(context).apply {
    contentDescription = label; minimumWidth = context.dp(48); minimumHeight = context.dp(48)
    background = context.touchSurface(if (primary) R.color.ichoi_accent else android.R.color.transparent, 28)
    setImageDrawable(MusicGlyph(icon, context.getColor(if(primary) R.color.ichoi_on_accent else R.color.ichoi_text)))
    setPadding(context.dp(12),context.dp(12),context.dp(12),context.dp(12)); scaleType=android.widget.ImageView.ScaleType.FIT_CENTER
    setOnClickListener { action() }
}
fun musicChip(context: Context, title: String, selected: Boolean = false, action: () -> Unit) = TextView(context).apply {
    text=title; textSize=14f; gravity=Gravity.CENTER; minHeight=context.dp(44)
    setTextColor(context.getColor(if(selected) R.color.ichoi_on_accent else R.color.ichoi_text))
    background=context.touchSurface(if(selected) R.color.ichoi_accent else R.color.ichoi_surface,24)
    setPadding(context.dp(18),context.dp(8),context.dp(18),context.dp(8)); isFocusable=true
    layoutParams=LinearLayout.LayoutParams(-2,-2).apply { marginEnd=context.dp(8) }
    setOnClickListener { action() }
}
