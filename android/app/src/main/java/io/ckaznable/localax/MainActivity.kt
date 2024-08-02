package io.ckaznable.localax

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import io.ckaznable.localax.ui.model.MainViewModel
import io.ckaznable.localax.ui.theme.LocalaxTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        startListenService()

        val viewModel = MainViewModel(application)

        enableEdgeToEdge()
        setContent {
            LocalaxTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    MainScreen(viewModel)
                }
            }
        }
    }

    private fun startListenService() {
        val serviceIntent = Intent(this, LocalaxService::class.java)
        startForegroundService(serviceIntent)
    }
}

@Composable
fun MainScreen(viewModel: MainViewModel) {
    Box(modifier = Modifier.fillMaxSize()) {
        // 列表
        ItemList(viewModel.items.value)

        if (viewModel.showDialog.value) {
            InComingVerifyDialog(
                "InComing Verify Request",
                "",
                onOk = { viewModel.onAcceptVerifyRequest() },
                onCancel = { viewModel.onRejectVerifyRequest() }
            )
        }
    }
}

@Composable
fun ItemList(items: List<String>) {
    LazyColumn {
        items(items) { item ->
            Text(
                text = item,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp)
            )
        }
    }
}

@Preview(showBackground = true)
@Composable
fun InComingVerifyDialogPreview() {
    LocalaxTheme {
        InComingVerifyDialog("Preview Title", "Preview Description", onOk = {}, onCancel = {})
    }
}

@Composable
fun InComingVerifyDialog(title: String, desc: String, onOk: () -> Unit, onCancel: () -> Unit) {
    Dialog(onDismissRequest = onCancel) {
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            elevation = CardDefaults.cardElevation(defaultElevation = 8.dp)
        ) {
            Column(
                modifier = Modifier.padding(16.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleLarge
                )
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = desc,
                    style = MaterialTheme.typography.bodyMedium
                )
                Spacer(modifier = Modifier.height(8.dp))
                Row {
                    Button(onClick = onOk, modifier = Modifier.padding(PaddingValues(0.dp, 0.dp, 8.dp, 0.dp))) {
                        Text("OK")
                    }
                    Button(onClick = onCancel) {
                        Text("cancel")
                    }
                }
            }
        }
    }
}