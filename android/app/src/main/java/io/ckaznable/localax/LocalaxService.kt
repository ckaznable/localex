package io.ckaznable.localax

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

class LocalaxService : Service() {
    private val serviceScope = CoroutineScope(Dispatchers.Default + Job())
    private val cleanupScope = CoroutineScope(Dispatchers.Default + Job())

    override fun onBind(intent: Intent?): IBinder? = null

    @SuppressLint("ForegroundServiceType")
    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        startForeground(NOTIFICATION_ID, createNotification())

        Log.d(LOG_TAG, "localax init")
        io.ckaznable.localax.rust.init("android", null)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        serviceScope.launch {
            Log.d(LOG_TAG, "localax start listen")
            io.ckaznable.localax.rust.listen()
        }
        return START_STICKY
    }

    override fun onDestroy() {
        cleanupScope.launch {
            try {
                io.ckaznable.localax.rust.stop();
            } finally {
                super.onDestroy()
                serviceScope.cancel()
            }
        }
    }

    private fun createNotificationChannel() {
        val channelName = "Listen Service Channel"
        val channel = NotificationChannel(CHANNEL_ID, channelName, NotificationManager.IMPORTANCE_DEFAULT)
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(channel)
    }

    private fun createNotification(): Notification {
        val notificationIntent = Intent(this, MainActivity::class.java)
        val pendingIntent = PendingIntent.getActivity(this, 0, notificationIntent, PendingIntent.FLAG_IMMUTABLE)

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Localax P2P")
            .setContentText("Service is running...")
            .setContentIntent(pendingIntent)
            .build()
    }

    companion object {
        private const val NOTIFICATION_ID = 1
        private const val LOG_TAG = "LibP2P"
        private const val CHANNEL_ID = "LocalaxListenChannel"
    }
}