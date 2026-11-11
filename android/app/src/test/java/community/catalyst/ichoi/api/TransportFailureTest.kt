package community.catalyst.ichoi.api

import community.catalyst.csilgen.generated.*
import community.catalyst.ichoi.storage.ServerProfile
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.coroutines.*

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29])
class TransportFailureTest {
    @Test fun stalledHandshakeHasADeadline() {
        val server = MockWebServer()
        server.start()
        server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {}))
        val done = CountDownLatch(1)
        var failure: Throwable? = null
        val request: suspend () -> Unit = {
            HttpCsilTransport(ServerProfile("deadline", server.url("/").toString(), true), null, 150)
                .call("LibraryService", "list-libraries", Page().toCbor())
        }
        request.startCoroutine(object : Continuation<Unit> {
            override val context = EmptyCoroutineContext
            override fun resumeWith(result: Result<Unit>) { failure = result.exceptionOrNull(); done.countDown() }
        })
        assertTrue("A stalled handshake must finish", done.await(5, TimeUnit.SECONDS))
        assertTrue("Expected a deadline failure, got $failure", failure is java.net.SocketTimeoutException)
        server.shutdown()
    }

    @Test fun serviceErrorIsDecodedBeforeTheSuccessRecord() {
        try {
            HttpCsilTransport.checkReply(ServiceError(401, "sign in required").toCbor())
            fail("Expected a sign-in error")
        } catch (error: CsilServiceException) {
            assertEquals(401L, error.code)
            assertEquals("sign in required", error.message)
        }
        HttpCsilTransport.checkReply(LibrariesResponse(emptyList()).toCbor())
    }
}
