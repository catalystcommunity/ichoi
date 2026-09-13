package community.catalyst.ichoi.security

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue

class ConnectionSecurityTest {
    @Test fun httpsIsAccepted() = assertIs<EndpointDecision.Secure>(ConnectionSecurity.check("https://music.example"))

    @Test fun publicHttpIsRejected() = assertIs<EndpointDecision.Rejected>(ConnectionSecurity.check("http://203.0.113.10"))

    @Test fun publicHostnamesAreNotResolvedIntoPrivateAddresses() =
        assertIs<EndpointDecision.Rejected>(ConnectionSecurity.check("http://music.example"))

    @Test fun privateHttpNeedsConsent() = assertIs<EndpointDecision.PrivateHttpNeedsConsent>(ConnectionSecurity.check("http://192.168.1.4"))

    @Test fun privateHttpIsAcceptedAfterConsent() = assertIs<EndpointDecision.Secure>(ConnectionSecurity.check("http://192.168.1.4", true))

    @Test fun ipv6LoopbackIsPrivate() = assertIs<EndpointDecision.PrivateHttpNeedsConsent>(ConnectionSecurity.check("http://[::1]"))

    @Test fun credentialsAndUnknownSchemesAreRejected() {
        assertIs<EndpointDecision.Rejected>(ConnectionSecurity.check("https://u:p@example.test"))
        assertIs<EndpointDecision.Rejected>(ConnectionSecurity.check("https://example.test/path"))
        assertIs<EndpointDecision.Rejected>(ConnectionSecurity.check("https://example.test?query=1"))
        assertIs<EndpointDecision.Rejected>(ConnectionSecurity.check("ftp://example.test"))
    }

    @Test fun redirectsAreRejected() {
        assertFalse(ConnectionSecurity.allowRedirect(301))
        assertTrue(ConnectionSecurity.allowRedirect(200))
    }

    @Test fun pinMismatchIsRejected() {
        assertFalse(ConnectionSecurity.pinMatches("00", byteArrayOf(1), byteArrayOf(2)))
    }
}
