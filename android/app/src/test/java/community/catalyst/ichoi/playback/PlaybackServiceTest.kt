package community.catalyst.ichoi.playback

import android.app.NotificationManager
import android.content.Intent
import android.media.MediaPlayer
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Looper
import community.catalyst.ichoi.ui.AppMessages
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.android.controller.ServiceController
import org.robolectric.annotation.Config
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements
import org.robolectric.shadow.api.Shadow
import org.robolectric.shadows.ShadowMediaPlayer
import org.robolectric.shadows.ShadowMediaSession
import java.time.Duration

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29], shadows = [PlaybackServiceTest.RecordingMediaSession::class])
class PlaybackServiceTest {
    // Robolectric has no media-session system service. Capture the app's outgoing
    // state and incoming command callback at that boundary.
    @Implements(MediaSession::class)
    class RecordingMediaSession : ShadowMediaSession() {
        var state: PlaybackState? = null
        var recordedCallback: MediaSession.Callback? = null
        @Implementation fun setPlaybackState(value: PlaybackState?) { state = value }
        @Implementation fun setCallback(value: MediaSession.Callback?) { recordedCallback = value }
    }
    private lateinit var controller: ServiceController<PlaybackService>
    private lateinit var service: PlaybackService
    private lateinit var player: MediaPlayer
    private val main get() = shadowOf(Looper.getMainLooper())

    @Before fun setUp() {
        ShadowMediaPlayer.setMediaInfoProvider { ShadowMediaPlayer.MediaInfo(60_000, 100) }
        ShadowMediaPlayer.setCreateListener { created, shadow ->
            player = created
            shadow.setInvalidStateBehavior(ShadowMediaPlayer.InvalidStateBehavior.EMULATE)
        }
        controller = Robolectric.buildService(PlaybackService::class.java).create()
        service = controller.get()
    }

    @After fun tearDown() {
        controller.destroy()
        AppMessages.current?.let { AppMessages.dismiss(it) }
    }

    @Test fun pauseResumeSeekAndStopControlThePlayer() {
        startTrack()
        main.idleFor(Duration.ofMillis(100))
        assertTrue(player.isPlaying)
        assertEquals(PlaybackStatus.Phase.PLAYING, PlaybackStatus.current.phase)
        command(PlaybackService.ACTION_PAUSE)
        assertFalse(player.isPlaying)
        assertEquals(PlaybackStatus.Phase.PAUSED, PlaybackStatus.current.phase)
        service.onStartCommand(Intent().setAction(PlaybackService.ACTION_SEEK)
            .putExtra(PlaybackService.EXTRA_POSITION, 20_000), 0, 2)
        main.idleFor(Duration.ofMillis(100))
        assertEquals(20_000, player.currentPosition)
        assertEquals(20_000, PlaybackStatus.current.positionMs)
        command(PlaybackService.ACTION_RESUME)
        assertTrue(player.isPlaying)
        command(PlaybackService.ACTION_STOP)
        assertStopped()
    }

    @Test fun stopWhileLoadingPreventsLatePreparedCallbackFromStartingAudio() {
        startTrack()
        val prepared = shadowOf(player).onPreparedListener
        val source = PlaybackService::class.java.getDeclaredField("mediaSource").apply { isAccessible = true }.get(service) as ServerMediaDataSource
        assertEquals(PlaybackStatus.Phase.LOADING, PlaybackStatus.current.phase)
        command(PlaybackService.ACTION_STOP)
        assertThrows(java.io.IOException::class.java) { source.readAt(0, ByteArray(1), 0, 1) }
        prepared.onPrepared(player)
        main.idleFor(Duration.ofSeconds(1))
        assertStopped()
        assertNull(AppMessages.current)
    }

    @Test fun callbacksFromReplacedTrackDoNotStopTheNewTrack() {
        startTrack()
        val oldPlayer = player
        val oldPrepared = shadowOf(oldPlayer).onPreparedListener
        val oldCompletion = shadowOf(oldPlayer).onCompletionListener
        startTrack("Second track")
        oldPrepared.onPrepared(oldPlayer)
        oldCompletion.onCompletion(oldPlayer)
        main.idleFor(Duration.ofMillis(100))
        assertTrue(player.isPlaying)
        assertEquals("Second track", PlaybackStatus.current.title)
        assertNull(AppMessages.current)
    }

