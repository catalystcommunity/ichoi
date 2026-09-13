package community.catalyst.ichoi.api

import community.catalyst.csilgen.generated.AsyncTransport
import community.catalyst.csilgen.generated.CborValue
import community.catalyst.csilgen.generated.CsilCbor
import community.catalyst.ichoi.security.PinnedNetwork
import community.catalyst.ichoi.storage.ServerProfile
import java.io.IOException
import java.net.URI
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString

/** CSIL-Events WebSocket carrier. It opens one transient socket for each RPC call. */
class HttpCsilTransport(private val profile: ServerProfile, private val token: String?) : AsyncTransport {
    override suspend fun call(service: String, op: String, request: ByteArray): ByteArray = suspendCoroutine { continuation ->
        val endpoint = URI(profile.url.trimEnd('/') + "/ws").let {
            URI(if (it.scheme.equals("https", true)) "wss" else "ws", it.userInfo, it.host, it.port, it.path, null, null)
        }
        val completed = AtomicBoolean(false)
        var socket: WebSocket? = null
        fun fail(error: Throwable) {
            if (completed.compareAndSet(false, true)) {
                socket?.cancel()
                continuation.resumeWithException(error)
            }
        }
        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                socket = webSocket
                val fields = mutableListOf<Pair<CborValue, CborValue>>(
                    CborValue.CText("versions") to CborValue.CArray(listOf(CborValue.CUint(1uL))),
                    CborValue.CText("profiles") to CborValue.CArray(listOf(CborValue.CText("verbose"))),
                )
                token?.let { fields += CborValue.CText("auth") to CborValue.CText(it) }
                webSocket.send(envelope(null, "\$hello", null, CsilCbor.encode(CborValue.CMap(fields))).toByteString())
            }

            override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                try {
                    val frame = decodeEnvelope(bytes.toByteArray())
                    if (frame.event == "\$hello-ack") {
                        webSocket.send(envelope(service, op, 1uL, request).toByteString())
                    } else if (frame.event == op && frame.id == 1uL && completed.compareAndSet(false, true)) {
                        webSocket.close(1000, "complete")
                        continuation.resume(frame.payload)
                    }
                } catch (error: Throwable) {
                    fail(error)
                }
            }

            override fun onFailure(webSocket: WebSocket, error: Throwable, response: Response?) = fail(error)
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                if (!completed.get()) fail(IOException("The server closed the CSIL connection"))
            }
        }
        try {
            socket = PinnedNetwork.client(profile).newWebSocket(
                okhttp3.Request.Builder().url(endpoint.toString()).build(),
                listener,
            )
        } catch (error: Throwable) {
            fail(error)
        }
    }

    private data class Envelope(val event: String, val id: ULong?, val payload: ByteArray)

    private fun envelope(service: String?, event: String, id: ULong?, payload: ByteArray): ByteArray {
        val fields = mutableListOf<Pair<CborValue, CborValue>>()
        id?.let { fields += CborValue.CText("id") to CborValue.CUint(it) }
        fields += CborValue.CText("event") to CborValue.CText(event)
        fields += CborValue.CText("payload") to CborValue.CTag(24uL, CborValue.CBytes(payload))
        service?.let { fields += CborValue.CText("service") to CborValue.CText(it) }
        return CsilCbor.encode(CborValue.CMap(fields))
    }

    private fun decodeEnvelope(bytes: ByteArray): Envelope {
        val value = CsilCbor.decode(bytes)
        val event = (CsilCbor.require(value, "event") as? CborValue.CText)?.value
            ?: throw IOException("CSIL envelope has no event")
        val id = (CsilCbor.mapGet(value, "id") as? CborValue.CUint)?.value
        val tagged = CsilCbor.require(value, "payload") as? CborValue.CTag
            ?: throw IOException("CSIL envelope has no payload")
        val payload = (tagged.value as? CborValue.CBytes)?.value
            ?: throw IOException("CSIL envelope payload is invalid")
        return Envelope(event, id, payload)
    }
}
