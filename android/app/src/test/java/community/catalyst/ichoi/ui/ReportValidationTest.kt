package community.catalyst.ichoi.ui

import kotlin.test.Test
import kotlin.test.assertFailsWith
import kotlin.test.assertEquals

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
}
