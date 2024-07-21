package io.ckaznable.localax.localex

sealed class ClientEvent {
    data class RequestVerify(
        val peerId: String,
    ): ClientEvent()

    data class DisconnectPeer(
        val peerId: String,
    ): ClientEvent()

    data class VerifyConfirm(
        val peerId: String,
        val confirm: Boolean,
    ): ClientEvent()
}