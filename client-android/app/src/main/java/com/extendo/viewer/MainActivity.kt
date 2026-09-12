package com.extendo.viewer

// Milestone-1 UI (parity with client-web): pair/connect, MJPEG viewer with
// RTT/fps overlay, transport switcher, quality presets, rotate, touch +
// keyboard backchannel. H.264/MediaCodec replaces the MJPEG layer next.

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                Root()
            }
        }
    }

    companion object {
        const val EXTRA_HOST = "host"
        const val EXTRA_TOKEN = "token"
        const val EXTRA_AUTOCONNECT = "autoconnect"
    }
}

@Composable
private fun Root() {
    val state by StreamRepository.state.collectAsState()
    when (val s = state) {
        is ConnState.Disconnected -> ConnectScreen(error = null)
        is ConnState.Connecting -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            CircularProgressIndicator()
        }
        is ConnState.Connected -> ViewerScreen(video = s.video)
        is ConnState.Error -> ConnectScreen(error = s.message)
    }
}

@Composable
private fun ConnectScreen(error: String?) {
    val ctx = LocalContext.current
    val activity = ctx as? ComponentActivity
    // Automation / QR-join: `am start -n com.extendo.viewer/.MainActivity
    // --es host http://10.0.2.2:9577 --es token <tok> --ez autoconnect true`
    val extraHost = activity?.intent?.getStringExtra(MainActivity.EXTRA_HOST).orEmpty()
    val extraToken = activity?.intent?.getStringExtra(MainActivity.EXTRA_TOKEN).orEmpty()
    val extraAuto = activity?.intent?.getBooleanExtra(MainActivity.EXTRA_AUTOCONNECT, false) == true
    var host by rememberSaveable { mutableStateOf(extraHost.ifEmpty { "http://192.168.1.10:9577" }) }
    var token by rememberSaveable { mutableStateOf(extraToken) }
    var autoFired by rememberSaveable { mutableStateOf(false) }
    val notifLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { /* proceed regardless; notification just stays silent */ }

    fun connect() {
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        StreamRepository.connect(host, token)
        StreamService.start(ctx, host, token)
    }

    // Headless/emulator smoke tests and automation land straight in the stream.
    LaunchedEffect(extraAuto, host, token) {
        if (extraAuto && !autoFired && host.isNotBlank() && token.isNotBlank()) {
            autoFired = true
            connect()
        }
    }

    Column(
        Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
    ) {
        Text("extendo second display", style = MaterialTheme.typography.headlineSmall)
        TextField(value = host, onValueChange = { host = it }, label = { Text("host (http://pc-ip:9577)") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        TextField(value = token, onValueChange = { token = it }, label = { Text("token / PIN") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        if (error != null) Text(error, color = MaterialTheme.colorScheme.error)
        Button(onClick = ::connect, enabled = host.isNotBlank() && token.isNotBlank(), modifier = Modifier.fillMaxWidth()) {
            Text("Connect")
        }
        Text(
            "Same Wi-Fi LAN (5GHz preferred), USB tethering, or ADB reverse. " +
                "Find the token on the host (install-dev output / config).",
            style = MaterialTheme.typography.bodySmall,
        )
    }
}

@Composable
private fun ViewerScreen(video: VideoConfig) {
    val ctx = LocalContext.current
    val bitmap by StreamRepository.bitmap.collectAsState()
    val overlay by StreamRepository.overlay.collectAsState()
    val transports by StreamRepository.transports.collectAsState()
    val best by StreamRepository.best.collectAsState()
    val rotated by StreamRepository.rotated.collectAsState()
    var notice by remember { mutableStateOf("") }
    val focusRequester = remember { FocusRequester() }
    LaunchedEffect(Unit) { focusRequester.requestFocus() }

    Column(Modifier.fillMaxSize()) {
        Text(overlay.ifEmpty { "connecting…" }, Modifier.padding(8.dp), style = MaterialTheme.typography.bodySmall)
        if (notice.isNotEmpty()) Text(notice, Modifier.padding(horizontal = 8.dp), style = MaterialTheme.typography.bodySmall)
        Box(
            Modifier
                .fillMaxWidth()
                .weight(1f, fill = false),
            contentAlignment = Alignment.Center,
        ) {
            val bmp = bitmap
            if (bmp != null) {
                Image(
                    bitmap = bmp.asImageBitmap(),
                    contentDescription = "extended display",
                    contentScale = ContentScale.Fit,
                    modifier = Modifier
                        .fillMaxWidth()
                        .focusRequester(focusRequester)
                        .onKeyEvent { e ->
                            if (e.key == Key.Back) return@onKeyEvent false
                            val down = e.type == KeyEventType.KeyDown
                            StreamRepository.sendKey(e.key.keyCode.toInt(), down)
                            true
                        }
                        .pointerInput(Unit) {
                            awaitEachGesture {
                                val size = size
                                if (size.width <= 0 || size.height <= 0) return@awaitEachGesture
                                val down = awaitFirstDown()
                                StreamRepository.sendTouch(
                                    down.position.x / size.width,
                                    down.position.y / size.height,
                                    true,
                                )
                                var done = false
                                while (!done) {
                                    val event = awaitPointerEvent()
                                    val pressed = event.changes.filter { it.pressed }
                                    if (pressed.isEmpty()) {
                                        StreamRepository.sendTouch(null, null, false)
                                        done = true
                                    } else {
                                        pressed.forEach {
                                            StreamRepository.sendTouch(
                                                (it.position.x / size.width).coerceIn(0f, 1f),
                                                (it.position.y / size.height).coerceIn(0f, 1f),
                                                null,
                                            )
                                        }
                                    }
                                }
                            }
                        },
                )
            } else {
                CircularProgressIndicator(Modifier.padding(32.dp))
            }
        }
        // Transport switcher (Wi-Fi <-> USB without re-pairing).
        if (transports.isNotEmpty()) {
            LazyRow(Modifier.padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(transports, key = { it.kind.name + it.addr }) { t ->
                    val selected = best?.addr == t.addr
                    OutlinedButton(onClick = { StreamRepository.switchTransport(t) }, enabled = !selected) {
                        Text(t.label(), maxLines = 1)
                    }
                }
            }
        }
        Row(Modifier.padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { StreamRepository.setQuality(1280, 720, 30) { notice = it } }) { Text("720p30") }
            OutlinedButton(onClick = { StreamRepository.setQuality(1920, 1080, 60) { notice = it } }) { Text("1080p60") }
            OutlinedButton(onClick = { StreamRepository.setRotated(!rotated) }) {
                Text(if (rotated) "landscape" else "portrait")
            }
            OutlinedButton(onClick = { StreamService.stop(ctx) }) { Text("leave") }
        }
    }
}
