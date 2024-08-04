package io.ckaznable.localax

import io.ckaznable.localax.rust.FfiDaemonPeer

sealed class LocalaxServiceEvent {
    data class UpdatePeerList(val list: List<FfiDaemonPeer>): LocalaxServiceEvent()
    data class InComingVerifyRequest(val peer: FfiDaemonPeer): LocalaxServiceEvent()
    data class VerifyResult(val peerId: ByteArray, val id: String, val result: Boolean): LocalaxServiceEvent() {
        override fun equals(other: Any?): Boolean {
            if (this === other)
                return true
            if (javaClass != other?.javaClass)
                return false

            other as VerifyResult

            if (!peerId.contentEquals(other.peerId))
                return false
            if (result != other.result)
                return false

            return true
        }

        override fun hashCode(): Int {
            var result1 = peerId.contentHashCode()
            result1 = 31 * result1 + result.hashCode()
            return result1
        }
    }
}

sealed class FrontendReply {
    data class ReplyVerifyRequest(
        val peerId: ByteArray,
        val result: Boolean
    ): FrontendReply() {
        override fun equals(other: Any?): Boolean {
            if (this === other)
                return true
            if (javaClass != other?.javaClass)
                return false

            other as ReplyVerifyRequest
            if (!peerId.contentEquals(other.peerId))
                return false
            if (result != other.result)
                return false

            return true
        }

        override fun hashCode(): Int {
            var result1 = peerId.contentHashCode()
            result1 = 31 * result1 + result.hashCode()
            return result1
        }
    }
    data class RequestVerify(val peerId: ByteArray): FrontendReply() {
        override fun equals(other: Any?): Boolean {
            if (this === other)
                return true
            if (javaClass != other?.javaClass)
                return false

            other as RequestVerify

            return peerId.contentEquals(other.peerId)
        }

        override fun hashCode(): Int {
            return peerId.contentHashCode()
        }
    }
}