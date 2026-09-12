package com.extendo.viewer

// HTTP client for the host contract (proto/PROTOCOL.md). Pure java.net +
// org.json: no Android imports, so parsing is JVM-unit-testable. Streaming
// (/video.mjpg) lives in MjpegParser; bitmap decode stays in the repository.

import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

data class PairResult(val accepted: Boolean, val reason: String, val video: VideoConfig)

data class HostInfo(
    val transports: List<Transport>,
    val video: VideoConfig,
    val tier: String,
    val coreOk: Boolean,
)

data class QualityResult(val ok: Boolean, val reason: String, val status: Int)

data class InputSend(val ok: Boolean, val status: Int)

class ExtendoApi(private var base: String, private val token: String) {

    fun setBase(url: String) {
        base = url.trimEnd('/')
    }

    fun base(): String = base

    /** Token for query strings / headers (URL-encoded for ?token= use). */
    fun tokenForUrl(): String = java.net.URLEncoder.encode(token, "UTF-8")

    fun rawToken(): String = token

    private fun open(path: String, method: String, authed: Boolean): HttpURLConnection {
        val sep = if (path.contains("?")) "&" else "?"
        val target = if (authed) "$base$path${sep}token=${tokenForUrl()}" else "$base$path"
        val c = URL(target).openConnection() as HttpURLConnection
        c.requestMethod = method
        c.connectTimeout = 5000
        c.readTimeout = 15000
        if (authed) c.setRequestProperty("x-extendo-token", token)
        return c
    }

    private fun read(c: HttpURLConnection): Pair<Int, String> {
        val code = c.responseCode
        val stream = if (code in 200..299) c.inputStream else c.errorStream
        val body = stream?.bufferedReader()?.readText().orEmpty()
        c.disconnect()
        return code to body
    }

    fun postJson(path: String, body: JSONObject, authed: Boolean = true): Pair<Int, String> {
        val c = open(path, "POST", authed)
        c.doOutput = true
        c.setRequestProperty("Content-Type", "application/json")
        c.outputStream.use { it.write(body.toString().toByteArray()) }
        return read(c)
    }

    fun getJson(path: String, authed: Boolean = false): Pair<Int, String> {
        val c = open(path, "GET", authed)
        return read(c)
    }

    /** Pair with the host. 429 = throttled (too many bad PINs), 403 = bad token. */
    fun pair(deviceName: String = "android-viewer"): PairResult {
        val (code, body) = postJson(
            "/pair",
            JSONObject().put("device_name", deviceName).put("token", token),
            authed = false,
        )
        if (code != 200) return PairResult(false, "http $code: ${body.take(120)}", VideoConfig())
        return parsePair(body)
    }

    fun info(): HostInfo? {
        val (code, body) = getJson("/info")
        if (code != 200) return null
        return parseInfo(body)
    }

    /** RTT probe, mirrors the web viewer's /ping loop. Returns -1 on failure. */
    fun ping(): Long {
        return try {
            val t0 = System.currentTimeMillis()
            val (code, _) = getJson("/ping?t0=$t0")
            if (code == 200) System.currentTimeMillis() - t0 else -1
        } catch (_: Exception) {
            -1
        }
    }

    fun best(): Pair<Transport?, List<Transport>> {
        val (code, body) = getJson("/best")
        if (code != 200) return null to emptyList()
        return parseBest(body)
    }

    /** Viewer heartbeat feeding the host adaptive loop. Returns the level label. */
    fun report(fps: Int, drops: Int): String {
        return try {
            val (code, body) = postJson("/report", JSONObject().put("fps", fps).put("drops", drops))
            if (code == 200) JSONObject(body).optString("adaptive", "") else ""
        } catch (_: Exception) {
            ""
        }
    }

    fun setQuality(width: Int, height: Int, fps: Int): QualityResult {
        val (code, body) = postJson(
            "/quality",
            JSONObject().put("width", width).put("height", height).put("fps", fps),
        )
        if (code == 200) return QualityResult(true, "", code)
        val reason = try {
            JSONObject(body).optString("reason", "http $code")
        } catch (_: Exception) {
            "http $code"
        }
        return QualityResult(false, reason, code)
    }

    fun sendInput(ev: JSONObject): InputSend {
        return try {
            val (code, _) = postJson("/input", ev)
            InputSend(code == 200, code)
        } catch (_: Exception) {
            InputSend(false, -1)
        }
    }

    companion object {
        fun parseTransport(o: JSONObject): Transport {
            val kind = when (o.optString("kind")) {
                "rndis" -> TransportKind.RNDIS
                "localhost" -> TransportKind.LOCALHOST
                else -> TransportKind.WIFI
            }
            return Transport(kind, o.optString("addr"), o.optInt("port", 9577), o.optInt("rttMs", o.optInt("rtt_ms", -1)))
        }

        fun parseVideo(o: JSONObject): VideoConfig = VideoConfig(
            width = o.optInt("width", 1280),
            height = o.optInt("height", 720),
            fps = o.optInt("fps", 30),
            bitrateKbps = o.optInt("bitrate_kbps", 4000),
            codec = o.optString("codec", "h264").ifEmpty { "h264" },
        )

        fun parsePair(body: String): PairResult {
            val o = JSONObject(body)
            return PairResult(
                accepted = o.optBoolean("accepted", false),
                reason = o.optString("reason", ""),
                video = if (o.isNull("video")) VideoConfig() else parseVideo(o.getJSONObject("video")),
            )
        }

        fun parseInfo(body: String): HostInfo {
            val o = JSONObject(body)
            val transports = mutableListOf<Transport>()
            val arr = o.optJSONArray("transports")
            if (arr != null) for (i in 0 until arr.length()) transports += parseTransport(arr.getJSONObject(i))
            val core = o.optJSONObject("core")
            return HostInfo(
                transports = transports,
                video = if (o.isNull("video")) VideoConfig() else parseVideo(o.getJSONObject("video")),
                tier = o.optString("tier", "free"),
                coreOk = core?.optBoolean("ok", false) == true,
            )
        }

        fun parseBest(body: String): Pair<Transport?, List<Transport>> {
            val o = JSONObject(body)
            val all = mutableListOf<Transport>()
            val arr = o.optJSONArray("probed")
            if (arr != null) for (i in 0 until arr.length()) all += parseTransport(arr.getJSONObject(i))
            val best = if (o.isNull("best")) null else parseTransport(o.getJSONObject("best"))
            return best to all
        }
    }
}
