package community.catalyst.ichoi.api

import community.catalyst.csilgen.generated.AsyncTransport
import community.catalyst.csilgen.generated.CborValue
import community.catalyst.csilgen.generated.CsilCbor
import community.catalyst.ichoi.security.ServerNetwork
import community.catalyst.ichoi.storage.ServerProfile
import java.io.IOException
import java.net.URI
import java.net.SocketTimeoutException
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString

/** CSIL-Events WebSocket carrier. It opens one transient socket for each RPC call. */
class CsilServiceException(val code: Long, message: String) : IOException(message)

class HttpCsilTransport(private val profile: ServerProfile, private val token: String?, private val timeoutMs: Long = 15_000) : AsyncTransport {
    companion object {
        private val deadlines = Executors.newSingleThreadScheduledExecutor { task ->
            Thread(task, "ichoi-request-deadlines").apply { isDaemon = true }
        }

        internal fun checkReply(payload: ByteArray) {
            val value = CsilCbor.decode(payload)
            val message = (CsilCbor.mapGet(value, "message") as? CborValue.CText)?.value ?: return
            val code = when (val field = CsilCbor.mapGet(value, "code")) {
                is CborValue.CUint -> if (field.value <= Long.MAX_VALUE.toULong()) field.value.toLong() else null
                is CborValue.CInt -> field.value
                else -> null
            }
            if (code != null) throw CsilServiceException(code, message)
        }
    }
    override suspend fun call(service: String, op: String, request: ByteArray): ByteArray = suspendCoroutine { continuation ->
        val endpoint = URI(profile.url.trimEnd('/') + "/ws").let {
            URI(if (it.scheme.equals("https", true)) "wss" else "ws", it.userInfo, it.host, it.port, it.path, null, null)
        }
        val completed = AtomicBoolean(false)
        var socket: WebSocket? = null
        var deadline: ScheduledFuture<*>? = null
        fun fail(error: Throwable) {
            if (completed.compareAndSet(false, true)) {
                deadline?.cancel(false)
                socket?.cancel()
                continuation.resumeWithException(error)
            }
        }
        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                socket = webSocket
                if (completed.get()) { webSocket.cancel(); return }
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
                    if (completed.get()) return
                    if (frame.event == "\$close" || frame.event == "\$error") {
                        fail(IOException("The server could not complete this request. Please try again."))
                    } else if (frame.event == "\$hello-ack") {
                        webSocket.send(envelope(service, op, 1uL, request).toByteString())
                    } else if (frame.event == op && frame.id == 1uL) {
                        checkReply(frame.payload)
                        if (completed.compareAndSet(false, true)) {
                            deadline?.cancel(false)
                            webSocket.close(1000, "complete")
                            continuation.resume(frame.payload)
                        }
                    }
                } catch (error: Throwable) {
                    fail(error)
                }
            }

            override fun onFailure(webSocket: WebSocket, error: Throwable, response: Response?) = fail(error)
            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                webSocket.close(code, reason)
                if (!completed.get()) fail(IOException("The server closed the connection. Please try again."))
            }
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                if (!completed.get()) fail(IOException("The server closed the CSIL connection"))
            }
        }
        try {
            deadline = deadlines.schedule({ fail(SocketTimeoutException("The server took too long to respond. Please try again.")) }, timeoutMs, TimeUnit.MILLISECONDS)
            socket = ServerNetwork.client(profile).newWebSocket(
                okhttp3.Request.Builder().url(endpoint.toString()).build(),
                listener,
            )
            if (completed.get()) socket?.cancel()
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
        if (tagged.tag != 24uL) throw IOException("CSIL envelope payload has an invalid tag")
        val payload = (tagged.value as? CborValue.CBytes)?.value
            ?: throw IOException("CSIL envelope payload is invalid")
        return Envelope(event, id, payload)
    }
}
