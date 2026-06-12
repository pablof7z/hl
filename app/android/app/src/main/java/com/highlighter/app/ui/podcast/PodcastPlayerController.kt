package com.highlighter.app.ui.podcast

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uniffi.highlighter_core.ArtifactRecord
import uniffi.highlighter_core.Chapter

/** Speeds offered by the player, matching the iOS speed selector. */
internal val PODCAST_SPEEDS = listOf(0.75f, 1f, 1.25f, 1.5f, 2f)

/**
 * Immutable snapshot of the player surfaced to Compose. A single StateFlow of
 * this keeps recomposition cheap and the UI a pure function of state.
 */
internal data class PodcastPlaybackState(
    val artifactId: String = "",
    val title: String = "",
    val showTitle: String = "",
    val imageUrl: String = "",
    val audioUrl: String = "",
    /** Episode duration metadata (seconds) if the artifact supplied it. */
    val metadataDurationSeconds: Long? = null,
    val chapters: List<Chapter> = emptyList(),
    val positionSeconds: Double = 0.0,
    /** Player-reported duration; 0 until the media is prepared. */
    val durationSeconds: Double = 0.0,
    val isPlaying: Boolean = false,
    val isBuffering: Boolean = false,
    val speed: Float = 1f,
    val errorMessage: String? = null,
) {
    val isLoaded: Boolean get() = artifactId.isNotEmpty()

    /** Best duration we can show: player value, else the metadata fallback. */
    val effectiveDurationSeconds: Double
        get() = when {
            durationSeconds > 0 -> durationSeconds
            metadataDurationSeconds != null && metadataDurationSeconds > 0 ->
                metadataDurationSeconds.toDouble()
            else -> 0.0
        }

    val progressFraction: Float
        get() {
            val d = effectiveDurationSeconds
            if (d <= 0) return 0f
            return (positionSeconds / d).coerceIn(0.0, 1.0).toFloat()
        }
}

/**
 * Thin wrapper around [ExoPlayer] that exposes a [StateFlow] for Compose,
 * polls position ~1Hz while playing, persists/restores per-episode position,
 * and applies sensible audio-focus defaults. Playback is entirely
 * platform-local; the Rust core only supplies the [ArtifactRecord] metadata.
 *
 * One instance is shared app-wide via [rememberPodcastPlayerController] so the
 * mini player (in the scaffold) and any Play affordance (e.g. a room artifact)
 * drive the same engine without routing state through the root composition.
 */
