package community.catalyst.ichoi.security

import community.catalyst.ichoi.storage.ServerProfile
import java.io.IOException
import kotlin.test.Test
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse

class ServerNetworkTest {
    @Test fun httpsNeedsNoPinAndUsesThePlatformCertificateCheck() {
        val client = ServerNetwork.client(ServerProfile("demo", "https://ichoi.catalystsquad.com"))
        // No custom trust manager: OkHttp keeps its default hostname verifier.
        assertFalse(client.hostnameVerifier.javaClass.name.contains("Ichoi"))
        assertFalse(client.followRedirects)
        assertFalse(client.followSslRedirects)
    }

    @Test fun publicHttpIsStillRejected() {
        assertFailsWith<IOException> { ServerNetwork.client(ServerProfile("p", "http://203.0.113.10")) }
    }

    @Test fun privateHttpStillNeedsConsent() {
        assertFailsWith<IOException> { ServerNetwork.client(ServerProfile("p", "http://192.168.1.4")) }
        ServerNetwork.client(ServerProfile("p", "http://192.168.1.4", allowPrivateHttp = true))
    }
}
