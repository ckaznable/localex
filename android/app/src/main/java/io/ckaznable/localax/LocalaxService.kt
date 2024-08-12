package io.ckaznable.localax

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Binder
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import io.ckaznable.localax.rust.FfiClientEvent
import io.ckaznable.localax.rust.FfiDaemonEvent
import io.ckaznable.localax.rust.FfiDaemonPeer
import io.ckaznable.localax.rust.dispatch
import io.ckaznable.localax.rust.listen
import io.ckaznable.localax.rust.recv
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch
import java.util.Locale

class LocalaxService : Service() {
    private val uiScope = CoroutineScope(Dispatchers.Default + Job())
    private val serviceScope = CoroutineScope(Dispatchers.Default + Job())
    private val cleanupScope = CoroutineScope(Dispatchers.Default + Job())
    private var isListeningDaemon = false

    private val _serviceFlow = MutableSharedFlow<LocalaxServiceEvent>()
    val serviceFlow = _serviceFlow.asSharedFlow()

    private val _uiFlow = MutableSharedFlow<FrontendReply>()
    private val uiFlow = _uiFlow.asSharedFlow()

    private val binder = LocalBinder()
    inner class LocalBinder : Binder() {
        fun getService(): LocalaxService = this@LocalaxService
    }

    override fun onBind(intent: Intent?): IBinder = binder

    @SuppressLint("ForegroundServiceType")
    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        startForeground(NOTIFICATION_ID, createNotification())

        Log.d(LOG_TAG, "localax init")
        io.ckaznable.localax.rust.init(getDeviceName(), null)
    }

    private fun getDeviceName(): String {
        return Build.MODEL
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        isListeningDaemon = true
        Log.d(LOG_TAG, "localax start listen")

        serviceScope.launch {
            handleUIEvent()
        }

        serviceScope.launch {
            listen()
        }

        serviceScope.launch {
            while (isListeningDaemon) {
                when (val data = recv()) {
                    is FfiDaemonEvent.InComingVerify -> handleInComingVerify(data.v1)
                    is FfiDaemonEvent.PeerList -> handlePeerList(data.v1)
                    is FfiDaemonEvent.VerifyResult -> handleVerifyResult(data.v1, data.v2, data.v3)
                    is FfiDaemonEvent.Error -> Log.d(LOG_TAG, "error: ${data.v1}")
                    else -> Unit
                }
            }
        }

        return START_STICKY
    }

    override fun onDestroy() {
        cleanupScope.launch {
            try {
                isListeningDaemon = false
                io.ckaznable.localax.rust.stop()
            } finally {
                super.onDestroy()
                serviceScope.cancel()
                uiScope.cancel()
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

    private suspend fun handleInComingVerify(peer: FfiDaemonPeer) {
        _serviceFlow.emit(LocalaxServiceEvent.InComingVerifyRequest(peer))
    }

    private suspend fun handlePeerList(list: List<FfiDaemonPeer>) {
        Log.d(LOG_TAG, "get peer list ${list.size}")
        _serviceFlow.emit(LocalaxServiceEvent.UpdatePeerList(list))
    }

    private suspend fun handleVerifyResult(peerId: ByteArray, id: String, result: Boolean) {
        _serviceFlow.emit(LocalaxServiceEvent.VerifyResult(peerId, id, result))
    }

    private suspend fun handleUIEvent() {
        uiFlow.collect { event ->
            when (event) {
                is FrontendReply.ReplyVerifyRequest -> {
                    Log.d(LOG_TAG, "client reply verify request: ${if (event.result) "YES" else "No"}")
                    dispatch(FfiClientEvent.VerifyConfirm(event.peerId, event.result))
                }
                is FrontendReply.RequestVerify -> {
                    Log.d(LOG_TAG, "client request verify")
                    dispatch(FfiClientEvent.RequestVerify(event.peerId))
                }
            }
        }
    }

    fun sendUIEvent(command: FrontendReply) {
        uiScope.launch {
            _uiFlow.emit(command)
        }
    }

    companion object {
        private const val NOTIFICATION_ID = 1
        private const val LOG_TAG = "LocalaxService"
        private const val CHANNEL_ID = "LocalaxListenChannel"
    }
}