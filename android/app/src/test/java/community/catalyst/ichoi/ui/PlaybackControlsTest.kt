package community.catalyst.ichoi.ui

import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import community.catalyst.ichoi.playback.PlaybackService
import community.catalyst.ichoi.playback.PlaybackStatus
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.android.controller.ActivityController
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowAlertDialog
import java.time.Duration

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29])
class PlaybackControlsTest {
    private lateinit var controller: ActivityController<MainActivity>
    private lateinit var activity: MainActivity

    @Before fun setUp() {
        controller = Robolectric.buildActivity(MainActivity::class.java).setup()
        activity = controller.get()
    }

    @After fun tearDown() {
        controller.pause().stop().destroy()
        PlaybackStatus.update(PlaybackStatus.State())
        AppMessages.current?.let { AppMessages.dismiss(it) }
    }

    @Test fun controlsRemainAvailableAcrossScreensAndSendServiceCommands() {
        PlaybackStatus.update(PlaybackStatus.State(PlaybackStatus.Phase.PLAYING, "A track", "https://demo.example", 1000, 60_000))
        val controls = views().filterIsInstance<PlaybackControls>().single()
        assertEquals(View.VISIBLE, controls.visibility)
        button("Pause").performClick()
        assertEquals(PlaybackService.ACTION_PAUSE, shadowOf(activity).nextStartedService.action)
        button("App settings").performClick()
        assertSame(controls, views().filterIsInstance<PlaybackControls>().single())
        PlaybackStatus.update(PlaybackStatus.current.copy(phase = PlaybackStatus.Phase.PAUSED))
        button("Resume").performClick()
        assertEquals(PlaybackService.ACTION_RESUME, shadowOf(activity).nextStartedService.action)
        button("Stop").performClick()
        assertEquals(PlaybackService.ACTION_STOP, shadowOf(activity).nextStartedService.action)
        PlaybackStatus.update(PlaybackStatus.State())
        assertEquals(View.GONE, controls.visibility)
    }

    @Test fun screenNavigationDoesNotRestartTheMessageTimer() {
        AppMessages.show("Playback failed")
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(9))
        button("App settings").performClick()
        assertNotNull(AppMessages.current)
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(1))
        assertNull(AppMessages.current)
    }

    @Test fun durationCanBeChangedFromSettingsWithoutAServer() {
        button("App settings").performClick()
        button("Message duration: 10 seconds").performClick()
        shadowOf(Looper.getMainLooper()).idle()
        val dialog = ShadowAlertDialog.getLatestAlertDialog()
        views(dialog.window!!.decorView).filterIsInstance<EditText>().single().setText("30")
        dialog.getButton(android.app.AlertDialog.BUTTON_POSITIVE).performClick()
        assertEquals(30, MessagePreferences(activity).durationSeconds)
        assertNotNull(button("Message duration: 30 seconds"))
    }

    private fun button(text: String) = views().filterIsInstance<Button>().first { it.text.toString() == text }
    private fun views(root: View = activity.findViewById(android.R.id.content)): Sequence<View> = sequence {
        yield(root)
        if (root is ViewGroup) for (index in 0 until root.childCount) yieldAll(views(root.getChildAt(index)))
    }
}
