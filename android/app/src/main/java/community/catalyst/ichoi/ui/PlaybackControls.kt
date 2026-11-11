package community.catalyst.ichoi.ui

import android.content.Context
import android.content.Intent
import community.catalyst.ichoi.R
import android.widget.Button
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import community.catalyst.ichoi.playback.PlaybackService
import community.catalyst.ichoi.playback.PlaybackStatus

class PlaybackControls(context: Context) : LinearLayout(context) {
    var openQueue: (() -> Unit)? = null
    private val title = TextView(context)
    private val time = TextView(context)
    private var seeking = false
    private val seek = SeekBar(context).apply { contentDescription = "Playback position" }
    private val toggle = Button(context).apply {
        setOnClickListener {
            command(if (PlaybackStatus.current.phase == PlaybackStatus.Phase.PLAYING) PlaybackService.ACTION_PAUSE else PlaybackService.ACTION_RESUME)
        }
    }
    private val listener: (PlaybackStatus.State) -> Unit = { state ->
        visibility = if (state.phase == PlaybackStatus.Phase.STOPPED) GONE else VISIBLE
        time.setTextColor(context.getColor(if (state.phase == PlaybackStatus.Phase.PLAYING) R.color.ichoi_signal else R.color.ichoi_muted))
        title.text = "${state.title}\n${state.server}"
        time.text = if (state.phase == PlaybackStatus.Phase.LOADING) "Loading…" else "${formatTime(state.positionMs)} / ${formatTime(state.durationMs)}"
        toggle.text = if (state.phase == PlaybackStatus.Phase.PAUSED) "Resume" else "Pause"
        toggle.isEnabled = state.phase == PlaybackStatus.Phase.PLAYING || state.phase == PlaybackStatus.Phase.PAUSED
        seek.isEnabled = toggle.isEnabled && state.durationMs > 0
        if (!seeking) { seek.max = state.durationMs; seek.progress = state.positionMs }
    }

    init {
        orientation = VERTICAL
        setBackgroundColor(context.getColor(R.color.ichoi_surface))
        title.setTextColor(context.getColor(R.color.ichoi_text))
        toggle.isAllCaps = false
        val padding = (12 * resources.displayMetrics.density).toInt()
        setPadding(padding, 0, padding, padding)
        addView(title)
        addView(time)
        addView(seek)
        addView(LinearLayout(context).apply {
            addView(Button(context).apply { text = "Queue"; isAllCaps = false; setOnClickListener { openQueue?.invoke() } }, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
            addView(toggle, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
            addView(Button(context).apply {
                text = "Stop"
                setOnClickListener { command(PlaybackService.ACTION_STOP) }
            }, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
        })
        seek.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onStartTrackingTouch(bar: SeekBar) { seeking = true }
            override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {}
            override fun onStopTrackingTouch(bar: SeekBar) {
                seeking = false
                command(PlaybackService.ACTION_SEEK, bar.progress)
            }
        })
        visibility = GONE
    }

    fun start() { PlaybackStatus.observe(listener) }
    fun stop() { PlaybackStatus.removeObserver(listener) }

    private fun command(action: String, position: Int = 0) {
        try {
            context.startService(Intent(context, PlaybackService::class.java).setAction(action)
                .putExtra(PlaybackService.EXTRA_POSITION, position))
        } catch (_: Exception) { AppMessages.show("The playback control could not be applied. Try again.") }
    }

    private fun formatTime(ms: Int): String = "${ms / 60_000}:${(ms / 1000 % 60).toString().padStart(2, '0')}"
}
