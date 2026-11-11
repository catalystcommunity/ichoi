package community.catalyst.ichoi.playback

import community.catalyst.ichoi.storage.ServerProfile
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okio.Buffer
import org.junit.Assert.*
import org.junit.Test

class MediaBufferTest {
    @Test fun cachedSeekUsesExactBytesWithoutAnotherRequest() {
        val server = MockWebServer(); server.start()
        val bytes = ByteArray(4096) { (it % 251).toByte() }
        server.enqueue(MockResponse().setResponseCode(206).setHeader("Content-Range", "bytes 0-4095/4096").setBody(Buffer().write(bytes)))
        val profile = ServerProfile("test", server.url("/").toString(), true)
        val source = ServerMediaDataSource(profile, server.url("/media/book").toString(), "test-bearer")
        try {
            val output = ByteArray(64)
            assertEquals(64, source.readAt(0, output, 0, 64))
            assertArrayEquals(bytes.copyOfRange(0, 64), output)
            assertEquals(64, source.readAt(3000, output, 0, 64))
            assertArrayEquals(bytes.copyOfRange(3000, 3064), output)
            assertEquals(1, server.requestCount)
            assertEquals("Bearer test-bearer", server.takeRequest().getHeader("Authorization"))
            assertEquals(-1, source.readAt(4096, output, 0, 64))
        } finally { source.close(); server.shutdown() }
    }
}
