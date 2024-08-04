package io.ckaznable.localax.ui.model

import android.annotation.SuppressLint
import android.app.Application
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import android.util.Log
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.ckaznable.localax.FrontendReply
import io.ckaznable.localax.LocalaxService
import io.ckaznable.localax.LocalaxServiceEvent
import io.ckaznable.localax.rust.FfiDaemonPeer
import io.ckaznable.localax.rust.FfiPeerVerifyState
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

class MainViewModel(application: Application) : AndroidViewModel(application) {
    var verificationPeer: FfiDaemonPeer? = null
    private var peers: List<FfiDaemonPeer> = emptyList()

    var items = mutableStateOf<List<String>>(emptyList())
        private set

    var showDialog = mutableStateOf(false)
        private set

    @SuppressLint("StaticFieldLeak")
    private var dataService: LocalaxService? = null
    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            Log.d(LOG_TAG, "connecting to LocalaxService")
            val binder = service as LocalaxService.LocalBinder
            dataService = binder.getService()
            collectDataFromService()
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            Log.d(LOG_TAG, "disconnected with LocalaxService")
            dataService = null
        }

        override fun onBindingDied(name: ComponentName?) {
            Log.d(LOG_TAG, "binding died")
        }

        override fun onNullBinding(name: ComponentName?) {
            Log.d(LOG_TAG, "null binding")
        }
    }

    init {
        bindService(application)
    }

    private fun bindService(context: Context) {
        val intent = Intent(context, LocalaxService::class.java)
        context.bindService(intent, serviceConnection, Context.BIND_AUTO_CREATE)
    }

    private fun collectDataFromService() {
        viewModelScope.launch {
            dataService?.serviceFlow?.collectLatest { data ->
                when (data) {
                    is LocalaxServiceEvent.InComingVerifyRequest -> handleInComingVerifyRequest(data.peer)
                    is LocalaxServiceEvent.UpdatePeerList -> handleUpdatePeerList(data.list)
                    is LocalaxServiceEvent.VerifyResult -> handleVerifyResult(data.id, data.result)
                }
            }
        }
    }

    private fun handleVerifyResult(peerId: String, result: Boolean) {
       peers
           .find { it.peerIdStr == peerId }
           ?.let { it.state = if (result) FfiPeerVerifyState.VERIFIED else it.state }
    }

    private fun handleUpdatePeerList(list: List<FfiDaemonPeer>) {
        Log.d(LOG_TAG, "get " + list.size + " peers")
        items.value = list.map { it.hostname + ":" + it.peerIdStr }
        peers = list
    }

    private fun handleInComingVerifyRequest(peer: FfiDaemonPeer) {
        verificationPeer = peer
        showDialog.value = true
    }

    private fun sendToService(event: FrontendReply) {
        dataService?.sendUIEvent(event)
    }

    fun requestVerify(index: Int) {
        peers.getOrNull(index)?.let {
            sendToService(FrontendReply.RequestVerify(it.peerId))
        }
    }

    private fun dismissDialog() {
        showDialog.value = false
    }

    fun onAcceptVerifyRequest() {
        dismissDialog()
        replyVerifyRequest(true)
    }

    fun onRejectVerifyRequest() {
        dismissDialog()
        replyVerifyRequest(false)
    }

    private fun replyVerifyRequest(result: Boolean) {
        verificationPeer?.let {
            sendToService(FrontendReply.ReplyVerifyRequest(it.peerId, result))
        }
        verificationPeer = null
    }

    override fun onCleared() {
        super.onCleared()
        getApplication<Application>().unbindService(serviceConnection)
        getApplication<Application>().stopService(Intent(getApplication(), LocalaxService::class.java))
    }

    companion object {
        private const val LOG_TAG = "MainViewModel"
    }
}