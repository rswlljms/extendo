package com.extendo.viewer

// Single source of truth for connection + stream state. Owned coroutines run
// here; StreamService keeps the process alive (Doze-proof foreground service)
// and the activity only observes. Bitmap decode stays here so MjpegParser and
// ExtendoApi keep zero Android imports (unit-testable on the JVM).

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.net.HttpURLConnection
import java.net.URL

object StreamRepository {
    val state: StateFlow<ConnState> get() = _state.asStateFlow()
    val bitmap: StateFlow<Bitmap?> get() = _bitmap.asStateFlow()
    val overlay: StateFlow<String> get() = _overlay.asStateFlow()
    val transports: StateFlow<List<Transport>> get() = _transports.asStateFlow()
    val best: StateFlow<Transport?> get() = _best.asStateFlow()
    val adaptive: StateFlow<String> get() = _adaptive.asStateFlow()
    val rotated: StateFlow<Boolean> get() = _rotated.asStateFlow()

    private val _state = MutableStateFlow<ConnState>(ConnState.Disconnected)
    private val _bitmap = MutableStateFlow<Bitmap?>(null)
    private val _overlay = MutableStateFlow("")
    private val _transports = MutableStateFlow<List<Transport>>(emptyList())
    private val _best = MutableStateFlow<Transport?>(null)
    private val _adaptive = MutableStateFlow("")
    private val _rotated = MutableStateFlow(false)

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var jobs: List<Job> = emptyList()
    private var api: ExtendoApi? = null
    private var sessionKey = ""

    private var framesThisSecond = 0
    private var fps = 0
    private var drops = 0
    private var rttMs = -1L
    private var lastFpsReset = System.currentTimeMillis()

    /** Idempotent: reconnects only when host/token changed or not connected. */
    @Synchronized
    fun connect(host: String, token: String) {
        val key = "$host|$token"
        if (key == sessionKey && _state.value !is ConnState.Disconnected && _state.value !is ConnState.Error) return
        disconnectLocked()
        sessionKey = key
        val cleanHost = host.trimEnd('/')
        api = ExtendoApi(cleanHost, token)
        _state.value = ConnState.Connecting
        _bitmap.value = null
        jobs = listOf(
            scope.launch { pairAndStream() },
            scope.launch { pingLoop() },
            scope.launch { reportLoop() },
        )
    }

    @Synchronized
    fun disconnect() {
        disconnectLocked()
        sessionKey = ""
        _state.value = ConnState.Disconnected
    }

    private fun disconnectLocked() {
        jobs.forEach { it.cancel() }
        jobs = emptyList()
        api = null
    }

    fun setRotated(v: Boolean) {
        _rotated.value = v
    }

    /** Wi-Fi ↔ USB switch without re-pairing: token stays, base URL changes. */
    fun switchTransport(t: Transport) {
        val a = api ?: return
        a.setBase(t.baseUrl())
        _best.value = t
        restartStream()
    }

    fun setQuality(width: Int, height: Int, fpsAsk: Int, onResult: (String) -> Unit) {
        scope.launch {
            val a = api ?: return@launch
            val r = try {
                a.setQuality(width, height, fpsAsk)
            } catch (e: Exception) {
                QualityResult(false, e.message ?: "error", -1)
            }
            if (r.ok) {
                val cur = (_state.value as? ConnState.Connected)?.video ?: VideoConfig()
                _state.value = ConnState.Connected(cur.copy(width = width, height = height, fps = fpsAsk))
                restartStream()
                onResult("quality ${width}x${height}@${fpsAsk}")
            } else {
                onResult(if (r.status == 402) "1080p60 requires Pro (free capped at 720p30)" else "quality failed: ${r.reason}")
            }
        }
    }

    fun sendTouch(x: Float?, y: Float?, down: Boolean?) {
        val a = api ?: return
        scope.launch {
            val ev = InputEvent(
                type = "touch",
                x = x?.coerceIn(0f, 1f),
                y = y?.coerceIn(0f, 1f),
                down = down,
            )
            val r = a.sendInput(ev.toJson())
            if (r.status == 402) _overlay.value = "touch/keyboard input requires Pro"
        }
    }

