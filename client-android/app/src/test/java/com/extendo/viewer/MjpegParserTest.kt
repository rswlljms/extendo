package com.extendo.viewer

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.io.ByteArrayInputStream

class MjpegParserTest {

    private fun part(body: ByteArray, withLength: Boolean = true): ByteArray {
        val head = if (withLength) {
            "Content-Type: image/jpeg\r\nContent-Length: ${body.size}\r\n\r\n"
        } else {
            "Content-Type: image/jpeg\r\n\r\n"
        }
        return head.toByteArray() + body
    }

    private fun stream(vararg bodies: ByteArray, withLength: Boolean = true): ByteArray {
        var raw = "--extendoframe\r\n".toByteArray().toList()
        for (b in bodies) {
            raw = raw + part(b, withLength).toList() + "\r\n--extendoframe\r\n".toByteArray().toList()
        }
        return raw.toByteArray()
    }

    @Test
    fun readsLengthDelimitedFramesExactly() {
        val f1 = byteArrayOf(0xFF.toByte(), 0xD8.toByte(), 1, 2, 3, 0x0A, 0xFF.toByte(), 0xD9.toByte())
        val f2 = byteArrayOf(0xFF.toByte(), 0xD8.toByte(), 9, 8, 7)
        val got = mutableListOf<ByteArray>()
        MjpegParser("extendoframe").readFrames(ByteArrayInputStream(stream(f1, f2)), { true }, got::add)
        assertEquals(2, got.size)
        assertArrayEquals(f1, got[0])
        assertArrayEquals(f2, got[1])
    }

    @Test
    fun stopsAtClosingBoundary() {
        val f1 = byteArrayOf(1, 2, 3)
        val raw = "--extendoframe\r\n".toByteArray() + part(f1) + "\r\n--extendoframe--\r\n".toByteArray()
        val got = mutableListOf<ByteArray>()
        MjpegParser("extendoframe").readFrames(ByteArrayInputStream(raw), { true }, got::add)
        assertEquals(1, got.size)
        assertArrayEquals(f1, got[0])
    }

    @Test
    fun fallsBackWhenContentLengthMissing() {
        // Binary bodies containing 0x0A must NOT split the frame on the slow path.
        val f1 = byteArrayOf(0xFF.toByte(), 0xD8.toByte(), 0x0A, 0x0D, 0x00, 5)
        val got = mutableListOf<ByteArray>()
        MjpegParser("extendoframe").readFrames(
            ByteArrayInputStream(stream(f1, withLength = false)), { true }, got::add
        )
        assertEquals(1, got.size)
        assertArrayEquals(f1, got[0])
    }

    @Test
    fun boundaryFromContentType() {
        assertEquals(
            "extendoframe",
            MjpegParser.boundaryFromContentType("multipart/x-mixed-replace; boundary=extendoframe"),
        )
        assertEquals("b", MjpegParser.boundaryFromContentType("multipart/x-mixed-replace;boundary=\"b\""))
        assertNull(MjpegParser.boundaryFromContentType("image/jpeg"))
        assertNull(MjpegParser.boundaryFromContentType(null))
    }

    @Test
    fun parseContentLength_caseInsensitive() {
        assertEquals(1234, MjpegParser.parseContentLength("Content-Length: 1234"))
        assertEquals(42, MjpegParser.parseContentLength("content-length: 42"))
        assertNull(MjpegParser.parseContentLength("Content-Type: image/jpeg"))
        assertNull(MjpegParser.parseContentLength("Content-Length: abc"))
        assertNull(MjpegParser.parseContentLength("no-colon-here"))
    }
}
