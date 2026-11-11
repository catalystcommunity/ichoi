package community.catalyst.ichoi.ui

import android.os.Looper
import android.content.Intent
import android.net.Uri
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.ScrollView
import android.widget.TextView
import community.catalyst.csilgen.generated.*
import community.catalyst.ichoi.api.IchoiApi
import community.catalyst.ichoi.storage.ServerProfile
import community.catalyst.ichoi.storage.ServerProfileStore
import community.catalyst.ichoi.security.ProfileSessions
import community.catalyst.ichoi.security.SessionTokens
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okio.ByteString
import okio.ByteString.Companion.toByteString
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.android.controller.ActivityController
import org.robolectric.annotation.Config
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29])
class SettingsNavigationTest {
    private lateinit var controller: ActivityController<MainActivity>
    private lateinit var activity: MainActivity
    private lateinit var api: IchoiApi
    private val server = MockWebServer()
    private val replies = LinkedBlockingQueue<() -> Unit>()

    @Before fun setUp() {
        server.start()
        controller = Robolectric.buildActivity(MainActivity::class.java).setup()
        activity = controller.get()
        val profile = ServerProfile("test-server", server.url("/").toString(), true)
        api = IchoiApi(profile, "test-token")
        setField("profile", profile)
        setField("sessionInfo", SessionInfo("test-user", "user@example.com", role = Role.Member))
    }

    @After fun tearDown() {
        controller.pause().stop().destroy()
        server.shutdown()
    }

    @Test fun blockedAccountsAppearOnlyInSettings() {
        openSettings()
        completeBlockedAccounts()
        assertTrue(labels().contains("Blocked accounts"))
        assertTrue(labels().contains("You have not blocked an account on this server."))
    }

    @Test fun lateBlockedAccountsDoNotAppearInLibrary() {
        openSettings()
        val reply = replies.poll(5, TimeUnit.SECONDS) ?: error("No blocked-account request")
        openLibrary()
        reply()
        drainReply()
        assertTrue(labels().contains("Your Ichoi library"))
        assertFalse(labels().contains("Blocked accounts"))
        assertFalse(labels().contains("You have not blocked an account on this server."))
    }

    @Test fun lateBlockedAccountFailureDoesNotAppearInLibrary() {
        openSettings(invalidResponse = true)
        val reply = replies.poll(5, TimeUnit.SECONDS) ?: error("No blocked-account request")
        openLibrary()
        reply()
        drainReply()
        assertFalse(labels().any { it.startsWith("Could not load blocked accounts") })
    }

