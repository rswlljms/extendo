package com.extendo.viewer

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ApiParseTest {

    @Test
    fun parseTransport_acceptsCamelAndSnakeRtt() {
        val camel = ExtendoApi.parseTransport(
            JSONObject("""{"kind":"rndis","addr":"192.168.42.129","port":9577,"rttMs":8}""")
        )
        assertEquals(TransportKind.RNDIS, camel.kind)
        assertEquals(8, camel.rttMs)
        assertEquals("http://192.168.42.129:9577", camel.baseUrl())

        val snake = ExtendoApi.parseTransport(
            JSONObject("""{"kind":"wifi","addr":"192.168.1.10","port":9577,"rtt_ms":12}""")
        )
        assertEquals(TransportKind.WIFI, snake.kind)
        assertEquals(12, snake.rttMs)

        val unknownKind = ExtendoApi.parseTransport(JSONObject("""{"addr":"10.0.0.5","port":9577}"""))
        assertEquals(TransportKind.WIFI, unknownKind.kind)
        assertEquals(-1, unknownKind.rttMs)
    }

    @Test
    fun parseVideo_defaults() {
        val v = ExtendoApi.parseVideo(
            JSONObject("""{"width":1920,"height":1080,"fps":60,"bitrate_kbps":8000,"codec":"h264"}""")
        )
        assertEquals(VideoConfig(1920, 1080, 60, 8000, "h264"), v)
        assertEquals("1920x1080@60", v.label())
        val empty = ExtendoApi.parseVideo(JSONObject("{}"))
        assertEquals(VideoConfig(), empty)
    }

    @Test
    fun parsePair_okAndRejected() {
        val ok = ExtendoApi.parsePair(
            """{"accepted":true,"reason":"","video":{"width":1280,"height":720,"fps":30,"bitrate_kbps":4000,"codec":"h264"}}"""
        )
        assertTrue(ok.accepted)
        assertEquals(1280, ok.video.width)
        val bad = ExtendoApi.parsePair("""{"accepted":false,"reason":"bad token/PIN","video":null}""")
        assertFalse(bad.accepted)
        assertEquals("bad token/PIN", bad.reason)
    }

    @Test
    fun parseInfo_readsTierAndCore() {
        val info = ExtendoApi.parseInfo(
            """{"transports":[{"kind":"localhost","addr":"127.0.0.1","port":9577,"rttMs":-1}],
               "video":{"width":1280,"height":720,"fps":30,"bitrate_kbps":4000,"codec":"h264"},
               "tier":"pro","core":{"ok":true}}"""
        )
        assertEquals(1, info.transports.size)
        assertEquals("127.0.0.1", info.transports[0].addr)
        assertEquals("pro", info.tier)
        assertTrue(info.coreOk)
    }

    @Test
    fun parseBest_readsBestAndProbed() {
        val (best, all) = ExtendoApi.parseBest(
            """{"best":{"kind":"localhost","addr":"127.0.0.1","port":9577,"rttMs":1},
               "probed":[{"kind":"localhost","addr":"127.0.0.1","port":9577,"rttMs":1},
                         {"kind":"wifi","addr":"192.168.1.10","port":9577,"rttMs":20}]}"""
        )
        assertEquals("127.0.0.1", best?.addr)
        assertEquals(2, all.size)
    }
}
