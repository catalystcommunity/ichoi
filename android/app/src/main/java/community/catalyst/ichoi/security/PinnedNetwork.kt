package community.catalyst.ichoi.security

import community.catalyst.ichoi.storage.ServerProfile
import java.io.IOException
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager
import okhttp3.OkHttpClient

/** Creates transient clients that apply the selected server policy and TLS pins. */
object PinnedNetwork {
    fun client(profile: ServerProfile): OkHttpClient {
        when (val decision = ConnectionSecurity.check(profile.url, profile.allowPrivateHttp)) {
            EndpointDecision.Secure -> Unit
            is EndpointDecision.PrivateHttpNeedsConsent -> throw IOException("Private HTTP requires consent")
            is EndpointDecision.Rejected -> throw IOException(decision.reason)
        }
        val builder = OkHttpClient.Builder()
            .followRedirects(false)
            .followSslRedirects(false)
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
        if (profile.url.startsWith("https://", true)) {
            val pins = profile.pinnedSha256.split(',').map(String::trim).filter(String::isNotEmpty)
            if (pins.isEmpty()) throw IOException("At least one TLS pin is required for HTTPS")
            val trust = object : X509TrustManager {
                override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
                override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) = Unit
                override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {
                    if (chain.isEmpty() || chain.none { certificate ->
                            pins.any { ConnectionSecurity.pinMatches(it, certificate.encoded, certificate.publicKey.encoded) }
                        }) {
                        throw CertificateException("Ichoi TLS pin mismatch")
                    }
                }
            }
            val context = SSLContext.getInstance("TLS").apply { init(null, arrayOf<TrustManager>(trust), null) }
            builder.sslSocketFactory(context.socketFactory, trust)
        }
        return builder.build()
    }
}
