package community.catalyst.ichoi.playback

/** Process-local playback state. The service and views access it on the main thread. */
object PlaybackStatus {
    enum class Phase { STOPPED, LOADING, PLAYING, PAUSED }
    data class State(
        val phase: Phase = Phase.STOPPED,
        val title: String = "",
        val server: String = "",
        val positionMs: Int = 0,
        val durationMs: Int = 0,
    )

    var current = State()
        private set
    private val listeners = mutableSetOf<(State) -> Unit>()
    fun update(value: State) { current = value; listeners.toList().forEach { it(value) } }
    fun observe(listener: (State) -> Unit) { listeners.add(listener); listener(current) }
    fun removeObserver(listener: (State) -> Unit) { listeners.remove(listener) }
}
