package community.catalyst.ichoi.storage

/** A server selected by the user. The pin is a SHA-256 certificate or SPKI digest. */
data class ServerProfile(
    val id: String,
    val url: String,
    val pinnedSha256: String,
    val allowPrivateHttp: Boolean = false,
)
