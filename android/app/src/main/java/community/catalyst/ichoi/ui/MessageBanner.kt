package community.catalyst.ichoi.ui

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.View
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView

/** An in-app message with a configurable timeout and an explicit dismiss button. */
class MessageBanner(context: Context) : LinearLayout(context) {
    private val handler = Handler(Looper.getMainLooper())
    private val label = TextView(context).apply {
        textSize = 16f
        accessibilityLiveRegion = View.ACCESSIBILITY_LIVE_REGION_POLITE
    }
    private var expiry: Runnable? = null
    private var active = false
    private val listener: (AppMessages.Message?) -> Unit = { message ->
        expiry?.let { handler.removeCallbacks(it) }
        expiry = null
        visibility = if (message == null) GONE else VISIBLE
        label.text = message?.text.orEmpty()
        if (message != null) {
            expiry = Runnable { AppMessages.dismiss(message) }.also {
                handler.postDelayed(it, MessagePreferences(context).durationSeconds * 1000L)
            }
        }
    }

    init {
        orientation = HORIZONTAL
        val padding = (12 * resources.displayMetrics.density).toInt()
        setPadding(padding, padding, padding, padding)
        addView(label, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
        addView(Button(context).apply {
            text = "Dismiss"
            setOnClickListener { AppMessages.current?.let { AppMessages.dismiss(it) } }
        })
        visibility = GONE
    }

    fun start() {
        if (active) return
        active = true
        AppMessages.observe(listener)
    }

    fun stop() {
        active = false
        AppMessages.removeObserver(listener)
        expiry?.let { handler.removeCallbacks(it) }
        expiry = null
    }
}
