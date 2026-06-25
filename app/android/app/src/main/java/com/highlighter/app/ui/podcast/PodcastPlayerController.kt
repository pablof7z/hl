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
import uniffi.highlighter_core.AudioOp
import uniffi.highlighter_core.AudioResult
import uniffi.highlighter_core.ArtifactRecord
import uniffi.highlighter_core.CapabilityRequest
import uniffi.highlighter_core.CapabilityResult
import uniffi.highlighter_core.Chapter
import uniffi.highlighter_core.HighlighterAppAction

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
 * polls position ~1Hz while playing, and applies sensible audio-focus defaults.
 * Android only executes media primitives; durable playback state is reported
 * to the Rust/NMP kernel through audio actions.
 *
 * One instance is shared app-wide via [rememberPodcastPlayerController] so the
 * mini player (in the scaffold) and any Play affordance (e.g. a room artifact)
 * drive the same engine without routing state through the root composition.
 */
internal class PodcastPlayerController(
    context: Context,
    private var dispatch: (HighlighterAppAction) -> Unit,
) {
    private val appContext = context.applicationContext
    private val scope = CoroutineScope(Dispatchers.Main.immediate + Job())

    private val _state = MutableStateFlow(PodcastPlaybackState())
    val state: StateFlow<PodcastPlaybackState> = _state.asStateFlow()

    private var player: ExoPlayer? = null
    private var pollJob: Job? = null
    private var lastReportedPositionBucket: Long? = null
    private var capabilityResult: ((CapabilityResult) -> Unit)? = null

    private val listener = object : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) {
            val buffering = playbackState == Player.STATE_BUFFERING
            val ended = playbackState == Player.STATE_ENDED
            _state.value = _state.value.copy(isBuffering = buffering)
            if (playbackState == Player.STATE_READY) syncDuration()
            if (ended) {
                reportProgress(force = true)
                capabilityResult?.invoke(CapabilityResult.Audio(AudioResult.Ended))
                _state.value = _state.value.copy(isPlaying = false)
                stopPolling()
            }
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            _state.value = _state.value.copy(isPlaying = isPlaying)
            if (isPlaying) {
                startPolling()
            } else {
                reportProgress(force = true)
                stopPolling()
            }
        }

        override fun onPlayerError(error: PlaybackException) {
            val message = error.localizedMessage ?: "Playback error"
            reportProgress(force = true)
            capabilityResult?.invoke(CapabilityResult.Audio(AudioResult.Error(message)))
            _state.value = _state.value.copy(
                errorMessage = message,
                isPlaying = false,
                isBuffering = false,
            )
            stopPolling()
        }
    }

    fun updateDispatch(dispatch: (HighlighterAppAction) -> Unit) {
        this.dispatch = dispatch
    }

    fun handleAudioOp(
        op: AudioOp,
        provideResult: (CapabilityResult) -> Unit,
    ) {
        capabilityResult = provideResult
        scope.launch {
            when (op) {
                is AudioOp.Load -> loadFromKernel(op.url, op.resumeAtSeconds)
                AudioOp.Play -> player?.playWhenReady = true
                AudioOp.Pause -> player?.playWhenReady = false
                is AudioOp.Seek -> seekEngineTo(op.seconds, forceReport = true)
                AudioOp.Stop -> stopFromKernel()
                is AudioOp.ExtractWaveform -> {
                    provideResult(
                        CapabilityResult.Audio(
                            AudioResult.WaveformPeaks(op.url, emptyList()),
                        ),
                    )
                }
            }
        }
    }

    /**
     * Load [artifact] and begin playback. If the same episode is already loaded
     * this simply resumes. Durable resume position is owned by Rust/NMP; Android
     * does not read or write a native position cache.
     */
    fun load(artifact: ArtifactRecord) {
        val url = selectAudioUrl(artifact.preview.audioUrl, artifact.preview.audioPreviewUrl) ?: run {
            _state.value = _state.value.copy(errorMessage = "This episode has no audio to play")
            return
        }
        val guid = podcastGuid(artifact)

        if (_state.value.artifactId == artifact.shareEventId && player != null) {
            play()
            return
        }

        lastReportedPositionBucket = null
        dispatch(HighlighterAppAction.AudioPlay(url = url, guid = guid, artifact = artifact))

        _state.value = PodcastPlaybackState(
            artifactId = artifact.shareEventId,
            title = artifact.preview.title.ifBlank { "Untitled episode" },
            showTitle = artifact.preview.podcastShowTitle.ifBlank { artifact.preview.author },
            imageUrl = artifact.preview.image,
            audioUrl = url,
            metadataDurationSeconds = artifact.preview.durationSeconds,
            chapters = artifact.preview.chapters,
            positionSeconds = 0.0,
            speed = _state.value.speed,
        )
    }

    fun play() {
        dispatch(HighlighterAppAction.AudioResume)
    }

    fun pause() {
        dispatch(HighlighterAppAction.AudioPause)
    }

    fun toggle() {
        if (_state.value.isPlaying) pause() else play()
    }

    /** Seek to an absolute position (seconds), clamped to [0, duration]. */
    fun seekTo(seconds: Double) {
        val duration = _state.value.effectiveDurationSeconds
        val clamped = if (duration > 0) seconds.coerceIn(0.0, duration) else seconds.coerceAtLeast(0.0)
        _state.value = _state.value.copy(positionSeconds = clamped)
        dispatch(HighlighterAppAction.AudioSeek(clamped))
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
        dispatch(HighlighterAppAction.AudioPause)
        stopFromKernel()
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

    private fun loadFromKernel(url: String, resumeAtSeconds: Double?) {
        val engine = ensurePlayer()
        val mediaItem = MediaItem.Builder()
            .setUri(url)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(_state.value.title.ifBlank { "Untitled episode" })
                    .setArtist(_state.value.showTitle)
                    .build(),
            )
            .build()
        engine.setMediaItem(mediaItem)
        engine.setPlaybackSpeed(_state.value.speed)
        engine.prepare()
        val resumeAt = resumeAtSeconds?.takeIf { it.isFinite() && it > 0.0 }
        if (resumeAt != null) {
            seekEngineTo(resumeAt, forceReport = false)
            _state.value = _state.value.copy(positionSeconds = resumeAt)
        }
        engine.playWhenReady = true
    }

    private fun seekEngineTo(seconds: Double, forceReport: Boolean) {
        val engine = player ?: return
        val clamped = seconds.coerceAtLeast(0.0)
        engine.seekTo((clamped * 1000).toLong())
        _state.value = _state.value.copy(positionSeconds = clamped)
        reportProgress(force = forceReport)
    }

    private fun stopFromKernel() {
        reportProgress(force = true)
        stopPolling()
        player?.let {
            it.removeListener(listener)
            it.release()
        }
        player = null
        capabilityResult = null
        _state.value = PodcastPlaybackState(speed = _state.value.speed)
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
        reportProgress()
    }

    private fun syncDuration() {
        val engine = player ?: return
        val durationMs = engine.duration
        if (durationMs != C.TIME_UNSET && durationMs > 0) {
            val durationSeconds = durationMs / 1000.0
            _state.value = _state.value.copy(durationSeconds = durationSeconds)
            capabilityResult?.invoke(
                CapabilityResult.Audio(AudioResult.Loaded(durationSeconds)),
            )
        }
    }

    private fun reportProgress(force: Boolean = false) {
        val s = _state.value
        if (s.artifactId.isNotEmpty() && s.positionSeconds > 0) {
            val bucket = (s.positionSeconds / POSITION_REPORT_INTERVAL_SECONDS).toLong()
            if (!force && bucket == lastReportedPositionBucket) return
            lastReportedPositionBucket = bucket
            capabilityResult?.invoke(
                CapabilityResult.Audio(
                    AudioResult.Progress(
                        currentSeconds = s.positionSeconds,
                        isPlaying = s.isPlaying,
                    ),
                ),
            )
        }
    }

    private fun podcastGuid(artifact: ArtifactRecord): String =
        artifact.preview.podcastItemGuid.ifBlank { artifact.shareEventId }

    private companion object {
        const val POSITION_REPORT_INTERVAL_SECONDS = 5.0
    }
}

