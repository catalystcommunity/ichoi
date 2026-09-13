package community.catalyst.ichoi.storage

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

class ServerProfileStore(context: Context) {
    private val prefs = context.getSharedPreferences("server_profiles", Context.MODE_PRIVATE)

    @Synchronized
    fun all(): List<ServerProfile> {
        val raw = prefs.getString(KEY_PROFILES, "[]") ?: "[]"
        val json = JSONArray(raw)
        return (0 until json.length()).map { i ->
            val item = json.getJSONObject(i)
            ServerProfile(item.getString("id"), item.getString("url"), item.optString("pin"), item.optBoolean("privateHttp", false))
        }
    }

    @Synchronized
    fun put(url: String, pin: String, allowPrivateHttp: Boolean = false): ServerProfile {
        val existing = all().firstOrNull { it.url == url }
        val profile = existing?.copy(pinnedSha256 = pin, allowPrivateHttp = allowPrivateHttp) ?: ServerProfile(UUID.randomUUID().toString(), url, pin, allowPrivateHttp)
        val values = all().filterNot { it.id == profile.id } + profile
        val json = JSONArray().apply {
            values.forEach { p -> put(JSONObject().apply { put("id", p.id); put("url", p.url); put("pin", p.pinnedSha256); put("privateHttp", p.allowPrivateHttp) }) }
        }
        prefs.edit().putString(KEY_PROFILES, json.toString()).apply()
        return profile
    }

    fun remove(id: String) {
        val json = JSONArray().apply {
            all().filterNot { it.id == id }.forEach { p ->
                put(JSONObject().apply { put("id", p.id); put("url", p.url); put("pin", p.pinnedSha256); put("privateHttp", p.allowPrivateHttp) })
            }
        }
        prefs.edit().putString(KEY_PROFILES, json.toString()).apply()
    }

    companion object { private const val KEY_PROFILES = "profiles" }
}
