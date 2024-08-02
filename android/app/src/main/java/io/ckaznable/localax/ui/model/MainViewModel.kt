package io.ckaznable.localax.ui.model

import android.annotation.SuppressLint
import android.app.Application
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.ckaznable.localax.FrontendReply
import io.ckaznable.localax.LocalaxService
import io.ckaznable.localax.LocalaxServiceEvent
import io.ckaznable.localax.rust.FfiDaemonPeer
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

class MainViewModel(application: Application) : AndroidViewModel(application) {
    private var verificationPeer: FfiDaemonPeer? = null

    var items = mutableStateOf<List<String>>(emptyList())
        private set

    var showDialog = mutableStateOf(false)
        private set

    @SuppressLint("StaticFieldLeak")
    private var dataService: LocalaxService? = null
    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            val binder = service as LocalaxService.LocalBinder
            dataService = binder.getService()
            collectDataFromService()
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            dataService = null
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
                }
            }
        }
    }

    private fun handleUpdatePeerList(list: List<FfiDaemonPeer>) {
        items.value = list.map { it.hostname }
    }

    private fun handleInComingVerifyRequest(peer: FfiDaemonPeer) {
        verificationPeer = peer
        showDialog.value = true
    }

    private fun sendToService(event: FrontendReply) {
        dataService?.sendUIEvent(event)
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
}