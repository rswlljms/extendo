package com.extendo.viewer

// Minimal multipart/x-mixed-replace reader for GET /video.mjpg.
// Pure JVM (no Android imports): boundary scanning and part splitting are
// unit-tested without a device. Bitmap decode happens in the repository.

import java.io.ByteArrayOutputStream
import java.io.EOFException
import java.io.InputStream

class MjpegParser(private val boundary: String) {

    /** Read frames until the stream ends or [isOpen] returns false. */
    fun readFrames(stream: InputStream, isOpen: () -> Boolean, onFrame: (ByteArray) -> Unit) {
        val buf = stream.buffered(64 * 1024)
        val marker = "--$boundary"
        // Skip the preamble: first boundary line.
        var line = readLine(buf) ?: return
        while (line.trim() != marker && line.trim() != "$marker--") {
            line = readLine(buf) ?: return
        }
        while (isOpen()) {
            if (line.trim() == "$marker--") return
            // Part headers.
            var contentLength: Int? = null
            while (true) {
                val h = readLine(buf) ?: return
                if (h.isEmpty()) break
                contentLength = contentLength ?: parseContentLength(h)
            }
            if (contentLength != null) {
                val body = try {
                    readExactly(buf, contentLength)
                } catch (_: EOFException) {
                    return
                }
                if (body.isNotEmpty()) onFrame(body)
                line = readLine(buf) ?: return
            } else {
                // No length: scan to the next boundary (slow path; our server
                // always sends Content-Length, this is for proxies that strip it).
                // Returns with the boundary line fully consumed.
                val body = readUntilBoundary(buf, marker)
                if (body.isNotEmpty()) onFrame(body)
                line = marker
            }
        }
    }

    companion object {
        /** `multipart/x-mixed-replace; boundary=extendoframe` -> `extendoframe`. */
        fun boundaryFromContentType(contentType: String?): String? {
            if (contentType == null) return null
            val idx = contentType.indexOf("boundary=")
            if (idx < 0) return null
            return contentType.substring(idx + "boundary=".length).trim().trim('"').trim()
                .takeIf { it.isNotEmpty() }
        }

        fun parseContentLength(headerLine: String): Int? {
            val parts = headerLine.split(":", limit = 2)
            if (parts.size != 2) return null
            if (!parts[0].trim().equals("Content-Length", ignoreCase = true)) return null
            return parts[1].trim().toIntOrNull()?.takeIf { it > 0 }
        }

        private fun readLine(stream: InputStream): String? {
            val out = ByteArrayOutputStream()
            while (true) {
                val b = stream.read()
                if (b < 0) return if (out.size() == 0) null else out.toString("ISO-8859-1")
                if (b == '\n'.code) break
                if (b != '\r'.code) out.write(b)
            }
            return out.toString("ISO-8859-1")
        }

        private fun readExactly(stream: InputStream, n: Int): ByteArray {
            val out = ByteArray(n)
            var off = 0
            while (off < n) {
                val r = stream.read(out, off, n - off)
                if (r < 0) throw EOFException("mjpeg part truncated ($off/$n bytes)")
                off += r
            }
            return out
        }

        private fun readUntilBoundary(stream: InputStream, marker: String): ByteArray {
            // Byte-window search (body is binary; line scanning would split on
            // 0x0A bytes inside JPEG entropy data). Consumes through the end
            // of the boundary line so the caller can continue with headers.
            val out = ByteArrayOutputStream()
            // NOTE: `marker` already includes the leading "--".
            val m = marker.toByteArray(Charsets.ISO_8859_1)
            val window = ArrayDeque<Byte>()
            while (true) {
                val b = stream.read()
                if (b < 0) break
                out.write(b)
                window.addLast(b.toByte())
                if (window.size > m.size) window.removeFirst()
                if (window.size == m.size && window.toByteArray().contentEquals(m)) {
                    val full = out.toByteArray()
                    var end = full.size - m.size
                    if (end >= 2 && full[end - 2] == '\r'.code.toByte() && full[end - 1] == '\n'.code.toByte()) {
                        end -= 2
                    } else if (end >= 1 && full[end - 1] == '\n'.code.toByte()) {
                        end -= 1
                    }
                    // Consume the rest of the boundary line ("--" suffix or CRLF).
                    while (true) {
                        val c = stream.read()
                        if (c < 0 || c == '\n'.code) break
                    }
                    return full.copyOf(end)
                }
            }
            return out.toByteArray()
        }
    }
}
