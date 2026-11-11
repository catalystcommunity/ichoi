package community.catalyst.ichoi.ui

import android.os.Looper
import android.view.View
import android.widget.Button
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.time.Duration

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29])
class MessageBannerTest {
    private lateinit var banner: MessageBanner
    private val main get() = shadowOf(Looper.getMainLooper())
    private val context get() = RuntimeEnvironment.getApplication()

    @Before fun setUp() {
        context.getSharedPreferences("app_settings", 0).edit().clear().commit()
        banner = MessageBanner(context)
        banner.start()
    }

    @After fun tearDown() {
        banner.stop()
        AppMessages.current?.let { AppMessages.dismiss(it) }
    }

    @Test fun defaultMessageRemainsForTenSeconds() {
        AppMessages.show("Connection failed")
        main.idleFor(Duration.ofMillis(9_999))
        assertEquals(View.VISIBLE, banner.visibility)
        main.idleFor(Duration.ofMillis(1))
        assertEquals(View.GONE, banner.visibility)
        assertNull(AppMessages.current)
    }

    @Test fun dismissButtonRemovesMessageImmediately() {
        AppMessages.show("Connection failed")
        (banner.getChildAt(1) as Button).performClick()
        assertNull(AppMessages.current)
        assertEquals(View.GONE, banner.visibility)
    }

    @Test fun replacementGetsItsOwnFullTimeoutEvenForIdenticalText() {
        AppMessages.show("Connection failed")
        val old = AppMessages.current!!
        main.idleFor(Duration.ofSeconds(9))
        AppMessages.show("Connection failed")
        AppMessages.dismiss(old)
        main.idleFor(Duration.ofSeconds(1))
        assertNotNull(AppMessages.current)
        main.idleFor(Duration.ofSeconds(9))
        assertNull(AppMessages.current)
    }

    @Test fun settingPersistsAndControlsTheNextMessage() {
        MessagePreferences(context).durationSeconds = 30
        assertEquals(30, MessagePreferences(context).durationSeconds)
        AppMessages.show("Connection failed")
        main.idleFor(Duration.ofSeconds(10))
        assertNotNull(AppMessages.current)
        main.idleFor(Duration.ofSeconds(20))
        assertNull(AppMessages.current)
    }

    @Test fun backgroundFailureWaitsForAVisibleBanner() {
        banner.stop()
        AppMessages.show("Playback failed")
        main.idleFor(Duration.ofSeconds(60))
        assertNotNull(AppMessages.current)
        banner.start()
        main.idleFor(Duration.ofSeconds(9))
        assertNotNull(AppMessages.current)
        main.idleFor(Duration.ofSeconds(1))
        assertNull(AppMessages.current)
    }
}
