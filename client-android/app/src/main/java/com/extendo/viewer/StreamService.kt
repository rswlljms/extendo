package com.extendo.viewer

// Foreground service keeping the stream alive under Doze / battery
// optimization (AGENTS.md section 7.7). The work itself lives in
// StreamRepository; this service is the keep-alive shell plus notification.

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.IBinder

class StreamService : Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        ensureChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                StreamRepository.disconnect()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
                return START_NOT_STICKY
            }
            ACTION_START -> {
                val host = intent.getStringExtra(EXTRA_HOST).orEmpty()
                val token = intent.getStringExtra(EXTRA_TOKEN).orEmpty()
                if (host.isEmpty() || token.isEmpty()) {
                    stopSelf()
                    return START_NOT_STICKY
                }
                startForeground(NOTIF_ID, notification("extendo ${host.substringAfter("://")}"))
                StreamRepository.connect(host, token)
                return START_STICKY
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        StreamRepository.disconnect()
        super.onDestroy()
    }

    private fun ensureChannel() {
        val nm = getSystemService(NotificationManager::class.java)
        if (nm.getNotificationChannel(CHANNEL) == null) {
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL, "extendo stream", NotificationManager.IMPORTANCE_LOW)
            )
        }
    }

    private fun notification(text: String): Notification {
        return Notification.Builder(this, CHANNEL)
            .setContentTitle("extendo second display")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.presence_video_online)
            .setOngoing(true)
            .build()
    }

    companion object {
        const val ACTION_START = "com.extendo.viewer.START"
        const val ACTION_STOP = "com.extendo.viewer.STOP"
        private const val EXTRA_HOST = "host"
        private const val EXTRA_TOKEN = "token"
        private const val CHANNEL = "extendo-stream"
        private const val NOTIF_ID = 9577

        fun start(ctx: Context, host: String, token: String) {
            val i = Intent(ctx, StreamService::class.java)
                .setAction(ACTION_START)
                .putExtra(EXTRA_HOST, host.trimEnd('/'))
                .putExtra(EXTRA_TOKEN, token)
            ctx.startForegroundService(i)
        }

        fun stop(ctx: Context) {
            ctx.startService(Intent(ctx, StreamService::class.java).setAction(ACTION_STOP))
        }
    }
}
