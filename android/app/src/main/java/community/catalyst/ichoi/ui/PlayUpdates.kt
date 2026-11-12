package community.catalyst.ichoi.ui

import android.app.Activity
import com.google.android.play.core.appupdate.AppUpdateManager
import com.google.android.play.core.appupdate.AppUpdateManagerFactory
import com.google.android.play.core.appupdate.AppUpdateOptions
import com.google.android.play.core.install.InstallStateUpdatedListener
import com.google.android.play.core.install.model.AppUpdateType
import com.google.android.play.core.install.model.InstallStatus
import com.google.android.play.core.install.model.UpdateAvailability

/** Google Play owns consent, download, verification, and installation. */
class PlayUpdates(
    private val activity: Activity,
    private val manager: AppUpdateManager = AppUpdateManagerFactory.create(activity),
    private val changed: (State) -> Unit,
) {
    enum class State { IDLE, CHECKING, CURRENT, AVAILABLE, DOWNLOADING, READY, FAILED, UNAVAILABLE }
    var state = State.IDLE; private set
    private var closed = false
    private var checking = false
    private var listening = false
    private var foreground = false
    private val listener = InstallStateUpdatedListener { update -> installState(update.installStatus()) }
    private fun set(value: State) { if (!closed) { state = value; changed(value) } }
    private fun installState(status: Int): Boolean {
        when (status) {
            InstallStatus.DOWNLOADED -> set(State.READY)
            InstallStatus.DOWNLOADING, InstallStatus.PENDING, InstallStatus.INSTALLING -> set(State.DOWNLOADING)
            InstallStatus.FAILED -> set(State.FAILED)
            InstallStatus.CANCELED -> set(State.AVAILABLE)
            else -> return false
        }
        return true
    }
    fun start() {
        foreground = true
        if (!listening) { manager.registerListener(listener); listening = true }
        check()
    }
    fun stop() { foreground = false; if (listening) { manager.unregisterListener(listener); listening = false } }
    fun close() { stop(); closed = true }
    fun unavailable() { set(State.UNAVAILABLE) }
    fun check(requestInstall: Boolean = false) {
        if (closed || checking) return
        checking = true
        set(State.CHECKING)
        manager.appUpdateInfo.addOnSuccessListener { info ->
            checking = false
            if (closed) return@addOnSuccessListener
            val status = info.installStatus()
            val canRetry = requestInstall && (status == InstallStatus.FAILED || status == InstallStatus.CANCELED)
            if (!canRetry && installState(status)) return@addOnSuccessListener
            if (info.updateAvailability() != UpdateAvailability.UPDATE_AVAILABLE) { set(State.CURRENT); return@addOnSuccessListener }
            if (!info.isUpdateTypeAllowed(AppUpdateType.FLEXIBLE)) { set(State.UNAVAILABLE); return@addOnSuccessListener }
            set(State.AVAILABLE)
            if (requestInstall && foreground && !activity.isFinishing && !activity.isDestroyed) {
                try {
                    // A fresh AppUpdateInfo is required for each consent attempt.
                    if (!manager.startUpdateFlowForResult(info, activity, AppUpdateOptions.newBuilder(AppUpdateType.FLEXIBLE).build(), REQUEST_CODE)) set(State.FAILED)
                } catch (_: Exception) { set(State.FAILED) }
            }
        }.addOnFailureListener { checking = false; set(State.FAILED) }
    }
    fun result(resultCode: Int) {
        if (resultCode == Activity.RESULT_OK) check()
        else if (resultCode == Activity.RESULT_CANCELED) set(State.AVAILABLE)
        else set(State.FAILED)
    }
    /** Call only after the person agrees to restart. Playback can be active. */
    fun install() {
        if (state != State.READY || closed) return
        set(State.DOWNLOADING)
        manager.completeUpdate().addOnFailureListener { set(State.FAILED) }
    }
    companion object { const val REQUEST_CODE = 412 }
}
