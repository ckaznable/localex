package io.ckaznable.localax.localex

sealed class DaemonEvent {
    data class VerifyResult(
        val peerId: String,
        val result: Boolean,
    ): DaemonEvent()

    data class InComingVerify(
        val peer: DaemonPeer,
    ): DaemonEvent()

    data class PeerList(
        val list: ArrayList<DaemonPeer>,
    ): DaemonEvent()
}