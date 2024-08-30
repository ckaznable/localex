package io.ckaznable.localax

import android.content.Context
import android.util.Base64
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKeys

class SecureByteArrayStorage(context: Context) {
    private val masterKeyAlias = MasterKeys.getOrCreate(MasterKeys.AES256_GCM_SPEC)

    private val sharedPreferences = EncryptedSharedPreferences.create(
        "secure_byte_array_prefs",
        masterKeyAlias,
        context,
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
    )

    fun saveByteArray(key: StorageKeys, byteArray: ByteArray) {
        val base64Encoded = Base64.encodeToString(byteArray, Base64.DEFAULT)
        sharedPreferences.edit().putString(key.key, base64Encoded).apply()
    }

    fun getByteArray(key: StorageKeys): ByteArray? {
        val base64Encoded = sharedPreferences.getString(key.key, null)
        return base64Encoded?.let { Base64.decode(it, Base64.DEFAULT) }
    }
}

enum class StorageKeys(val key: String) {
    KeyPair("keypair"),
}