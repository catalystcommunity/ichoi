package community.catalyst.ichoi.playback

import android.os.Looper
import community.catalyst.csilgen.generated.*
import community.catalyst.ichoi.storage.ServerProfile
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okio.ByteString
import okio.ByteString.Companion.toByteString
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.util.concurrent.TimeUnit

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29])
class RemotePlaybackTest {
    @Test fun targetStateAndQueueCommandsStayOnTheSelectedDevice() {
        val server = MockWebServer(); server.start()
        val player = Player("shared-kitchen", PlayerKind.Shared, "Kitchen")
        val queue = listOf(QueueItem(41uL, "track-a", title = "A"), QueueItem(42uL, "track-b", title = "B"))
        val state = PlayerState(player.id, 1uL, PlayerStatus.Paused, repeatMode = RepeatMode.Off, queue = queue)
        var received: CommandRequest? = null
        fun reply(operation: String, bytes: ByteArray) {
            server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
                override fun onMessage(socket: WebSocket, bytesIn: ByteString) {
                    val envelope = CsilCbor.decode(bytesIn.toByteArray())
                    val event = (CsilCbor.require(envelope, "event") as CborValue.CText).value
                    if (event == operation) {
                        if (operation == "control") {
                            val payload = CsilCbor.require(envelope, "payload") as CborValue.CTag
                            received = commandRequestFromCbor((payload.value as CborValue.CBytes).value)
                        }
                        socket.send(frame(event, bytes).toByteString())
                    } else if (event == "\$hello") socket.send(frame("\$hello-ack", byteArrayOf()).toByteString())
                }
                override fun onClosing(socket: WebSocket, code: Int, reason: String) { socket.close(code, reason) }
            }))
        }
        reply("get-state", state.toCbor())
        RemotePlayback.stop()
        RemotePlayback.configure(ServerProfile("remote-test", server.url("/").toString(), true), null)
        RemotePlayback.select(RuntimeEnvironment.getApplication(), player)
        waitFor { RemotePlayback.state?.revision == 1uL }
        reply("control", state.copy(revision = 2uL, queue = queue.reversed()).toCbor())
        RemotePlayback.move(1, 0)
        waitFor { RemotePlayback.state?.revision == 2uL }
        assertEquals(player.id, received?.playerId)
        assertEquals(CmdMoveItem("move-item", 42uL, 41uL), (received!!.command as PlayerCommandVariant5).value)
        assertEquals("track-b", RemotePlayback.state!!.queue.first().trackId)
        RemotePlayback.select(RuntimeEnvironment.getApplication(), null)
        assertNull(RemotePlayback.state)
        assertNull(RemotePlayback.target)
        server.shutdown()
    }
    private fun waitFor(condition: () -> Boolean) {
        val end = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < end) { shadowOf(Looper.getMainLooper()).idle(); Thread.sleep(10) }
        assertTrue(condition())
    }
    private fun frame(event: String, payload: ByteArray) = CsilCbor.encode(CborValue.CMap(listOf(
        CborValue.CText("event") to CborValue.CText(event),
        CborValue.CText("id") to CborValue.CUint(1uL),
        CborValue.CText("payload") to CborValue.CTag(24uL, CborValue.CBytes(payload)))))
}
