package community.catalyst.ichoi.ui

import android.app.Dialog
import android.content.Context
import android.content.Intent
import android.view.Gravity
import android.widget.*
import community.catalyst.ichoi.R
import community.catalyst.ichoi.playback.*
import community.catalyst.csilgen.generated.PlayerStatus

/** A compact player stays above navigation. Detailed controls live in Now playing. */
class PlaybackControls(context: Context) : LinearLayout(context) {
    var openQueue: (() -> Unit)? = null
    var chooseTarget: (() -> Unit)? = null
    var artwork: ((String?, (android.graphics.Bitmap?) -> Unit) -> Unit)? = null
    private val cover = MusicCover(context,44)
    private val title = musicText(context,"Choose something to play",14f).apply { setTypeface(typeface,android.graphics.Typeface.BOLD) }
    private val subtitle = musicText(context,"This phone",12f,true)
    private val toggle = iconButton(context,MusicIcon.PLAY,"Play",true) { togglePlayback() }
    private val progress = createSeekBar()
    private val miniPosition = musicText(context, "0:00", 11f, true)
    private val miniDuration = musicText(context, "0:00", 11f, true)
    private val main = android.os.Handler(android.os.Looper.getMainLooper())
    private var previewPosition: Int? = null
    private var playbackIdentity = ""
    private var gestureIdentity: String? = null
    private val clearPreview = Runnable { previewPosition = null; render() }
    private var artKey: String? = null
    private var sheet: Dialog? = null
    private var fullTitle: TextView? = null
    private var fullArtist: TextView? = null
    private var fullCover: MusicCover? = null
    private var fullPlay: ImageButton? = null
    private var device: TextView? = null
    private var positionLabel: TextView? = null
    private var durationLabel: TextView? = null
    private var seek: SeekBar? = null
    private var seeking=false
    private val listener: (PlaybackStatus.State) -> Unit = { render() }
    private val remoteListener: () -> Unit = { render() }
    init {
        orientation=VERTICAL; setPadding(context.dp(12),context.dp(4),context.dp(12),0)
        val row=LinearLayout(context).apply {
            gravity=Gravity.CENTER_VERTICAL; background=context.surface(R.color.ichoi_surface,12)
            setPadding(context.dp(8),context.dp(6),context.dp(4),context.dp(6))
        }
        val identity=LinearLayout(context).apply {
            gravity=Gravity.CENTER_VERTICAL; isFocusable=true; contentDescription="Open Now playing"; background=context.touchSurface()
            addView(cover)
            addView(LinearLayout(context).apply { orientation=VERTICAL; setPadding(context.dp(10),0,context.dp(8),0); addView(title); addView(subtitle) },LayoutParams(0,-2,1f))
            setOnClickListener { showNowPlaying() }
        }
        row.addView(identity,LayoutParams(0,-2,1f)); row.addView(iconButton(context,MusicIcon.DEVICE,"Change playback device") { chooseTarget?.invoke() },LayoutParams(context.dp(44),context.dp(48)))
        row.addView(toggle,LayoutParams(context.dp(48),context.dp(48)))
        row.addView(iconButton(context,MusicIcon.NEXT,"Next") { next() },LayoutParams(context.dp(44),context.dp(48)))
        addView(row)
        addView(LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(context.dp(8), 0, context.dp(8), 0)
            addView(miniPosition)
            addView(progress, LayoutParams(0, context.dp(48), 1f))
            addView(miniDuration)
        })
        render()
    }
    private fun togglePlayback() {
        if(RemotePlayback.target!=null) RemotePlayback.toggle()
        else if(PlaybackStatus.current.phase==PlaybackStatus.Phase.STOPPED) PlaybackQueue.play(context,0)
        else command(if(PlaybackStatus.current.phase==PlaybackStatus.Phase.PLAYING) PlaybackService.ACTION_PAUSE else PlaybackService.ACTION_RESUME)
    }
    private fun next() { if(RemotePlayback.target!=null) RemotePlayback.next() else PlaybackQueue.completed(context) }
    fun showNowPlaying() {
        if(sheet?.isShowing==true) return
        val dialog=Dialog(context); sheet=dialog
        val body=LinearLayout(context).apply { orientation=VERTICAL;setPadding(context.dp(24),context.dp(12),context.dp(24),context.dp(24));setBackgroundColor(context.getColor(R.color.ichoi_bg)) }
        body.addView(LinearLayout(context).apply {
            gravity=Gravity.CENTER_VERTICAL
            addView(iconButton(context,MusicIcon.DOWN,"Close Now playing") { dialog.dismiss() })
            addView(musicText(context,"NOW PLAYING",12f,true).apply { gravity=Gravity.CENTER },LayoutParams(0,-2,1f))
            addView(iconButton(context,MusicIcon.QUEUE,"Open queue") { dialog.dismiss();openQueue?.invoke() })
        })
        fullCover=MusicCover(context,(resources.configuration.screenWidthDp-48).coerceAtMost(380)).also { body.addView(it,LayoutParams(-1,context.dp((resources.configuration.screenWidthDp-48).coerceAtMost(380))).apply { topMargin=context.dp(20);bottomMargin=context.dp(28) });it.setImageDrawable(cover.drawable.constantState?.newDrawable() ?: MusicGlyph(MusicIcon.LIBRARY,context.getColor(R.color.ichoi_muted)));it.setPadding(cover.paddingLeft,cover.paddingTop,cover.paddingRight,cover.paddingBottom) }
        fullTitle=musicText(context,"",26f).apply { maxLines=2;setTypeface(typeface,android.graphics.Typeface.BOLD) }.also { body.addView(it) }
        fullArtist=musicText(context,"",16f,true).also { body.addView(it,LayoutParams(-1,-2).apply { topMargin=context.dp(6);bottomMargin=context.dp(20) }) }
        seek = createSeekBar().also { body.addView(it, LayoutParams(-1, context.dp(48))) }
        body.addView(LinearLayout(context).apply {
            positionLabel=musicText(context,"",12f,true).also { addView(it,LayoutParams(0,-2,1f)) }
            durationLabel=musicText(context,"",12f,true).also { addView(it) }
        })
        body.addView(LinearLayout(context).apply {
            gravity=Gravity.CENTER;setPadding(0,context.dp(16),0,context.dp(16))
            addView(iconButton(context,MusicIcon.PREVIOUS,"Previous") { if(RemotePlayback.target!=null) RemotePlayback.previous() else { val index=PlaybackQueue.entries.indexOfFirst { it.id==PlaybackQueue.currentId };if(index>0) PlaybackQueue.play(context,index-1) } },LayoutParams(context.dp(64),context.dp(64)))
            fullPlay=iconButton(context,MusicIcon.PLAY,"Play",true) { togglePlayback() }.also { addView(it,LayoutParams(context.dp(76),context.dp(76)).apply { marginStart=context.dp(20);marginEnd=context.dp(20) }) }
            addView(iconButton(context,MusicIcon.NEXT,"Next") { next() },LayoutParams(context.dp(64),context.dp(64)))
        })
        body.addView(LinearLayout(context).apply {
            gravity=Gravity.CENTER_VERTICAL
            device=musicText(context,"",14f).apply { setTextColor(context.getColor(R.color.ichoi_accent));background=context.touchSurface();minHeight=context.dp(48);gravity=Gravity.CENTER_VERTICAL;setOnClickListener { chooseTarget?.invoke() } }.also { addView(it,LayoutParams(0,-2,1f)) }
            addView(iconButton(context,MusicIcon.STOP,"Stop") { if(RemotePlayback.target!=null) RemotePlayback.command(community.catalyst.csilgen.generated.PlayerCommandVariant9(community.catalyst.csilgen.generated.CmdPause("pause"))) else command(PlaybackService.ACTION_STOP) })
        })
        dialog.setContentView(ScrollView(context).apply { isFillViewport=true;addView(body) }); dialog.window?.setBackgroundDrawableResource(R.color.ichoi_bg)
        dialog.setOnDismissListener { sheet=null;fullTitle=null;fullArtist=null;fullCover=null;fullPlay=null;device=null;positionLabel=null;durationLabel=null;seek=null;seeking=false }
        dialog.show();dialog.window?.setLayout(-1,-1);render()
    }
    private fun render() {
        val remote=RemotePlayback.target;val state=PlaybackStatus.current;val remoteState=RemotePlayback.state
        val remoteItem=remoteState?.queue?.getOrNull(remoteState.currentIndex?.toInt()?:-1)
        val entry=PlaybackQueue.entries.firstOrNull { it.id==PlaybackQueue.currentId }?:PlaybackQueue.entries.firstOrNull()
        val playing=if(remote!=null) remoteState?.status==PlayerStatus.Playing else state.phase==PlaybackStatus.Phase.PLAYING
        val position=if(remote!=null) (remoteState?.positionMs?:0uL).coerceAtMost(Int.MAX_VALUE.toULong()).toInt() else state.positionMs
        val duration=if(remote!=null) (remoteItem?.durationMs?:0uL).coerceAtMost(Int.MAX_VALUE.toULong()).toInt() else state.durationMs
        val name=if(remote!=null) remoteItem?.title?:"Ready on ${remote.name}" else state.title.ifBlank { entry?.track?.title?:"Choose something to play" }
        val artist=if(remote!=null) remoteItem?.artist else entry?.track?.artistName
        title.text=name;subtitle.text=listOfNotNull(artist,remote?.name?:"This phone").joinToString(" · ")
        fullTitle?.text=name;fullArtist?.text=artist?:"Your Ichoi library"
        for(control in listOfNotNull(toggle,fullPlay)) { control.contentDescription=if(playing) "Pause" else "Play";control.setImageDrawable(MusicGlyph(if(playing) MusicIcon.PAUSE else MusicIcon.PLAY,context.getColor(R.color.ichoi_on_accent))) }
        device?.text="▣  ${remote?.name?:"This phone"}  ›";device?.contentDescription="Change playback device: ${remote?.name?:"This phone"}"
        val identity = if (remote != null) "${remote.id}:${remoteItem?.queueItemId}" else "${PlaybackQueue.currentId}:${state.server}:${state.title}:${state.phase == PlaybackStatus.Phase.STOPPED}"
        if (identity != playbackIdentity) {
            playbackIdentity = identity
            previewPosition = null
            main.removeCallbacks(clearPreview)
        }
        if (!seeking && (previewPosition?.let { kotlin.math.abs(it.toLong() - position) < 1000 } == true || (remote != null && RemotePlayback.error != null))) {
            previewPosition = null
            main.removeCallbacks(clearPreview)
        }
        val displayed = (previewPosition ?: position).coerceIn(0, duration.coerceAtLeast(0))
        positionLabel?.text = formatTime(displayed)
        durationLabel?.text = formatTime(duration)
        miniPosition.text = formatTime(displayed)
        miniDuration.text = formatTime(duration)
        val ready = duration > 0 && (remote != null || state.phase == PlaybackStatus.Phase.PLAYING || state.phase == PlaybackStatus.Phase.PAUSED)
        for (bar in listOfNotNull(progress, seek)) {
            bar.max = duration.coerceAtLeast(0)
            bar.progress = displayed
            bar.isEnabled = ready
            bar.contentDescription = "Playback position, ${formatTime(displayed)} of ${formatTime(duration)}"
        }
        val key=if(remote==null) entry?.track?.albumId else null
        if(key!=artKey) { artKey=key;cover.placeholder();artwork?.invoke(key) { bitmap -> if(artKey==key) { cover.show(bitmap);fullCover?.show(bitmap) } } }
    }
    private fun createSeekBar() = SeekBar(context).apply {
        contentDescription = "Playback position"
        progressTintList = android.content.res.ColorStateList.valueOf(context.getColor(R.color.ichoi_accent))
        thumbTintList = progressTintList
        keyProgressIncrement = 5_000
        setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onStartTrackingTouch(bar: SeekBar) {
                seeking = true
                gestureIdentity = playbackIdentity
                main.removeCallbacks(clearPreview)
            }
            override fun onProgressChanged(bar: SeekBar, value: Int, fromUser: Boolean) {
                if (!fromUser) return
                if (seeking && gestureIdentity != playbackIdentity) return
                previewPosition = value
                if (!seeking) commitSeek(value) else render()
            }
            override fun onStopTrackingTouch(bar: SeekBar) {
                seeking = false
                if (gestureIdentity == playbackIdentity) commitSeek(bar.progress)
                else { previewPosition = null; render() }
                gestureIdentity = null
            }
        })
        // A cancelled drag must not seek (for example, when the system takes the gesture).
        setOnTouchListener { _, event ->
            if (event.actionMasked == android.view.MotionEvent.ACTION_CANCEL) {
                gestureIdentity = null
                previewPosition = null
            }
            false
        }
    }
    private fun commitSeek(value: Int) {
        previewPosition = value
        main.removeCallbacks(clearPreview)
        main.postDelayed(clearPreview, 3_000)
        if (RemotePlayback.target != null) RemotePlayback.seek(value)
        else command(PlaybackService.ACTION_SEEK, value)
        render()
    }
    fun refresh()=render()
    fun start() { PlaybackStatus.observe(listener);RemotePlayback.observe(remoteListener) }
    fun stop() { PlaybackStatus.removeObserver(listener);RemotePlayback.removeObserver(remoteListener);sheet?.dismiss();main.removeCallbacks(clearPreview);previewPosition=null }
    private fun command(action:String,position:Int=0) { try { context.startService(Intent(context,PlaybackService::class.java).setAction(action).putExtra(PlaybackService.EXTRA_POSITION,position)) } catch(_:Exception) { AppMessages.show("The playback control could not be applied. Try again.") } }
    private fun formatTime(ms:Int)="${ms/60000}:${(ms/1000%60).toString().padStart(2,'0')}"
}