    fun sendKey(keyCode: Int, down: Boolean) {
        // NOTE: Compose keyCode == Android keyCode here; mapping Android keys
        // to Windows VK codes is milestone-2 work (host takes VK today).
        val a = api ?: return
        scope.launch {
            val r = a.sendInput(InputEvent(type = "key", keyCode = keyCode, down = down).toJson())
            if (r.status == 402) _overlay.value = "touch/keyboard input requires Pro"
        }
    }

    private fun restartStream() {
        jobs.getOrNull(0)?.cancel()
        val pairJob = scope.launch { streamOnly() }
        jobs = listOf(pairJob) + jobs.drop(1)
    }

    private suspend fun pairAndStream() {
        val a = api ?: return
        try {
            val pair = a.pair()
            if (!pair.accepted) {
                _state.value = ConnState.Error("pair failed: ${pair.reason.ifEmpty { "bad token/PIN" }}")
                return
            }
            _state.value = ConnState.Connected(pair.video)
            refreshTransports(a)
            streamOnly()
        } catch (e: Exception) {
            _state.value = ConnState.Error("connect failed: ${e.message}")
        }
    }

    private suspend fun streamOnly() {
        val a = api ?: return
        while (scope.coroutineContext.isActive) {
            try {
                val url = URL("${a.base()}/video.mjpg?token=${a.tokenForUrl()}")
                val c = url.openConnection() as HttpURLConnection
                c.connectTimeout = 5000
                c.readTimeout = 0 // infinite stream
                c.setRequestProperty("x-extendo-token", a.tokenForUrl())
                c.connect()
                if (c.responseCode != 200) {
                    _overlay.value = "stream http ${c.responseCode}, retrying…"
                    c.disconnect()
                    delay(1500)
                    continue
                }
                val ct = c.contentType
                val boundary = MjpegParser.boundaryFromContentType(ct) ?: "extendoframe"
                val parser = MjpegParser(boundary)
                parser.readFrames(c.inputStream, { scope.coroutineContext.isActive }) { bytes ->
                    val bmp = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                    if (bmp != null) {
                        _bitmap.value?.recycle()
                        _bitmap.value = bmp
                        onFrame()
                    } else {
                        drops++
                    }
                }
                c.disconnect()
            } catch (e: Exception) {
                if (!scope.coroutineContext.isActive) return
                drops++
                _overlay.value = "reconnecting… (${e.message?.take(60)})"
            }
            delay(1000)
        }
    }

    private suspend fun refreshTransports(a: ExtendoApi) {
        try {
            val (b, all) = a.best()
            if (all.isNotEmpty()) {
                _transports.value = all
                _best.value = b
            }
        } catch (_: Exception) {
        }
    }

    private suspend fun pingLoop() {
        while (scope.coroutineContext.isActive) {
            val a = api
            if (a != null) {
                rttMs = a.ping()
                if (rttMs >= 0) refreshTransportsThrottled(a)
            }
            delay(2000)
        }
    }

    private var lastTransportRefresh = 0L

    private suspend fun refreshTransportsThrottled(a: ExtendoApi) {
        if (System.currentTimeMillis() - lastTransportRefresh > 10_000) {
            lastTransportRefresh = System.currentTimeMillis()
            refreshTransports(a)
        }
    }

    private suspend fun reportLoop() {
        while (scope.coroutineContext.isActive) {
            delay(5000)
            val a = api ?: continue
            _adaptive.value = a.report(fps, drops)
        }
    }

    private fun onFrame() {
        framesThisSecond++
        val now = System.currentTimeMillis()
        if (now - lastFpsReset >= 1000) {
            fps = framesThisSecond
            framesThisSecond = 0
            lastFpsReset = now
        }
        val video = (_state.value as? ConnState.Connected)?.video
        val w = video?.width ?: 0
        val h = video?.height ?: 0
        val dims = if (_rotated.value && w > 0) "${h}x${w}" else "${w}x${h}"
        val auto = _adaptive.value
        _overlay.value = "rtt ${rttMs}ms · fps $fps · drops $drops · ${dims} mjpeg" +
            (if (auto.isNotEmpty()) " · auto:$auto" else "")
    }
}