internal class PodcastPlayerController(
    context: Context,
    private val positionStore: PodcastPositionStore,
) {
    private val appContext = context.applicationContext
    private val scope = CoroutineScope(Dispatchers.Main.immediate + Job())

    private val _state = MutableStateFlow(PodcastPlaybackState())
    val state: StateFlow<PodcastPlaybackState> = _state.asStateFlow()

    private var player: ExoPlayer? = null
    private var pollJob: Job? = null

    private val listener = object : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) {
            val buffering = playbackState == Player.STATE_BUFFERING
            val ended = playbackState == Player.STATE_ENDED
            _state.value = _state.value.copy(isBuffering = buffering)
            if (playbackState == Player.STATE_READY) syncDuration()
            if (ended) {
                persistCurrentPosition()
                _state.value = _state.value.copy(isPlaying = false)
                stopPolling()
            }
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            _state.value = _state.value.copy(isPlaying = isPlaying)
            if (isPlaying) startPolling() else { persistCurrentPosition(); stopPolling() }
        }

        override fun onPlayerError(error: PlaybackException) {
            persistCurrentPosition()
            _state.value = _state.value.copy(
                errorMessage = error.localizedMessage ?: "Playback error",
                isPlaying = false,
                isBuffering = false,
            )
            stopPolling()
        }
    }

    /**
     * Load [artifact] and begin playback. If the same episode is already
     * loaded this simply resumes. Restores the saved position (when recent)
     * before playing, matching iOS.
     */
    fun load(artifact: ArtifactRecord) {
        val url = selectAudioUrl(artifact.preview.audioUrl, artifact.preview.audioPreviewUrl) ?: run {
            _state.value = _state.value.copy(errorMessage = "This episode has no audio to play")
            return
        }

        if (_state.value.artifactId == artifact.shareEventId && player != null) {
            play()
            return
        }

        val engine = ensurePlayer()
        val resumeAt = positionStore.lastPosition(artifact.shareEventId)

        _state.value = PodcastPlaybackState(
            artifactId = artifact.shareEventId,
            title = artifact.preview.title.ifBlank { "Untitled episode" },
            showTitle = artifact.preview.podcastShowTitle.ifBlank { artifact.preview.author },
            imageUrl = artifact.preview.image,
            audioUrl = url,
            metadataDurationSeconds = artifact.preview.durationSeconds,
            chapters = artifact.preview.chapters,
            positionSeconds = resumeAt ?: 0.0,
            speed = _state.value.speed,
        )

        val mediaItem = MediaItem.Builder()
            .setUri(url)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(artifact.preview.title.ifBlank { "Untitled episode" })
                    .setArtist(artifact.preview.podcastShowTitle.ifBlank { artifact.preview.author })
                    .build(),
            )
            .build()
        engine.setMediaItem(mediaItem)
        engine.setPlaybackSpeed(_state.value.speed)
        engine.prepare()
        if (resumeAt != null && resumeAt > 0) {
            engine.seekTo((resumeAt * 1000).toLong())
        }
        engine.playWhenReady = true
    }

    fun play() {
        player?.playWhenReady = true
    }

    fun pause() {
        player?.playWhenReady = false
    }

    fun toggle() {
        if (_state.value.isPlaying) pause() else play()
    }

    /** Seek to an absolute position (seconds), clamped to [0, duration]. */
    fun seekTo(seconds: Double) {
        val engine = player ?: return
        val duration = _state.value.effectiveDurationSeconds
        val clamped = if (duration > 0) seconds.coerceIn(0.0, duration) else seconds.coerceAtLeast(0.0)
        engine.seekTo((clamped * 1000).toLong())
        _state.value = _state.value.copy(positionSeconds = clamped)
        persistCurrentPosition()
    }

    /** Relative skip (e.g. ±15s). */
    fun skip(deltaSeconds: Double) {
        seekTo(_state.value.positionSeconds + deltaSeconds)
    }

    fun setSpeed(speed: Float) {
        player?.setPlaybackSpeed(speed)
        _state.value = _state.value.copy(speed = speed)
    }

    /** Stop playback, drop the episode, and release the engine. */
    fun clear() {
        persistCurrentPosition()
        stopPolling()
        player?.let {
            it.removeListener(listener)
            it.release()
        }
        player = null
        _state.value = PodcastPlaybackState(speed = _state.value.speed)
    }

    /** Release everything; call when the owning scope is torn down. */
    fun release() {
        clear()
        scope.coroutineContext[Job]?.cancel()
    }

    private fun ensurePlayer(): ExoPlayer {
        player?.let { return it }
        val engine = ExoPlayer.Builder(appContext)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_SPEECH)
                    .build(),
                /* handleAudioFocus = */ true,
            )
            .setHandleAudioBecomingNoisy(true)
            .build()
        engine.addListener(listener)
        player = engine
        return engine
    }

    private fun startPolling() {
        if (pollJob?.isActive == true) return
        pollJob = scope.launch {
            while (isActive) {
                syncPosition()
                delay(1000)
            }
        }
    }

    private fun stopPolling() {
        pollJob?.cancel()
        pollJob = null
    }

    private fun syncPosition() {
        val engine = player ?: return
        val pos = (engine.currentPosition.coerceAtLeast(0L)) / 1000.0
        _state.value = _state.value.copy(positionSeconds = pos)
        persistCurrentPosition()
    }

    private fun syncDuration() {
        val engine = player ?: return
        val durationMs = engine.duration
        if (durationMs != C.TIME_UNSET && durationMs > 0) {
            _state.value = _state.value.copy(durationSeconds = durationMs / 1000.0)
        }
    }

    private fun persistCurrentPosition() {
        val s = _state.value
        if (s.artifactId.isNotEmpty() && s.positionSeconds > 0) {
            positionStore.save(s.artifactId, s.positionSeconds)
        }
    }
}

/**
 * App-scoped singleton holder. RootScene/MainActivity are out of this feature's
 * editing scope, so rather than thread the controller through the root
 * composition we keep one process-lifetime instance keyed by the application
 * context. Both the scaffold's mini player and the room artifact Play button
 * resolve the same controller through this.
 */
private object PodcastPlayerHolder {
    @Volatile
    private var instance: PodcastPlayerController? = null

    fun get(context: Context): PodcastPlayerController {
        instance?.let { return it }
        return synchronized(this) {
            instance ?: PodcastPlayerController(
                context = context.applicationContext,
                positionStore = PodcastPositionStore(
                    SharedPrefsBackingStore(context.applicationContext),
                ),
            ).also { instance = it }
        }
    }
}

/** SharedPreferences-backed [PositionBackingStore]. */
private class SharedPrefsBackingStore(context: Context) : PositionBackingStore {
    private val prefs = context.getSharedPreferences("highlighter.podcast", Context.MODE_PRIVATE)
    override fun getString(key: String): String? = prefs.getString(key, null)
    override fun putString(key: String, value: String) {
        prefs.edit().putString(key, value).apply()
    }
}

/** Returns the process-wide shared [PodcastPlayerController]. */
@Composable
internal fun rememberPodcastPlayerController(): PodcastPlayerController {
    val context = androidx.compose.ui.platform.LocalContext.current
    return PodcastPlayerHolder.get(context)
}