    @Test fun completionRemovesNotificationAndStopsService() {
        startTrack()
        main.idleFor(Duration.ofMillis(100))
        shadowOf(player).invokeCompletionListener()
        assertStopped()
        assertNull(AppMessages.current)
    }

    @Test fun playbackErrorStopsServiceAndPublishesDismissibleMessage() {
        startTrack()
        main.idleFor(Duration.ofMillis(100))
        shadowOf(player).invokeErrorListener(MediaPlayer.MEDIA_ERROR_UNKNOWN, MediaPlayer.MEDIA_ERROR_IO)
        assertStopped()
        assertTrue(AppMessages.current!!.text.contains("Test track"))
        AppMessages.dismiss(AppMessages.current!!)
        assertNull(AppMessages.current)
    }

    @Test fun sourceSetupFailureIsReported() {
        startTrack(serverUrl = "http://public.example.com")
        assertStopped()
        assertNotNull(AppMessages.current)
    }

    @Test fun notificationsOfferStopWhileLoadingAndPauseResumeWhenReady() {
        startTrack()
        val notifications = shadowOf(service.getSystemService(NotificationManager::class.java))
        val loading = notifications.getNotification(7)
        assertEquals(listOf("Stop"), loading.actions.map { it.title.toString() })
        main.idleFor(Duration.ofMillis(100))
        val playing = notifications.getNotification(7)
        assertEquals(listOf("Pause", "Stop"), playing.actions.map { it.title.toString() })
        service.onStartCommand(shadowOf(playing.actions[0].actionIntent).savedIntent, 0, 2)
        assertFalse(player.isPlaying)
        val paused = notifications.getNotification(7)
        assertEquals("Resume", paused.actions[0].title)
        service.onStartCommand(shadowOf(paused.actions[1].actionIntent).savedIntent, 0, 3)
        assertStopped()
    }

    @Test fun mediaSessionExposesPauseSeekAndStop() {
        startTrack()
        main.idleFor(Duration.ofMillis(100))
        val session = PlaybackService::class.java.getDeclaredField("mediaSession").apply { isAccessible = true }.get(service) as MediaSession
        val recorded = Shadow.extract<RecordingMediaSession>(session)
        assertEquals(PlaybackState.STATE_PLAYING, recorded.state?.state)
        assertTrue(recorded.state!!.actions and PlaybackState.ACTION_SEEK_TO != 0L)
        recorded.recordedCallback!!.onPause()
        main.idle()
        assertEquals(PlaybackStatus.Phase.PAUSED, PlaybackStatus.current.phase)
        recorded.recordedCallback!!.onSeekTo(15_000)
        main.idleFor(Duration.ofMillis(100))
        assertEquals(15_000, player.currentPosition)
        recorded.recordedCallback!!.onStop()
        main.idle()
        assertStopped()
    }

    private fun startTrack(title: String = "Test track", serverUrl: String = "https://server.example") {
        service.onStartCommand(Intent()
            .putExtra(PlaybackService.EXTRA_TITLE, title)
            .putExtra(PlaybackService.EXTRA_URL, "$serverUrl/media/track")
            .putExtra(PlaybackService.EXTRA_PROFILE_ID, "profile")
            .putExtra(PlaybackService.EXTRA_SERVER_URL, serverUrl), 0, 1)
    }

    private fun command(action: String) { service.onStartCommand(Intent().setAction(action), 0, 2) }

    private fun assertStopped() {
        assertEquals(PlaybackStatus.Phase.STOPPED, PlaybackStatus.current.phase)
        assertEquals(ShadowMediaPlayer.State.END, shadowOf(player).state)
        assertTrue(shadowOf(service).isStoppedBySelf)
        assertTrue(shadowOf(service).isForegroundStopped)
        assertNull(shadowOf(service.getSystemService(NotificationManager::class.java)).getNotification(7))
    }
}
