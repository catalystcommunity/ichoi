package community.catalyst.ichoi.playback

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.graphics.drawable.Icon
import android.media.AudioAttributes
import android.media.MediaMetadata
import android.media.MediaPlayer
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import community.catalyst.ichoi.ui.AppMessages
import community.catalyst.ichoi.R
import community.catalyst.ichoi.ui.MainActivity
import community.catalyst.ichoi.storage.ServerProfile

/** Streams one track through MediaPlayer. No media bytes are persisted. */
class PlaybackService : Service() {
    private var player: MediaPlayer? = null
    private var mediaSource: ServerMediaDataSource? = null
    private lateinit var mediaSession: MediaSession
    private val handler = Handler(Looper.getMainLooper())
    private val progress = object : Runnable {
        override fun run() {
            if (PlaybackStatus.current.phase != PlaybackStatus.Phase.PLAYING) return
            control {
                publishPosition()
                handler.postDelayed(this, 1000)
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "Playback", NotificationManager.IMPORTANCE_LOW))
        mediaSession = MediaSession(this, "Ichoi").apply {
            setCallback(object : MediaSession.Callback() {
                override fun onPlay() { resumePlayback() }
                override fun onPause() { pausePlayback() }
                override fun onStop() { finishPlayback() }
                override fun onSeekTo(pos: Long) { seekTo(pos) }
                override fun onCustomAction(action: String, extras: android.os.Bundle?) {
                    if (action == ACTION_STOP) finishPlayback()
                }
            })
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> finishPlayback()
            ACTION_PAUSE -> pausePlayback()
            ACTION_RESUME -> resumePlayback()
            ACTION_SEEK -> seekTo(intent.getIntExtra(EXTRA_POSITION, 0).toLong())
            else -> {
                val url = intent?.getStringExtra(EXTRA_URL)
                val profileId = intent?.getStringExtra(EXTRA_PROFILE_ID)
                val serverUrl = intent?.getStringExtra(EXTRA_SERVER_URL)
                if (url == null || profileId == null || serverUrl == null) {
                    finishPlayback()
                    return START_NOT_STICKY
                }
                releasePlayer()
                PlaybackStatus.update(PlaybackStatus.State(
                    phase = PlaybackStatus.Phase.LOADING,
                    title = intent.getStringExtra(EXTRA_TITLE) ?: "Ichoi",
                    server = serverUrl,
                ))
                mediaSession.isActive = true
                publishSession()
                startForeground(NOTIFICATION_ID, notification())
                control {
                    val current = MediaPlayer()
                    player = current
                    current.setAudioAttributes(AudioAttributes.Builder()
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).setUsage(AudioAttributes.USAGE_MEDIA).build())
                    current.setOnPreparedListener { prepared ->
                        if (player === prepared) control {
                            prepared.start()
                            PlaybackStatus.update(PlaybackStatus.current.copy(
                                phase = PlaybackStatus.Phase.PLAYING, durationMs = prepared.duration.coerceAtLeast(0)))
                            publishSession()
                            updateNotification()
                            handler.post(progress)
                        }
                    }
                    current.setOnCompletionListener { completed -> if (player === completed) { finishPlayback(); PlaybackQueue.completed(this) } }
                    current.setOnErrorListener { failed, _, _ ->
                        if (player === failed) failPlayback()
                        true
                    }
                    current.setOnSeekCompleteListener { sought -> if (player === sought) control { publishPosition() } }
                    val source = ServerMediaDataSource(
                        ServerProfile(profileId, serverUrl, intent.getBooleanExtra(EXTRA_PRIVATE_HTTP, false)),
                        url, intent.getStringExtra(EXTRA_TOKEN), intent.getLongExtra(EXTRA_DURATION, 0))
                    mediaSource = source
                    current.setDataSource(source)
                    current.prepareAsync()
                }
            }
        }
        // Do not restart audio after Android terminates the service.
        if (player == null) stopSelf()
        return START_NOT_STICKY
    }

    private fun pausePlayback() {
        if (PlaybackStatus.current.phase != PlaybackStatus.Phase.PLAYING) return
        control {
            player?.pause()
            handler.removeCallbacks(progress)
            PlaybackStatus.update(PlaybackStatus.current.copy(phase = PlaybackStatus.Phase.PAUSED))
            publishPosition()
            updateNotification()
        }
    }

    private fun resumePlayback() {
        if (PlaybackStatus.current.phase != PlaybackStatus.Phase.PAUSED) return
        control {
            player?.start()
            PlaybackStatus.update(PlaybackStatus.current.copy(phase = PlaybackStatus.Phase.PLAYING))
            publishSession()
            updateNotification()
            handler.removeCallbacks(progress)
            handler.post(progress)
        }
    }

    private fun seekTo(position: Long) {
        val state = PlaybackStatus.current
        if (state.phase != PlaybackStatus.Phase.PLAYING && state.phase != PlaybackStatus.Phase.PAUSED) return
        control { player?.seekTo(position.coerceIn(0L, state.durationMs.toLong()), MediaPlayer.SEEK_CLOSEST) }
    }

    private fun publishPosition() {
        PlaybackStatus.update(PlaybackStatus.current.copy(positionMs = player?.currentPosition?.coerceAtLeast(0) ?: 0))
        publishSession()
    }

    private fun publishSession() {
        val current = PlaybackStatus.current
        val ready = current.phase == PlaybackStatus.Phase.PLAYING || current.phase == PlaybackStatus.Phase.PAUSED
        val state = when (current.phase) {
            PlaybackStatus.Phase.LOADING -> PlaybackState.STATE_BUFFERING
            PlaybackStatus.Phase.PLAYING -> PlaybackState.STATE_PLAYING
            PlaybackStatus.Phase.PAUSED -> PlaybackState.STATE_PAUSED
            PlaybackStatus.Phase.STOPPED -> PlaybackState.STATE_STOPPED
        }
        val actions = PlaybackState.ACTION_STOP or if (ready)
            (PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_SEEK_TO) else 0L
        mediaSession.setPlaybackState(PlaybackState.Builder()
            .setActions(actions)
            .setState(state, current.positionMs.toLong(), if (current.phase == PlaybackStatus.Phase.PLAYING) 1f else 0f)
            .addCustomAction(ACTION_STOP, "Stop", R.drawable.ic_stop)
            .build())
        mediaSession.setMetadata(MediaMetadata.Builder()
            .putString(MediaMetadata.METADATA_KEY_TITLE, current.title)
            .putString(MediaMetadata.METADATA_KEY_ARTIST, current.server)
            .putLong(MediaMetadata.METADATA_KEY_DURATION, current.durationMs.toLong()).build())
    }

    private fun notification(): Notification {
        val current = PlaybackStatus.current
        val builder = Notification.Builder(this, CHANNEL)
            .setContentTitle(current.title)
            .setContentText(if (current.phase == PlaybackStatus.Phase.LOADING) "Loading…" else current.server)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setOnlyAlertOnce(true)
            .setOngoing(current.phase != PlaybackStatus.Phase.PAUSED)
            .setContentIntent(PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE))
            .setDeleteIntent(commandIntent(ACTION_STOP))
        if (current.phase != PlaybackStatus.Phase.LOADING) {
            val paused = current.phase == PlaybackStatus.Phase.PAUSED
            builder.addAction(Notification.Action.Builder(
                Icon.createWithResource(this, if (paused) android.R.drawable.ic_media_play else android.R.drawable.ic_media_pause),
                if (paused) "Resume" else "Pause",
                commandIntent(if (paused) ACTION_RESUME else ACTION_PAUSE)).build())
        }
        builder.addAction(Notification.Action.Builder(Icon.createWithResource(this, R.drawable.ic_stop), "Stop", commandIntent(ACTION_STOP)).build())
        val style = Notification.MediaStyle().setMediaSession(mediaSession.sessionToken)
        if (current.phase == PlaybackStatus.Phase.LOADING) style.setShowActionsInCompactView(0)
        else style.setShowActionsInCompactView(0, 1)
        return builder.setStyle(style).build()
    }

    private fun commandIntent(action: String): PendingIntent = PendingIntent.getService(
        this, 0, Intent(this, PlaybackService::class.java).setAction(action), PendingIntent.FLAG_IMMUTABLE)

    // This is always a MediaStyle notification with a media-session token.
    // Media-session notifications are exempt from POST_NOTIFICATIONS:
    // https://developer.android.com/develop/ui/views/notifications/notification-permission#media-sessions
    @SuppressLint("NotificationPermission")
    private fun updateNotification() {
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification())
    }

    private inline fun control(action: () -> Unit) {
        try { action() } catch (_: Exception) { failPlayback() }
    }

    private fun failPlayback() {
        val title = PlaybackStatus.current.title
        finishPlayback()
        AppMessages.show("Could not play $title. Check the connection to the server and try again.")
    }

    private fun releasePlayer() {
        handler.removeCallbacks(progress)
        val old = player
        player = null
        // Cancel a pending read before native release can wait for that read.
        mediaSource?.close()
        mediaSource = null
        old?.release()
    }

    private fun finishPlayback() {
        releasePlayer()
        PlaybackStatus.update(PlaybackStatus.State())
        publishSession()
        mediaSession.isActive = false
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        releasePlayer()
        PlaybackStatus.update(PlaybackStatus.State())
        mediaSession.release()
        super.onDestroy()
    }
    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val ACTION_STOP = "community.catalyst.ichoi.STOP_PLAYBACK"
        const val ACTION_PAUSE = "community.catalyst.ichoi.PAUSE_PLAYBACK"
        const val ACTION_RESUME = "community.catalyst.ichoi.RESUME_PLAYBACK"
        const val ACTION_SEEK = "community.catalyst.ichoi.SEEK_PLAYBACK"
        const val EXTRA_DURATION = "duration_ms"
        const val EXTRA_POSITION = "position_ms"
        const val EXTRA_URL = "stream_url"
        const val EXTRA_TITLE = "track_title"
        const val EXTRA_TOKEN = "session_token"
        const val EXTRA_PROFILE_ID = "profile_id"
        const val EXTRA_SERVER_URL = "server_url"
        const val EXTRA_PRIVATE_HTTP = "private_http"
        private const val CHANNEL = "ichoi_playback"
        private const val NOTIFICATION_ID = 7
    }
}
