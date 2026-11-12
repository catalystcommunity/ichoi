package community.catalyst.ichoi.ui

import android.app.Activity
import android.os.Looper
import com.google.android.play.core.appupdate.testing.FakeAppUpdateManager
import com.google.android.play.core.install.model.InstallErrorCode
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29])
class PlayUpdatesTest {
    private val controller = Robolectric.buildActivity(Activity::class.java)
    private lateinit var manager: FakeAppUpdateManager
    private lateinit var updates: PlayUpdates
    @Before fun setup() {
        controller.setup()
        manager = FakeAppUpdateManager(controller.get())
        updates = PlayUpdates(controller.get(), manager) {}
        manager.setUpdateAvailable(200)
        updates.start(); idle()
    }
    @After fun cleanup() { updates.close(); controller.pause().stop().destroy() }
    private fun idle() = shadowOf(Looper.getMainLooper()).idle()
    private fun consent() { updates.check(true); idle(); manager.userAcceptsUpdate(); manager.downloadStarts(); idle() }

    @Test fun backgroundCheckDoesNotShowConsentOrInterruptPlayback() {
        assertEquals(PlayUpdates.State.AVAILABLE, updates.state)
        assertFalse(manager.isConfirmationDialogVisible)
        assertFalse(manager.isImmediateFlowVisible)
    }
    @Test fun downloadWaitsForExplicitRestartAndSurvivesBackground() {
        consent()
        assertEquals(PlayUpdates.State.DOWNLOADING, updates.state)
        updates.stop()
        manager.downloadCompletes(); idle()
        updates.start(); idle()
        assertEquals(PlayUpdates.State.READY, updates.state)
        assertFalse(manager.isInstallSplashScreenVisible)
        updates.install(); idle()
        assertTrue(manager.isInstallSplashScreenVisible)
    }
    @Test fun declinedConsentCanBeRetriedWithFreshInfo() {
        updates.check(true); idle()
        assertTrue(manager.isConfirmationDialogVisible)
        manager.userRejectsUpdate(); updates.result(Activity.RESULT_CANCELED); idle()
        assertEquals(PlayUpdates.State.AVAILABLE, updates.state)
        updates.check(true); idle()
        assertTrue(manager.isConfirmationDialogVisible)
    }
    @Test fun failedDownloadCanBeRetried() {
        consent()
        manager.downloadFails(); idle()
        assertEquals(PlayUpdates.State.FAILED, updates.state)
        updates.check(true); idle()
        assertTrue(manager.isConfirmationDialogVisible)
    }
    @Test fun failedCheckRecoversAndClosedControllerDoesNotPublish() {
        manager.setInstallErrorCode(InstallErrorCode.ERROR_UNKNOWN)
        updates.check(); idle()
        assertEquals(PlayUpdates.State.FAILED, updates.state)
        manager.setInstallErrorCode(InstallErrorCode.NO_ERROR)
        manager.setUpdateNotAvailable()
        updates.check(); idle()
        assertEquals(PlayUpdates.State.CURRENT, updates.state)
        updates.close()
        manager.setUpdateAvailable(201)
        updates.check(); idle()
        assertEquals(PlayUpdates.State.CURRENT, updates.state)
    }
}