/**
 * App-scoped singleton holder. RootScene/MainActivity are out of this feature's
 * editing scope, so rather than thread the controller through the root
 * composition we keep one process-lifetime instance keyed by the application
 * context. Both the scaffold's mini player and the room artifact Play button
 * resolve the same controller through this.
 */
internal object PodcastPlayerHolder {
    @Volatile
    private var instance: PodcastPlayerController? = null

    fun get(context: Context, dispatch: (HighlighterAppAction) -> Unit): PodcastPlayerController {
        instance?.let {
            it.updateDispatch(dispatch)
            return it
        }
        return synchronized(this) {
            instance ?: PodcastPlayerController(
                context = context.applicationContext,
                dispatch = dispatch,
            ).also { instance = it }
        }
    }

    fun handleCapabilityRequest(
        request: CapabilityRequest,
        provideResult: (CapabilityResult) -> Unit,
    ): Boolean {
        val op = (request as? CapabilityRequest.Audio)?.v1 ?: return false
        instance?.handleAudioOp(op, provideResult) ?: provideResult(
            CapabilityResult.Audio(AudioResult.Error("Podcast player is not available")),
        )
        return true
    }
}

/** Returns the process-wide shared [PodcastPlayerController]. */
@Composable
internal fun rememberPodcastPlayerController(
    dispatch: (HighlighterAppAction) -> Unit,
): PodcastPlayerController {
    val context = androidx.compose.ui.platform.LocalContext.current
    return PodcastPlayerHolder.get(context, dispatch)
}
