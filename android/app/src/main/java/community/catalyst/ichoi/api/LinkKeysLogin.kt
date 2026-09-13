package community.catalyst.ichoi.api

import community.catalyst.ichoi.security.PinnedNetwork
import community.catalyst.ichoi.storage.ServerProfile
import java.io.IOException
import java.net.URI
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject

/** Starts the server-selected LinkKeys provider and returns its browser URL. */
object LinkKeysLogin {
    fun start(profile: ServerProfile, identity: String): String {
        require(identity.isNotBlank()) { "Enter a LinkKeys identity" }
        val client = PinnedNetwork.client(profile)
        val statusRequest = Request.Builder().url(resolve(profile.url, "/api/auth")).get().build()
        val status = client.newCall(statusRequest).execute().use { response ->
            if (!response.isSuccessful) throw IOException("The server did not provide login settings")
            JSONObject(response.body?.string() ?: throw IOException("The login settings were empty"))
        }
        val provider = status.optJSONObject("regular_rp") ?: status.optJSONObject("local_rp")
            ?: throw IOException("LinkKeys login is not enabled on this server")
        val startPath = provider.getString("start_url")
        val base = URI(profile.url)
        val origin = URI(base.scheme, null, base.host, base.port, null, null, null).toString()
        val body = JSONObject()
            .put("identity", identity.trim())
            .put("return_url", "ichoi://linkkeys")
            .toString()
            .toRequestBody("application/json".toMediaType())
        val request = Request.Builder()
            .url(resolve(profile.url, startPath))
            .header("Origin", origin)
            .post(body)
            .build()
        return client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) throw IOException("The server rejected the login request")
            JSONObject(response.body?.string() ?: throw IOException("The login response was empty"))
                .getString("redirect_url")
        }
    }

    private fun resolve(base: String, path: String): String =
        base.trimEnd('/') + "/" + path.trimStart('/')
}
