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
import org.robolectric.shadows.ShadowDialog
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
        button("Play").performClick()
        assertEquals(PlaybackService.ACTION_RESUME, shadowOf(activity).nextStartedService.action)
        controls.showNowPlaying()
        val expanded = ShadowDialog.getLatestDialog()
        views(expanded.window!!.decorView).first { it.contentDescription == "Stop" }.performClick()
        assertEquals(PlaybackService.ACTION_STOP, shadowOf(activity).nextStartedService.action)
        PlaybackStatus.update(PlaybackStatus.State())
        assertEquals(View.VISIBLE, controls.visibility)
        assertNotNull(button("Change playback device"))
        assertNotNull(views(expanded.window!!.decorView).first { it.contentDescription == "Open queue" })
        expanded.dismiss()
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

    @org.robolectric.annotation.GraphicsMode(org.robolectric.annotation.GraphicsMode.Mode.NATIVE)
    @Test fun miniPlayerKeepsSeekingVisibleInACompactLayout() {
        val controls = views().filterIsInstance<PlaybackControls>().single()
        val density = activity.resources.displayMetrics.density
        controls.measure(View.MeasureSpec.makeMeasureSpec((390*density).toInt(),View.MeasureSpec.EXACTLY),View.MeasureSpec.makeMeasureSpec(0,View.MeasureSpec.UNSPECIFIED))
        assertTrue("Persistent player must stay compact: ${controls.measuredHeight/density} dp; " + views(controls).joinToString { "${it.javaClass.simpleName}:${it.measuredHeight/density}" }, controls.measuredHeight <= 116*density)
        assertEquals(1, views(controls).filterIsInstance<android.widget.SeekBar>().count())
        controls.showNowPlaying()
        val expanded = ShadowDialog.getLatestDialog()
        assertTrue(views(expanded.window!!.decorView).any { it is android.widget.SeekBar })
        expanded.dismiss()
    }

    @Test fun compactProgressSupportsTapAndDragWithoutSendingEveryMovement() {
        PlaybackStatus.update(PlaybackStatus.State(PlaybackStatus.Phase.PLAYING, "Track", "server", 1_000, 60_000))
        val bar = views().filterIsInstance<android.widget.SeekBar>().single()
        bar.layout(0, 0, 300, 48)
        touch(bar, android.view.MotionEvent.ACTION_DOWN, .25f)
        touch(bar, android.view.MotionEvent.ACTION_MOVE, .75f)
        assertEquals(45_000, bar.progress, 200)
        assertNull(shadowOf(activity).nextStartedService)
        PlaybackStatus.update(PlaybackStatus.current.copy(positionMs = 2_000))
        assertEquals(45_000, bar.progress, 200)
        touch(bar, android.view.MotionEvent.ACTION_UP, .75f)
        val command = shadowOf(activity).nextStartedService
        assertEquals(PlaybackService.ACTION_SEEK, command.action)
        assertEquals(45_000, command.getIntExtra(PlaybackService.EXTRA_POSITION, -1), 200)
        assertNull(shadowOf(activity).nextStartedService)
        PlaybackStatus.update(PlaybackStatus.current.copy(positionMs = 3_000))
        assertEquals(45_000, bar.progress, 200)
        PlaybackStatus.update(PlaybackStatus.current.copy(positionMs = 45_000))
        assertEquals(45_000, bar.progress)
        touch(bar, android.view.MotionEvent.ACTION_DOWN, .5f)
        touch(bar, android.view.MotionEvent.ACTION_UP, .5f)
        assertEquals(30_000, shadowOf(activity).nextStartedService.getIntExtra(PlaybackService.EXTRA_POSITION, -1), 200)
    }

    @Test fun cancelledDragAndTrackChangeDoNotSeekTheWrongTrack() {
        PlaybackStatus.update(PlaybackStatus.State(PlaybackStatus.Phase.PAUSED, "First", "server", 1_000, 60_000))
        val bar = views().filterIsInstance<android.widget.SeekBar>().single()
        bar.layout(0, 0, 300, 48)
        touch(bar, android.view.MotionEvent.ACTION_DOWN, .5f)
        touch(bar, android.view.MotionEvent.ACTION_CANCEL, .5f)
        assertNull(shadowOf(activity).nextStartedService)
        assertEquals(1_000, bar.progress)
        touch(bar, android.view.MotionEvent.ACTION_DOWN, .5f)
        PlaybackStatus.update(PlaybackStatus.current.copy(title = "Second", positionMs = 0))
        touch(bar, android.view.MotionEvent.ACTION_UP, .8f)
        assertNull(shadowOf(activity).nextStartedService)
        assertEquals(0, bar.progress)
    }

    @Test fun unconfirmedSeekReturnsToReportedPositionAndExpandedPlayerSharesPreview() {
        PlaybackStatus.update(PlaybackStatus.State(PlaybackStatus.Phase.PAUSED, "Track", "server", 1_000, 60_000))
        val controls = views().filterIsInstance<PlaybackControls>().single()
        controls.showNowPlaying()
        val expanded = ShadowDialog.getLatestDialog()
        val fullBar = views(expanded.window!!.decorView).filterIsInstance<android.widget.SeekBar>().single()
        fullBar.layout(0, 0, 300, 48)
        touch(fullBar, android.view.MotionEvent.ACTION_DOWN, .5f)
        touch(fullBar, android.view.MotionEvent.ACTION_UP, .5f)
        assertEquals(30_000, views().filterIsInstance<android.widget.SeekBar>().single().progress, 200)
        assertEquals(PlaybackService.ACTION_SEEK, shadowOf(activity).nextStartedService.action)
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(3))
        assertEquals(1_000, fullBar.progress)
        assertEquals(PlaybackStatus.Phase.PAUSED, PlaybackStatus.current.phase)
        expanded.dismiss()
    }

    @Test fun accessibilitySeekUsesTheSameCommandAndUnknownDurationDisablesScrubbing() {
        val bar = views().filterIsInstance<android.widget.SeekBar>().single()
        assertFalse(bar.isEnabled)
        PlaybackStatus.update(PlaybackStatus.State(PlaybackStatus.Phase.PAUSED, "Track", "server", 1_000, 60_000))
        assertTrue(bar.isEnabled)
        val args = android.os.Bundle().apply {
            putFloat(android.view.accessibility.AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE, 20_000f)
        }
        assertTrue(bar.performAccessibilityAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS.id, args))
        assertEquals(20_000, shadowOf(activity).nextStartedService.getIntExtra(PlaybackService.EXTRA_POSITION, -1))
        assertEquals(PlaybackStatus.Phase.PAUSED, PlaybackStatus.current.phase)
        PlaybackStatus.update(PlaybackStatus.State())
        assertFalse(bar.isEnabled)
    }

    private fun touch(bar: android.widget.SeekBar, action: Int, fraction: Float) {
        val x = bar.paddingLeft + (bar.width - bar.paddingLeft - bar.paddingRight) * fraction
        android.view.MotionEvent.obtain(0, 0, action, x, 24f, 0).also {
            bar.dispatchTouchEvent(it)
            it.recycle()
        }
    }
    private fun assertEquals(expected: Int, actual: Int, tolerance: Int) {
        assertTrue("Expected $expected ± $tolerance, got $actual", kotlin.math.abs(expected - actual) <= tolerance)
    }

    private fun button(text: String) = views().first { (it is Button && it.text.toString() == text) || it.contentDescription == text }
    private fun views(root: View = activity.findViewById(android.R.id.content)): Sequence<View> = sequence {
        yield(root)
        if (root is ViewGroup) for (index in 0 until root.childCount) yieldAll(views(root.getChildAt(index)))
    }
}
