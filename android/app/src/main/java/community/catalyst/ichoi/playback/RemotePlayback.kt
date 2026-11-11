package community.catalyst.ichoi.playback

import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import community.catalyst.csilgen.generated.*
import community.catalyst.ichoi.api.HttpCsilTransport
import community.catalyst.ichoi.storage.ServerProfile
import community.catalyst.ichoi.ui.AppMessages
import kotlin.coroutines.*

/** Shared output state is independent of the library screen. Commands run in order. */
object RemotePlayback {
    private val main = Handler(Looper.getMainLooper())
    private var client: PlayerAsyncClient? = null
    private var profileKey: Pair<String, String?>? = null
    private var generation = 0
    var target: Player? = null
        private set
    var state: PlayerState? = null
        private set
    var error: String? = null
        private set
    private val listeners = mutableSetOf<() -> Unit>()
    private val pending = ArrayDeque<suspend () -> Unit>()
    private var running = false
    private var visible = false
    private var polling = false
    private val poll = Runnable { refresh() }

    fun configure(profile: ServerProfile, token: String?) {
        if (profileKey == (profile.id to token)) return
        generation++; profileKey = profile.id to token
        client = PlayerAsyncClient(HttpCsilTransport(profile, token))
        target = null; state = null; error = null; pending.clear(); changed()
    }
    fun observe(listener: () -> Unit) { listeners.add(listener); listener() }
    fun removeObserver(listener: () -> Unit) { listeners.remove(listener) }
    fun start() { visible = true; refresh() }
    fun stop() { visible = false; main.removeCallbacks(poll) }
    private fun changed() { listeners.toList().forEach { it() } }
    fun list(done: (List<Player>?, Throwable?) -> Unit) {
        val api = client ?: return
        val epoch = generation
        run({ api.listPlayers(ListPlayersRequest(PlayerKind.Shared)).players }) { value, failure -> if (epoch == generation) done(value, failure) }
    }
    fun select(context: Context, player: Player?) {
        generation++; pending.clear(); main.removeCallbacks(poll)
        target = player; state = null; error = null; changed()
        if (player != null) {
            context.startService(Intent(context, PlaybackService::class.java).setAction(PlaybackService.ACTION_PAUSE))
            refresh()
        }
    }
    fun refresh() {
        val api = client ?: return
        val player = target ?: return
        if (polling) return
        polling = true
        val epoch = generation
        run({ api.getState(SubscribeRequest(player.id)) }) { value, failure ->
            polling = false
            if (epoch == generation) {
                if (failure == null) { if (value != null && value.revision >= (state?.revision ?: 0uL)) state = value; error = null } else error = failure.message ?: "Could not reach this device"
                changed()
            }
            main.removeCallbacks(poll)
            if (visible && target != null) main.postDelayed(poll, 2_000)
        }
    }
    fun command(command: PlayerCommand) {
        val api = client ?: return
        val player = target ?: return
        val epoch = generation
        pending.addLast {
            try {
                val updated = api.control(CommandRequest(player.id, command))
                main.post { if (epoch == generation) { if (updated.revision >= (state?.revision ?: 0uL)) state = updated; error = null; changed() } }
            } catch (failure: Throwable) {
                main.post { if (epoch == generation) { error = failure.message; AppMessages.show(failure.message ?: "The device command failed. Try again."); changed() } }
            }
        }
        nextCommand()
    }
    private fun nextCommand() {
        if (running || pending.isEmpty()) return
        running = true
        val action = pending.removeFirst()
        run(action) { _, _ -> running = false; nextCommand() }
    }
    fun enqueue(track: Track) = command(PlayerCommandVariant0(CmdEnqueue("enqueue", listOf(track.id))))
    fun play(track: Track) = command(PlayerCommandVariant8(CmdReplaceAndPlay("replace-and-play", listOf(track.id))))
    fun playItem(item: QueueItem) = command(PlayerCommandVariant7(CmdPlay("play", queueItemId = item.queueItemId)))
    fun remove(item: QueueItem) = command(PlayerCommandVariant3(CmdRemoveItem("remove-item", item.queueItemId)))
    fun move(index: Int, to: Int) {
        val queue = state?.queue ?: return
        if (index !in queue.indices || to !in queue.indices) return
        val item = queue[index]; val remaining = queue.filter { it.queueItemId != item.queueItemId }
        command(PlayerCommandVariant5(CmdMoveItem("move-item", item.queueItemId, remaining.getOrNull(to)?.queueItemId)))
    }
    fun toggle() = command(if (state?.status == PlayerStatus.Playing) PlayerCommandVariant9(CmdPause("pause")) else PlayerCommandVariant7(CmdPlay("play")))
    fun seek(ms: Int) = command(PlayerCommandVariant12(CmdSeek("seek", ms.coerceAtLeast(0).toULong())))
    fun next() = command(PlayerCommandVariant10(CmdNext("next")))
    fun previous() = command(PlayerCommandVariant11(CmdPrevious("previous")))
    fun undo() = command(PlayerCommandVariant16(CmdUndo("undo")))
    private fun <T> run(work: suspend () -> T, done: (T?, Throwable?) -> Unit) {
        work.startCoroutine(object : Continuation<T> {
            override val context = EmptyCoroutineContext
            override fun resumeWith(result: Result<T>) { main.post { done(result.getOrNull(), result.exceptionOrNull()) } }
        })
    }
}
