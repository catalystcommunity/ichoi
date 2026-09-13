package community.catalyst.ichoi.security

import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class ProfileSessionsTest {
    private class MemoryTokens : SessionTokens {
        private val values = mutableMapOf<String, String>()
        override fun save(profileId: String, token: String) { values[profileId] = token }
        override fun get(profileId: String): String? = values[profileId]
        override fun remove(profileId: String) { values.remove(profileId) }
    }

    @Test fun profilesReplaceAndSignOutSeparately() {
        val sessions = ProfileSessions(MemoryTokens())
        sessions.replace("one", "first")
        sessions.replace("two", "second")
        sessions.replace("one", "replacement")
        assertEquals("replacement", sessions.token("one"))
        assertEquals("second", sessions.token("two"))
        sessions.signOut("one")
        assertNull(sessions.token("one"))
        assertEquals("second", sessions.token("two"))
    }

    @Test fun deletionRemovesOnlyAfterServerSuccess() {
        val sessions = ProfileSessions(MemoryTokens())
        sessions.replace("one", "token")
        assertFailsWith<IllegalArgumentException> {
            runSuspend { sessions.deleteAfterConfirmation("one", "alice", "Alice") { true } }
        }
        assertEquals("token", sessions.token("one"))
        assertFailsWith<IllegalStateException> {
            runSuspend { sessions.deleteAfterConfirmation("one", "alice", "alice") { error("offline") } }
        }
        assertEquals("token", sessions.token("one"))
        assertEquals(false, runSuspend { sessions.deleteAfterConfirmation("one", "alice", "alice") { false } })
        assertEquals("token", sessions.token("one"))
        assertEquals(true, runSuspend { sessions.deleteAfterConfirmation("one", "alice", "alice") { true } })
        assertNull(sessions.token("one"))
    }

    private fun <T> runSuspend(block: suspend () -> T): T {
        var result: Result<T>? = null
        block.startCoroutine(object : Continuation<T> {
            override val context = EmptyCoroutineContext
            override fun resumeWith(value: Result<T>) { result = value }
        })
        return result!!.getOrThrow()
    }
}
