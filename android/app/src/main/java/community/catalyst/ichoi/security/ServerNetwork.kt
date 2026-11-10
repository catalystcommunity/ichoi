package community.catalyst.ichoi.security

import community.catalyst.ichoi.storage.ServerProfile
import java.io.IOException
import java.util.concurrent.TimeUnit
import okhttp3.OkHttpClient

/**
 * Creates transient clients that apply the selected server policy. HTTPS uses the normal
 * platform certificate check, the same as a browser.
 */
object ServerNetwork {
    fun client(profile: ServerProfile): OkHttpClient {
        when (val decision = ConnectionSecurity.check(profile.url, profile.allowPrivateHttp)) {
            EndpointDecision.Secure -> Unit
            is EndpointDecision.PrivateHttpNeedsConsent -> throw IOException("Private HTTP requires consent")
            is EndpointDecision.Rejected -> throw IOException(decision.reason)
        }
        return OkHttpClient.Builder()
            .followRedirects(false)
            .followSslRedirects(false)
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .build()
    }
}
