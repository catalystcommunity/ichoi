package community.catalyst.ichoi.security

interface SessionTokens {
    fun save(profileId: String, token: String)
    fun get(profileId: String): String?
    fun remove(profileId: String)
}

/** Keeps session changes scoped to one server profile. */
class ProfileSessions(private val store: SessionTokens) {
    fun token(profileId: String): String? = store.get(profileId)
    fun replace(profileId: String, token: String) = store.save(profileId, token)
    fun signOut(profileId: String) = store.remove(profileId)

    suspend fun deleteAfterConfirmation(
        profileId: String,
        currentHandle: String,
        enteredHandle: String,
        request: suspend (String) -> Boolean,
    ): Boolean {
        require(enteredHandle == currentHandle) { "Enter the current handle exactly" }
        val deleted = request(enteredHandle)
        if (deleted) store.remove(profileId)
        return deleted
    }
}
