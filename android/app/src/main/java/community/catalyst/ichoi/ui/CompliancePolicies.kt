package community.catalyst.ichoi.ui

import java.net.URI
import java.net.URLDecoder
import java.nio.charset.StandardCharsets

object FirstRunPolicy {
    fun canConnect(termsAccepted: Boolean): Boolean = termsAccepted
}

object AccountValidation {
    fun requireExactHandle(entered: String, current: String) {
        require(entered == current) { "Enter the current handle exactly" }
    }
}

object ReportValidation {
    fun validate(targetId: String, details: String?): String? {
        require(targetId.isNotBlank()) { "A report target is required" }
        if (details != null) {
            require(details.codePointCount(0, details.length) <= ReportDialog.MAX_DETAILS) {
                "Details are limited to 2,000 Unicode scalar values"
            }
        }
        return details?.takeIf { it.isNotBlank() }
    }
}

object LinkKeysCallback {
    fun exchangeCode(raw: String): String? {
        val uri = runCatching { URI(raw) }.getOrNull() ?: return null
        if (uri.scheme != "ichoi" || uri.host != "linkkeys") return null
        return uri.rawFragment.orEmpty().split('&').mapNotNull { part ->
            val values = part.split('=', limit = 2)
            if (values.size == 2 && values[0] == "linkkeys_exchange") {
                URLDecoder.decode(values[1], StandardCharsets.UTF_8.name())
            } else null
        }.firstOrNull { it.isNotEmpty() }
    }
}
