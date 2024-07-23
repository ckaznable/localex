package io.ckaznable.localax

import io.ckaznable.localax.localex.ClientEvent
import io.ckaznable.localax.localex.DaemonEvent

class Localex {
    external fun init(name: String, key: ByteArray)
    external fun dispatch(event: ClientEvent)
    external fun recv(): DaemonEvent
    external fun listen()
    external fun getKeyPair(): ByteArray
    external fun stop()

    companion object {
        init {
            System.loadLibrary("localex")
        }
    }
}