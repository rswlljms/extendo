package com.extendo.viewer

// Phase-0/1 models. Field names match proto/extendo.proto JSON form
// (see ExtendoApi.parseTransport / parseVideo for the snake_case mapping).
enum class TransportKind { WIFI, RNDIS, LOCALHOST }

data class Transport(val kind: TransportKind, val addr: String, val port: Int, val rttMs: Int = -1) {
    companion object {
        fun isRndisIp(ip: String) =
            ip.startsWith("192.168.42.") || ip.startsWith("192.168.43.") || ip.startsWith("192.168.137.")
    }

    fun baseUrl() = "http://$addr:$port"
    fun label() = "$kind $addr ${if (rttMs >= 0) "${rttMs}ms" else "?"}"
}

data class VideoConfig(val width: Int = 1280, val height: Int = 720, val fps: Int = 30, val bitrateKbps: Int = 4000, val codec: String = "h264") {
    fun label() = "${width}x${height}@${fps}"
}

data class Stats(val rttMs: Int = -1, val fps: Int = 0, val drops: Int = 0, val bitrateKbps: Int = 0)

/** Touch/keyboard event sent to POST /input (host SendInput bridge). */
data class InputEvent(
    val type: String,
    val x: Float? = null,
    val y: Float? = null,
    val keyCode: Int? = null,
    val down: Boolean? = null,
) {
    fun toJson(): org.json.JSONObject {
        val o = org.json.JSONObject().put("type", type)
        if (x != null) o.put("x", x.toDouble())
        if (y != null) o.put("y", y.toDouble())
        if (keyCode != null) o.put("key_code", keyCode)
        if (down != null) o.put("down", down)
        return o
    }
}

sealed interface ConnState {
    data object Disconnected : ConnState
    data object Connecting : ConnState
    data class Connected(val video: VideoConfig) : ConnState
    data class Error(val message: String) : ConnState
}

// TODO(phase-2): MediaCodec H.264 hardware decode + UDP/RTP; milestone 1
// renders /video.mjpg like client-web (same contract, no protobuf needed).
class ViewerViewModel {
    /** Lowest RTT wins; ties break localhost > rndis > wifi (PROTOCOL.md). */
    fun pickBest(c: List<Transport>): Transport? =
        c.sortedWith(compareBy({ if (it.rttMs < 0) Int.MAX_VALUE else it.rttMs }, ::rank)).firstOrNull()

    companion object {
        fun rank(t: Transport): Int = when (t.kind) {
            TransportKind.LOCALHOST -> 0
            TransportKind.RNDIS -> 1
            TransportKind.WIFI -> 2
        }
    }
}