    @Test fun successfulSignInOpensLibrary() {
        val selected = ServerProfileStore(activity).put(server.url("/").toString(), true)
        val tokens = mutableMapOf<String, String>()
        setField("sessions", ProfileSessions(object : SessionTokens {
            override fun save(profileId: String, token: String) { tokens[profileId] = token }
            override fun get(profileId: String) = tokens[profileId]
            override fun remove(profileId: String) { tokens.remove(profileId) }
        }))
        activity.getPreferences(0).edit().putString("pendingLoginProfile", selected.id).commit()
        val signedIn = SessionInfo("test-user", "user@example.com", role = Role.Member, token = "new-token")
        enqueueResponse("authenticate", signedIn.toCbor())
        enqueueResponse("list-libraries", LibrariesResponse(emptyList()).toCbor())
        enqueueResponse("whoami", signedIn.toCbor())
        invoke("handleLoginCallback", arrayOf(Intent::class.java),
            Intent(Intent.ACTION_VIEW, Uri.parse("ichoi://linkkeys#linkkeys_exchange=test-code")))
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!labels().contains("Playlists") && System.nanoTime() < deadline) {
            shadowOf(Looper.getMainLooper()).idle()
            Thread.sleep(10)
        }
        assertEquals("new-token", tokens[selected.id])
        assertTrue(labels().contains("Your Ichoi library"))
        assertTrue(labels().contains("Playlists"))
        assertFalse(labels().contains("Blocked accounts"))
        assertFalse(activity.getPreferences(0).contains("pendingLoginProfile"))
    }

    @Test fun lateResponseDoesNotPopulateASecondSettingsVisit() {
        openSettings()
        val firstReply = replies.poll(5, TimeUnit.SECONDS) ?: error("No first request")
        openLibrary()
        openSettings()
        val secondReply = replies.poll(5, TimeUnit.SECONDS) ?: error("No second request")
        firstReply()
        drainReply()
        assertFalse(labels().contains("Blocked accounts"))
        secondReply()
        drainReply()
        assertEquals(1, labels().count { it == "Blocked accounts" })
    }

    @Test fun libraryStartsAtTopAfterScrollingSettings() {
        openSettings((1..30).map {
            BlockedAccount("blocked-$it", "blocked-$it@example.com", blockedAt = java.time.Instant.EPOCH)
        })
        completeBlockedAccounts()
        activity.window.decorView.apply {
            measure(View.MeasureSpec.makeMeasureSpec(400, View.MeasureSpec.EXACTLY),
                View.MeasureSpec.makeMeasureSpec(600, View.MeasureSpec.EXACTLY))
            layout(0, 0, 400, 600)
        }
        val oldScroll = views().filterIsInstance<ScrollView>().single()
        oldScroll.scrollTo(0, 400)
        assertTrue("Settings must be scrolled before navigation", oldScroll.scrollY > 0)
        openLibrary()
        val libraryScroll = views().filterIsInstance<ScrollView>().single()
        assertEquals(0, libraryScroll.scrollY)
    }

    @Test fun backToLibraryPrecedesAccountControls() {
        openSettings()
        completeBlockedAccounts()
        val buttons = views().filterIsInstance<Button>().map { it.text.toString() }.toList()
        assertTrue(buttons.indexOf("Back to library") < buttons.indexOf("Sign out"))
    }

    @Test fun queueBackRestoresTheSameSearchViewAndScroll() {
        invoke("showSearch", arrayOf(IchoiApi::class.java), api)
        val before = views().filterIsInstance<ScrollView>().single()
        val input = views().filterIsInstance<android.widget.EditText>().single()
        input.setText("restored search")
        before.scrollTo(0, 120)
        val position = before.scrollY
        invoke("showQueue", arrayOf(Boolean::class.javaPrimitiveType!!), false)
        assertTrue(labels().contains("Now playing"))
        invoke("backToBrowse", emptyArray())
        val restored = views().filterIsInstance<ScrollView>().single()
        assertSame(before, restored)
        assertEquals("restored search", views().filterIsInstance<android.widget.EditText>().single().text.toString())
        assertEquals(position, restored.scrollY)
    }

    private fun openSettings(accounts: List<BlockedAccount> = emptyList(), invalidResponse: Boolean = false) {
        val payload = if (invalidResponse) byteArrayOf() else BlockedAccountsResponse(accounts, accounts.size.toULong()).toCbor()
        enqueueResponse("list-blocked-accounts", payload, delayed = true)
        invoke("showSettings", arrayOf(IchoiApi::class.java, BlockedAccountsResponse::class.java), api, null)
        shadowOf(Looper.getMainLooper()).idle()
    }

    private fun enqueueResponse(operation: String, payload: ByteArray, delayed: Boolean = false) {
        server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                val request = CsilCbor.decode(bytes.toByteArray())
                val event = (CsilCbor.require(request, "event") as CborValue.CText).value
                if (event == "\$hello") {
                    webSocket.send(envelope("\$hello-ack", byteArrayOf()).toByteString())
                } else if (event == operation) {
                    val reply: () -> Unit = { webSocket.send(envelope(event, payload).toByteString()); Unit }
                    if (delayed) replies.put(reply) else reply()
                }
            }
            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                webSocket.close(code, reason)
            }
        }))
    }

    private fun openLibrary() {
        invoke("showLibrary", arrayOf(IchoiApi::class.java, LibrariesResponse::class.java), api, LibrariesResponse(emptyList()))
        shadowOf(Looper.getMainLooper()).idle()
    }

    private fun completeBlockedAccounts() {
        val reply = replies.poll(5, TimeUnit.SECONDS) ?: error("No blocked-account request")
        reply()
        drainReply()
    }

    private fun drainReply() {
        val main = shadowOf(Looper.getMainLooper())
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (main.isIdle && System.nanoTime() < deadline) Thread.sleep(10)
        assertFalse("The response did not reach the UI thread", main.isIdle)
        main.idle()
    }

    private fun envelope(event: String, payload: ByteArray) = CsilCbor.encode(CborValue.CMap(listOf(
        CborValue.CText("event") to CborValue.CText(event),
        CborValue.CText("id") to CborValue.CUint(1uL),
        CborValue.CText("payload") to CborValue.CTag(24uL, CborValue.CBytes(payload)),
    )))

    private fun setField(name: String, value: Any) {
        MainActivity::class.java.getDeclaredField(name).apply { isAccessible = true }.set(activity, value)
    }

    private fun invoke(name: String, types: Array<Class<*>>, vararg values: Any?) {
        MainActivity::class.java.getDeclaredMethod(name, *types).apply { isAccessible = true }.invoke(activity, *values)
    }

    private fun views(root: View = activity.findViewById(android.R.id.content)): Sequence<View> = sequence {
        yield(root)
        if (root is ViewGroup) for (index in 0 until root.childCount) yieldAll(views(root.getChildAt(index)))
    }

    private fun labels() = views().filterIsInstance<TextView>().map { it.text.toString() }.toList()
}
