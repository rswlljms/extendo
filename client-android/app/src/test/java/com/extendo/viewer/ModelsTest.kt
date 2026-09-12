package com.extendo.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ModelsTest {
    private val vm = ViewerViewModel()

    @Test
    fun pickBest_prefersLowestRtt() {
        val c = listOf(
            Transport(TransportKind.WIFI, "192.168.1.10", 9577, 40),
            Transport(TransportKind.RNDIS, "192.168.42.129", 9577, 8),
            Transport(TransportKind.LOCALHOST, "127.0.0.1", 9577, 2),
        )
        assertEquals("127.0.0.1", vm.pickBest(c)?.addr)
    }

    @Test
    fun pickBest_tieBreakLocalhostOverRndisOverWifi() {
        val c = listOf(
            Transport(TransportKind.WIFI, "192.168.1.10", 9577, 5),
            Transport(TransportKind.RNDIS, "192.168.42.129", 9577, 5),
            Transport(TransportKind.LOCALHOST, "127.0.0.1", 9577, 5),
        )
        assertEquals(TransportKind.LOCALHOST, vm.pickBest(c)?.kind)
        val noLocal = c.filter { it.kind != TransportKind.LOCALHOST }
        assertEquals(TransportKind.RNDIS, vm.pickBest(noLocal)?.kind)
    }

    @Test
    fun pickBest_unknownRttSortsLast() {
        val c = listOf(
            Transport(TransportKind.WIFI, "192.168.1.10", 9577, -1),
            Transport(TransportKind.WIFI, "192.168.1.11", 9577, 50),
        )
        assertEquals("192.168.1.11", vm.pickBest(c)?.addr)
    }

    @Test
    fun pickBest_emptyIsNull() {
        assertNull(vm.pickBest(emptyList()))
    }

    @Test
    fun isRndisIp_matchesTetheringRanges() {
        assert(Transport.isRndisIp("192.168.42.129"))
        assert(Transport.isRndisIp("192.168.43.1"))
        assert(Transport.isRndisIp("192.168.137.1"))
        assert(!Transport.isRndisIp("192.168.1.10"))
        assert(!Transport.isRndisIp("10.0.0.5"))
    }

    @Test
    fun inputEvent_omitsNulls() {
        val move = InputEvent(type = "touch", x = 0.5f, y = 0.25f, down = null).toJson()
        assertEquals("touch", move.getString("type"))
        assertEquals(0.5, move.getDouble("x"), 1e-6)
        assert(!move.has("down"))
        val up = InputEvent(type = "touch", down = false).toJson()
        assert(!up.has("x"))
        assert(!up.getBoolean("down"))
        val key = InputEvent(type = "key", keyCode = 65, down = true).toJson()
        assertEquals(65, key.getInt("key_code"))
    }
}
