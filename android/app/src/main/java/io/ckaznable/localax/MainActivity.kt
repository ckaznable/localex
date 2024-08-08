package io.ckaznable.localax

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
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
    val statusBarHeight = WindowInsets.systemBars.asPaddingValues().calculateTopPadding()
    Box(modifier = Modifier
        .fillMaxSize()
        .padding(top = statusBarHeight)) {

        ItemList(viewModel.items.value, onSelect = { viewModel.requestVerify(it) })

        if (viewModel.showDialog.value && viewModel.verificationPeer != null) {
            InComingVerifyDialog(
                viewModel.verificationPeer?.hostname + " InComing Verify Request",
                viewModel.verificationPeer?.peerIdStr ?: "",
                onOk = { viewModel.onAcceptVerifyRequest() },
                onCancel = { viewModel.onRejectVerifyRequest() }
            )
        }
    }
}

@Preview(showBackground = true)
@Composable
fun ItemListPreview() {
    val items = listOf(
        "otherRemote:xxx",
        "client 1:yyy"
    )

    LocalaxTheme {
        ItemList(items, onSelect = {})
    }
}

@Composable
fun ItemList(items: List<String>, onSelect: (Int) -> Unit) {
    var selectedIndex by remember { mutableIntStateOf(-1) }

    val normalTextColor = Color.DarkGray
    val selectedTextColor = Color(0xFFFFFFFF)
    val selectedBackgroundColor = MaterialTheme.colorScheme.primary

    LazyColumn {
        itemsIndexed(items) { index, item ->
            val backgroundColor = if (index % 2 == 0) {
                MaterialTheme.colorScheme.secondary
            } else {
                MaterialTheme.colorScheme.tertiary
            }

            val isSelected = index == selectedIndex

            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(if (isSelected) selectedBackgroundColor else backgroundColor)
                    .clickable {
                        selectedIndex = index
                        onSelect(index)
                    }
                    .padding(8.dp)
            ) {
                Text(
                    text = item,
                    color = if (isSelected) selectedTextColor else normalTextColor,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
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
                modifier = Modifier.padding(16.dp)
                    .fillMaxWidth(),
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
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                ) {
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