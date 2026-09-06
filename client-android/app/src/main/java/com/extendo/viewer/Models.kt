package com.extendo.viewer

// Phase-0 models. Field names match proto/extendo.proto JSON form.
enum class TransportKind { WIFI, RNDIS, LOCALHOST }

data class Transport(val kind: TransportKind, val addr: String, val port: Int, val rttMs: Int = -1) {
    companion object {
        fun isRndisIp(ip: String) =
            ip.startsWith("192.168.42.") || ip.startsWith("192.168.43.") || ip.startsWith("192.168.137.")
    }
}

data class VideoConfig(val width: Int = 1280, val height: Int = 720, val fps: Int = 30, val bitrateKbps: Int = 4000, val codec: String = "h264")
data class Stats(val rttMs: Int = -1, val fps: Int = 0, val drops: Int = 0, val bitrateKbps: Int = 0)

// TODO(phase-1): MediaCodec H.264 hardware decode + UDP/RTP; Phase-0 renders /frames SSE like client-web.
class ViewerViewModel {
    fun pickBest(c: List<Transport>): Transport? =
        c.sortedWith(compareBy({ if (it.rttMs < 0) Int.MAX_VALUE else it.rttMs }, { it.kind.ordinal })).firstOrNull()
}
