package io.ckaznable.localax.localex

data class DaemonPeer(
    val peerId: String,
    val state: VerifyState,
    val hostname: String,
)