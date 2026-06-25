package com.highlighter.app.ui.capture

import android.annotation.SuppressLint
import android.util.Log
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import uniffi.highlighter_core.normalizeIsbn
import java.util.concurrent.Executors

private const val TAG = "IsbnBarcodeScanner"

/**
 * Full-screen CameraX barcode scanner for book ISBNs.
 *
 * Mirrors iOS BookScannerView / BookScannerModel. Continuously analyses the
 * camera stream with ML Kit BarcodeScanning; the first valid EAN-13/ISBN-13
 * barcode that passes [normalizeIsbn] is handed to [onResult] and the screen
 * dismisses.
 *
 * @param onResult Called with the normalized 13-digit ISBN on a successful
 *   scan, or null when the user cancels via the close button.
 */
@Composable
internal fun IsbnBarcodeScannerScreen(
    onResult: (String?) -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current

    var fired by remember { mutableStateOf(false) }
    var statusText by remember { mutableStateOf("Point at the book's back cover barcode") }

    val analysisExecutor = remember { Executors.newSingleThreadExecutor() }

    DisposableEffect(Unit) {
        onDispose { analysisExecutor.shutdown() }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black),
    ) {
        // ── Camera preview ────────────────────────────────────────────────────
        AndroidView(
            factory = { ctx ->
                val previewView = PreviewView(ctx)
                val cameraProviderFuture = ProcessCameraProvider.getInstance(ctx)
                cameraProviderFuture.addListener({
                    val cameraProvider = cameraProviderFuture.get()
                    val preview = Preview.Builder().build().also {
                        it.surfaceProvider = previewView.surfaceProvider
                    }
                    val imageAnalysis = ImageAnalysis.Builder()
                        .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                        .build()
                    imageAnalysis.setAnalyzer(analysisExecutor) { imageProxy ->
                        if (!fired) {
                            analyzeFrame(imageProxy) { rawIsbn ->
                                if (!fired) {
                                    val normalized = normalizeIsbn(rawIsbn)
                                    if (normalized != null) {
                                        fired = true
                                        onResult(normalized)
                                    } else {
                                        statusText = "Not a book ISBN — keep scanning"
                                    }
                                }
                            }
                        } else {
                            imageProxy.close()
                        }
                    }

                    try {
                        cameraProvider.unbindAll()
                        cameraProvider.bindToLifecycle(
                            lifecycleOwner,
                            CameraSelector.DEFAULT_BACK_CAMERA,
                            preview,
                            imageAnalysis,
                        )
                    } catch (e: Exception) {
                        Log.e(TAG, "Camera bind failed", e)
                        statusText = "Camera failed to start"
                    }
                }, ContextCompat.getMainExecutor(ctx))
                previewView
            },
            modifier = Modifier.fillMaxSize(),
        )

        // ── Reticle hint ──────────────────────────────────────────────────────
        Box(
            modifier = Modifier
                .align(Alignment.Center)
                .fillMaxWidth(0.8f)
                .height(120.dp)
                .background(Color.Transparent)
                .padding(2.dp),
        ) {
            // Corner bracket overlay drawn as a surface outline.
            Surface(
                modifier = Modifier.fillMaxSize(),
                shape = RoundedCornerShape(12.dp),
                color = Color.Transparent,
                border = androidx.compose.foundation.BorderStroke(2.dp, Color.White.copy(alpha = 0.7f)),
            ) {}
        }

        // ── Status text ───────────────────────────────────────────────────────
        Box(
            modifier = Modifier
                .align(Alignment.Center)
                .padding(top = 140.dp),
        ) {
            Text(
                text = statusText,
                style = MaterialTheme.typography.bodySmall,
                color = Color.White.copy(alpha = 0.85f),
                modifier = Modifier
                    .background(Color.Black.copy(alpha = 0.4f), RoundedCornerShape(8.dp))
                    .padding(horizontal = 12.dp, vertical = 6.dp),
            )
        }

        // ── Dismiss button ────────────────────────────────────────────────────
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.Start,
        ) {
            IconButton(
                onClick = { onResult(null) },
                modifier = Modifier
                    .background(Color.Black.copy(alpha = 0.45f), CircleShape)
                    .testTag("capture_scan_barcode"),
            ) {
                Icon(Icons.Default.Close, contentDescription = "Cancel scan", tint = Color.White)
            }
        }
    }
}

/**
 * Processes one [ImageProxy] frame with ML Kit BarcodeScanning and calls
 * [onFound] with the raw display value when a barcode is detected.
 * Always closes [imageProxy] when done.
 */
@SuppressLint("UnsafeOptInUsageError")
private fun analyzeFrame(imageProxy: ImageProxy, onFound: (String) -> Unit) {
    val mediaImage = imageProxy.image
    if (mediaImage == null) {
        imageProxy.close()
        return
    }
    val image = InputImage.fromMediaImage(mediaImage, imageProxy.imageInfo.rotationDegrees)
    val scanner = BarcodeScanning.getClient()
    scanner.process(image)
        .addOnSuccessListener { barcodes ->
            for (barcode in barcodes) {
                val raw = barcode.displayValue ?: continue
                // Accept EAN-13 (ISBN-13 lives here) and EAN-8, plus ISBN format.
                if (barcode.format == Barcode.FORMAT_EAN_13 ||
                    barcode.format == Barcode.FORMAT_EAN_8 ||
                    barcode.valueType == Barcode.TYPE_ISBN
                ) {
                    onFound(raw)
                    break
                }
            }
        }
        .addOnFailureListener { e ->
            Log.w(TAG, "Barcode analysis error: ${e.message}")
        }
        .addOnCompleteListener {
            imageProxy.close()
        }
}
