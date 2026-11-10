package community.catalyst.ichoi.storage

/** A server selected by the user. */
data class ServerProfile(
    val id: String,
    val url: String,
    val allowPrivateHttp: Boolean = false,
)
