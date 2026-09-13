package community.catalyst.ichoi.playback

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.os.IBinder
import community.catalyst.ichoi.ui.MainActivity
import community.catalyst.ichoi.storage.ServerProfile

/** Streams one track through MediaPlayer. No media bytes are persisted. */
class PlaybackService : Service() {
    private var player: MediaPlayer? = null

    override fun onCreate() {
        super.onCreate()
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, "Playback", NotificationManager.IMPORTANCE_LOW))
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopPlayback()
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return START_NOT_STICKY
        }
        val url = intent?.getStringExtra(EXTRA_URL) ?: return START_NOT_STICKY
        val token = intent.getStringExtra(EXTRA_TOKEN)
        val profile = ServerProfile(
            id = intent.getStringExtra(EXTRA_PROFILE_ID) ?: return START_NOT_STICKY,
            url = intent.getStringExtra(EXTRA_SERVER_URL) ?: return START_NOT_STICKY,
            pinnedSha256 = intent.getStringExtra(EXTRA_PIN).orEmpty(),
            allowPrivateHttp = intent.getBooleanExtra(EXTRA_PRIVATE_HTTP, false),
        )
        startForeground(NOTIFICATION_ID, notification(intent.getStringExtra(EXTRA_TITLE) ?: "Ichoi"))
        stopPlayback()
        player = MediaPlayer().apply {
            setAudioAttributes(AudioAttributes.Builder().setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).setUsage(AudioAttributes.USAGE_MEDIA).build())
            setDataSource(PinnedMediaDataSource(profile, url, token))
            setOnCompletionListener { stopPlayback() }
            setOnErrorListener { _, _, _ -> stopPlayback(); true }
            prepareAsync()
            setOnPreparedListener { it.start() }
        }
        return START_STICKY
    }

    private fun notification(title: String): Notification = Notification.Builder(this, CHANNEL)
        .setContentTitle(title).setContentText("Streaming from your Ichoi server")
        .setSmallIcon(android.R.drawable.ic_media_play).setOngoing(true)
        .setContentIntent(PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE))
        .build()

    private fun stopPlayback() { player?.run { reset(); release() }; player = null }
    override fun onDestroy() { stopPlayback(); super.onDestroy() }
    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val ACTION_STOP = "community.catalyst.ichoi.STOP_PLAYBACK"
        const val EXTRA_URL = "stream_url"
        const val EXTRA_TITLE = "track_title"
        const val EXTRA_TOKEN = "session_token"
        const val EXTRA_PROFILE_ID = "profile_id"
        const val EXTRA_SERVER_URL = "server_url"
        const val EXTRA_PIN = "server_pin"
        const val EXTRA_PRIVATE_HTTP = "private_http"
        private const val CHANNEL = "ichoi_playback"
        private const val NOTIFICATION_ID = 7
    }
}
