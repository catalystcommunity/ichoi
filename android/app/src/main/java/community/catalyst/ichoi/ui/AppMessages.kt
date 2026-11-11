package community.catalyst.ichoi.ui

import android.content.Context

/** Messages stay available until a visible banner dismisses them. Use on the main thread. */
object AppMessages {
    data class Message(val text: String)
    var current: Message? = null
        private set
    private val listeners = mutableSetOf<(Message?) -> Unit>()

    fun show(text: String) {
        current = Message(text)
        listeners.toList().forEach { it(current) }
    }

    fun dismiss(message: Message) {
        if (current !== message) return
        current = null
        listeners.toList().forEach { it(null) }
    }

    fun observe(listener: (Message?) -> Unit) { listeners.add(listener); listener(current) }
    fun removeObserver(listener: (Message?) -> Unit) { listeners.remove(listener) }
}

class MessagePreferences(context: Context) {
    private val prefs = context.getSharedPreferences("app_settings", Context.MODE_PRIVATE)
    var durationSeconds: Int
        get() = prefs.getInt("messageDurationSeconds", 10).coerceIn(1, 60)
        set(value) {
            require(value in 1..60) { "Enter a duration from 1 to 60 seconds." }
            prefs.edit().putInt("messageDurationSeconds", value).apply()
        }
}
