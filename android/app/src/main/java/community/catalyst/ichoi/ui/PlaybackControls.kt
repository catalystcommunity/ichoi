package community.catalyst.ichoi.ui

import android.content.Context
import android.content.Intent
import android.view.Gravity
import android.widget.Button
import android.widget.LinearLayout
import android.widget.SeekBar
import android.widget.TextView
import community.catalyst.ichoi.R
import community.catalyst.ichoi.playback.PlaybackService
import community.catalyst.ichoi.playback.PlaybackStatus
import community.catalyst.ichoi.playback.PlaybackQueue
import community.catalyst.ichoi.playback.RemotePlayback
import community.catalyst.csilgen.generated.PlayerStatus

class PlaybackControls(context: Context) : LinearLayout(context) {
    var openQueue: (() -> Unit)? = null
    var chooseTarget: (() -> Unit)? = null
    private val title = TextView(context).apply { textSize = 15f; maxLines = 1; ellipsize = android.text.TextUtils.TruncateAt.END; setTypeface(typeface, android.graphics.Typeface.BOLD) }
    private val time = TextView(context).apply { textSize = 11f }
    private var seeking = false
    private val seek = SeekBar(context).apply { contentDescription = "Playback position" }
    private val target = control("This phone") { chooseTarget?.invoke() }
    private val toggle = control("Play") {
        if (RemotePlayback.target != null) RemotePlayback.toggle()
        else if (PlaybackStatus.current.phase == PlaybackStatus.Phase.STOPPED) PlaybackQueue.play(context, 0)
        else command(if (PlaybackStatus.current.phase == PlaybackStatus.Phase.PLAYING) PlaybackService.ACTION_PAUSE else PlaybackService.ACTION_RESUME)
    }
    private val listener: (PlaybackStatus.State) -> Unit = { render() }
    private val remoteListener: () -> Unit = { render() }
    private fun control(label: String, action: () -> Unit) = Button(context).apply {
        text = label; isAllCaps = false; textSize = 13f; minWidth = 0; minimumWidth = 0
        setPadding(8, 0, 8, 0); setOnClickListener { action() }
    }
    init {
        orientation = VERTICAL
        setBackgroundColor(context.getColor(R.color.ichoi_surface))
        title.setTextColor(context.getColor(R.color.ichoi_text))
        time.setTextColor(context.getColor(R.color.ichoi_muted))
        val padding = (12 * resources.displayMetrics.density).toInt()
        setPadding(padding, padding / 2, padding, padding / 2)
        addView(LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            addView(title, LayoutParams(0, -2, 1f))
            addView(toggle, LayoutParams(-2, dp(44)))
            addView(control("Next") {
                if (RemotePlayback.target != null) RemotePlayback.next()
                else PlaybackQueue.completed(context)
            }, LayoutParams(-2, dp(44)))
        })
        addView(LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            addView(time, LayoutParams(-2, -2))
            addView(seek, LayoutParams(0, dp(30), 1f))
        })
        addView(LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            addView(target, LayoutParams(0, dp(44), 1f))
            addView(control("Queue") { openQueue?.invoke() }, LayoutParams(-2, dp(44)))
            addView(control("Stop") {
                if (RemotePlayback.target != null) RemotePlayback.command(community.catalyst.csilgen.generated.PlayerCommandVariant9(community.catalyst.csilgen.generated.CmdPause("pause")))
                else command(PlaybackService.ACTION_STOP)
            }, LayoutParams(-2, dp(44)))
        })
        seek.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onStartTrackingTouch(bar: SeekBar) { seeking = true }
            override fun onProgressChanged(bar: SeekBar, progress: Int, fromUser: Boolean) {}
            override fun onStopTrackingTouch(bar: SeekBar) {
                seeking = false
                if (RemotePlayback.target != null) RemotePlayback.seek(bar.progress) else command(PlaybackService.ACTION_SEEK, bar.progress)
            }
        })
        render()
    }
    private fun render() {
        val remote = RemotePlayback.target
        val state = PlaybackStatus.current
        val remoteState = RemotePlayback.state
        val item = remoteState?.queue?.getOrNull(remoteState.currentIndex?.toInt() ?: -1)
        val playing = if (remote != null) remoteState?.status == PlayerStatus.Playing else state.phase == PlaybackStatus.Phase.PLAYING
        val position = if (remote != null) (remoteState?.positionMs ?: 0uL).coerceAtMost(Int.MAX_VALUE.toULong()).toInt() else state.positionMs
        val duration = if (remote != null) (item?.durationMs ?: 0uL).coerceAtMost(Int.MAX_VALUE.toULong()).toInt() else state.durationMs
        visibility = VISIBLE
        title.text = if (remote != null) item?.title ?: "Ready on ${remote.name}" else state.title.ifBlank { "Ready when you are" }
        target.text = "Playing on · ${remote?.name ?: "This phone"} ▾"
        target.contentDescription = "Playback device: ${remote?.name ?: "This phone"}. Change device"
        time.text = RemotePlayback.error?.takeIf { remote != null } ?: if (state.phase == PlaybackStatus.Phase.LOADING && remote == null) "Loading…" else "${formatTime(position)} / ${formatTime(duration)}"
        toggle.text = if (playing) "Pause" else "Play"
        toggle.isEnabled = if (remote != null) remoteState != null else state.phase != PlaybackStatus.Phase.LOADING
        seek.isEnabled = duration > 0
        if (!seeking) { seek.max = duration; seek.progress = position }
    }
    fun start() { PlaybackStatus.observe(listener); RemotePlayback.observe(remoteListener) }
    fun stop() { PlaybackStatus.removeObserver(listener); RemotePlayback.removeObserver(remoteListener) }
    private fun command(action: String, position: Int = 0) {
        try { context.startService(Intent(context, PlaybackService::class.java).setAction(action).putExtra(PlaybackService.EXTRA_POSITION, position)) }
        catch (_: Exception) { AppMessages.show("The playback control could not be applied. Try again.") }
    }
    private fun dp(value: Int) = (value * resources.displayMetrics.density).toInt()
    private fun formatTime(ms: Int): String = "${ms / 60_000}:${(ms / 1000 % 60).toString().padStart(2, '0')}"
}
