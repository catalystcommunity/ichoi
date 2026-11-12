package community.catalyst.ichoi.ui

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.InsetDrawable
import android.graphics.drawable.RippleDrawable
import android.widget.Button
import android.widget.LinearLayout
import community.catalyst.ichoi.R

/** Visible boundaries and spacing for each native touch target. */
fun appButton(context: Context, label: String, primary: Boolean = false, action: () -> Unit): Button = Button(context).apply {
    fun dp(value: Int) = (value * resources.displayMetrics.density).toInt()
    val accent = context.getColor(R.color.ichoi_accent)
    val textColor = context.getColor(if (primary) R.color.ichoi_on_accent else R.color.ichoi_text)
    val shape = GradientDrawable().apply {
        cornerRadius = dp(10).toFloat()
        setColor(context.getColor(if (primary) R.color.ichoi_accent else R.color.ichoi_surface))
        setStroke(dp(1), context.getColor(if (primary) R.color.ichoi_accent else R.color.ichoi_border))
    }
    text = label; isAllCaps = false; textSize = 14f
    minWidth = dp(48); minimumWidth = dp(48); minHeight = dp(52); minimumHeight = dp(52)
    setTextColor(ColorStateList(arrayOf(intArrayOf(-android.R.attr.state_enabled), intArrayOf()), intArrayOf(context.getColor(R.color.ichoi_muted), textColor)))
    background = InsetDrawable(RippleDrawable(ColorStateList.valueOf(Color.argb(45, Color.red(accent), Color.green(accent), Color.blue(accent))), shape, null), dp(4), dp(2), dp(4), dp(2))
    setPadding(dp(14), dp(10), dp(14), dp(10))
    stateListAnimator = null
    layoutParams = LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(4); bottomMargin = dp(4) }
    setOnClickListener { action() }
}
