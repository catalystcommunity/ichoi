package community.catalyst.ichoi.security

import java.net.Inet6Address
import java.net.InetAddress
import java.net.URI

sealed class EndpointDecision {
    data object Secure : EndpointDecision()
    data class PrivateHttpNeedsConsent(val uri: URI) : EndpointDecision()
    data class Rejected(val reason: String) : EndpointDecision()
}

object ConnectionSecurity {
    fun normalize(raw: String): String {
        val value = raw.trim()
        if (value.isEmpty() || value.contains("://")) return value
        return "https://$value"
    }
    fun check(rawUrl: String, allowPrivateHttp: Boolean = false): EndpointDecision {
        val uri = runCatching { URI(normalize(rawUrl)) }.getOrElse { return EndpointDecision.Rejected("Invalid server URL") }
        if (uri.userInfo != null || uri.host.isNullOrBlank() || uri.rawQuery != null || uri.rawFragment != null ||
            (uri.path.isNotEmpty() && uri.path != "/")) {
            return EndpointDecision.Rejected("Use only the server origin without credentials, a path, a query, or a fragment")
        }
        return when (uri.scheme?.lowercase()) {
            "https" -> EndpointDecision.Secure
            "http" -> if (isPrivate(uri.host!!)) {
                if (allowPrivateHttp) EndpointDecision.Secure else EndpointDecision.PrivateHttpNeedsConsent(uri)
            } else EndpointDecision.Rejected("Plain HTTP is allowed only for private network addresses")
            else -> EndpointDecision.Rejected("Use HTTPS")
        }
    }

    fun isPrivate(host: String): Boolean {
        val normalized = host.removePrefix("[").removeSuffix("]").lowercase()
        if (normalized == "localhost" || normalized.endsWith(".localhost")) return true
        if (!normalized.contains(':') && normalized.any { !it.isDigit() && it != '.' }) return false
        val address = runCatching { InetAddress.getByName(normalized) }.getOrNull() ?: return false
        if (address.isLoopbackAddress || address.isLinkLocalAddress || address.isSiteLocalAddress) return true
        if (address is Inet6Address) return (address.address.first().toInt() and 0xfe) == 0xfc
        val octets = address.address.map { it.toInt() and 0xff }
        return octets.size == 4 && (octets[0] == 10 || octets[0] == 127 ||
            (octets[0] == 169 && octets[1] == 254) ||
            (octets[0] == 172 && octets[1] in 16..31) ||
            (octets[0] == 192 && octets[1] == 168))
    }

    /** Redirects are never followed. Reconnect to the exact user-selected URL instead. */
    fun allowRedirect(statusCode: Int): Boolean = statusCode !in 300..399
}
