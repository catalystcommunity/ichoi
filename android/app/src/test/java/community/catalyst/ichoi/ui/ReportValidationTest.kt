package community.catalyst.ichoi.ui

import kotlin.test.Test
import kotlin.test.assertFailsWith
import kotlin.test.assertEquals
import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine

class ReportValidationTest {
    @Test fun unicodeScalarLimitIsDocumented() {
        val details = "😀".repeat(ReportDialog.MAX_DETAILS)
        check(details.codePointCount(0, details.length) == ReportDialog.MAX_DETAILS)
        assertFailsWith<IllegalArgumentException> {
            require(details.plus("😀").codePointCount(0, details.length + 2) <= ReportDialog.MAX_DETAILS)
        }
    }

    @Test fun targetAndDetailsAreValidated() {
        assertFailsWith<IllegalArgumentException> { ReportValidation.validate("", null) }
        assertFailsWith<IllegalArgumentException> {
            ReportValidation.validate("playlist-1", "😀".repeat(ReportDialog.MAX_DETAILS + 1))
        }
        assertEquals(null, ReportValidation.validate("playlist-1", ""))
        assertEquals("context", ReportValidation.validate("account-1", "context"))
    }

    @Test fun deletionHandleMustMatchExactly() {
        AccountValidation.requireExactHandle("sam@example", "sam@example")
        assertFailsWith<IllegalArgumentException> {
            AccountValidation.requireExactHandle("Sam@example", "sam@example")
        }
    }

    @Test fun firstRunAndLinkKeysCallbackAreStable() {
        check(!FirstRunPolicy.canConnect(false))
        check(FirstRunPolicy.canConnect(true))
        assertEquals("one time", LinkKeysCallback.exchangeCode("ichoi://linkkeys#linkkeys_exchange=one%20time"))
        assertEquals(null, LinkKeysCallback.exchangeCode("https://attacker.example/#linkkeys_exchange=code"))
    }

    @Test fun accountBlockActionsKeepTheSelectedProfileAndRefreshOnlyAfterSuccess() {
        val calls = mutableListOf<String>()
        val refreshed = runSuspend {
            AccountBlockAction.change(
                "server-one",
                "blocked@example.com",
                { profile, account -> calls += "$profile:$account" },
                { profile -> calls += "refresh:$profile"; "refreshed:$profile" },
            )
        }
        assertEquals("refreshed:server-one", refreshed)
        assertEquals(
            listOf("server-one:blocked@example.com", "refresh:server-one"),
            calls,
        )
        assertFailsWith<IllegalStateException> {
            runSuspend {
                AccountBlockAction.change(
                    "server-two",
                    "blocked@example.com",
                    { _, _ -> error("service failed") },
                    { profile -> calls += "refresh:$profile" },
                )
            }
        }
        assertEquals(false, calls.contains("refresh:server-two"))
        assertFailsWith<IllegalArgumentException> {
            runSuspend { AccountBlockAction.change("", "blocked@example.com", { _, _ -> }, { _ -> }) }
        }
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
