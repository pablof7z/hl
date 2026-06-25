package com.highlighter.app.ui.capture

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.util.Log
import androidx.camera.core.CameraSelector
import androidx.camera.core.ExperimentalGetImage
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
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
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

private const val TAG = "CameraCapture"

/**
 * Result of a camera capture pass.
 *
 * [jpegBytes] — JPEG-encoded page image ready for UploadCapturePhoto.
 * [width] / [height] — intrinsic dimensions of the image.
 */
internal data class CaptureResult(
    val jpegBytes: ByteArray,
    val width: UInt,
    val height: UInt,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is CaptureResult) return false
        return jpegBytes.contentEquals(other.jpegBytes) &&
            width == other.width && height == other.height
    }

    override fun hashCode(): Int {
        var result = jpegBytes.contentHashCode()
        result = 31 * result + width.hashCode()
        result = 31 * result + height.hashCode()
        return result
    }
}

/**
 * Full-screen CameraX viewfinder with a capture button.
 *
 * On capture, the image is:
 *  1. JPEG-compressed (quality 90).
 *  2. [onCapture] is called with the [CaptureResult].
 *
 * The caller dismisses/replaces this screen; this composable only drives
 * the camera preview and capture flow.
 */
@Composable
internal fun CameraCaptureScreen(
    onCapture: (CaptureResult) -> Unit,
    onDismiss: () -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val scope = rememberCoroutineScope()

    var isCaptureInProgress by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    val imageCapture = remember { ImageCapture.Builder().build() }

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
                    try {
                        cameraProvider.unbindAll()
                        cameraProvider.bindToLifecycle(
                            lifecycleOwner,
                            CameraSelector.DEFAULT_BACK_CAMERA,
                            preview,
                            imageCapture,
                        )
                    } catch (backE: Exception) {
                        Log.e(TAG, "Back camera bind failed, trying front camera", backE)
                        try {
                            cameraProvider.unbindAll()
                            cameraProvider.bindToLifecycle(
                                lifecycleOwner,
                                CameraSelector.DEFAULT_FRONT_CAMERA,
                                preview,
                                imageCapture,
                            )
                        } catch (frontE: Exception) {
                            Log.e(TAG, "Front camera bind also failed", frontE)
                            errorMessage = "Camera failed to start: ${frontE.message}"
                        }
                    }
                }, ContextCompat.getMainExecutor(ctx))
                previewView
            },
            modifier = Modifier.fillMaxSize(),
        )

        // ── Top bar: dismiss ─────────────────────────────────────────────────
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.Start,
        ) {
            IconButton(
                onClick = onDismiss,
                modifier = Modifier
                    .background(Color.Black.copy(alpha = 0.45f), CircleShape)
                    .testTag("capture_camera_button"),
            ) {
                Icon(Icons.Default.Close, contentDescription = "Cancel", tint = Color.White)
            }
        }

        // ── Capture button + status ──────────────────────────────────────────
        Column(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 48.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            errorMessage?.let { msg ->
                Surface(
                    color = MaterialTheme.colorScheme.errorContainer,
                    shape = RoundedCornerShape(8.dp),
                    modifier = Modifier.padding(horizontal = 24.dp),
                ) {
                    Text(
                        text = msg,
                        modifier = Modifier.padding(12.dp),
                        color = MaterialTheme.colorScheme.onErrorContainer,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                Spacer(modifier = Modifier.height(12.dp))
            }

            if (isCaptureInProgress) {
                CircularProgressIndicator(
                    color = Color.White,
                    modifier = Modifier.size(56.dp),
                )
                Spacer(modifier = Modifier.height(8.dp))
                Text("Capturing…", color = Color.White, style = MaterialTheme.typography.bodySmall)
            } else {
                // Large shutter button
                Surface(
                    shape = CircleShape,
                    color = Color.White,
                    shadowElevation = 4.dp,
                    modifier = Modifier
                        .size(72.dp)
                        .testTag("capture_take_photo"),
                    onClick = {
                        isCaptureInProgress = true
                        errorMessage = null
                        scope.launch {
                            try {
                                val result = captureAndOcr(context, imageCapture)
                                onCapture(result)
                            } catch (e: Exception) {
                                Log.e(TAG, "Capture/OCR failed", e)
                                errorMessage = e.message ?: "Capture failed"
                                isCaptureInProgress = false
                            }
                        }
                    },
                ) {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center,
                    ) {
                        Surface(
                            shape = CircleShape,
                            color = Color.White.copy(alpha = 0.6f),
                            modifier = Modifier.size(60.dp),
                        ) {}
                    }
                }
            }
        }
    }
}

// ── Capture pipeline ─────────────────────────────────────────────────────────

/**
 * Takes a photo, compresses it to JPEG (quality 90), and returns the
 * [CaptureResult]. OCR runs later through the Rust-owned capability flow after
 * the image handle is written to disk.
 *
 * Runs image processing on [Dispatchers.IO] and resumes on the caller's
 * coroutine.
 */
@OptIn(ExperimentalGetImage::class)
private suspend fun captureAndOcr(
    context: Context,
    imageCapture: ImageCapture,
): CaptureResult = withContext(Dispatchers.IO) {
    // 1. Capture image from camera.
    val imageProxy: ImageProxy = suspendCancellableCoroutine { cont ->
        imageCapture.takePicture(
            ContextCompat.getMainExecutor(context),
            object : ImageCapture.OnImageCapturedCallback() {
                override fun onCaptureSuccess(image: ImageProxy) = cont.resume(image)
                override fun onError(exc: ImageCaptureException) = cont.resumeWithException(exc)
            },
        )
    }

    // 2. Decode to bitmap, apply rotation, re-encode to JPEG.
    val bitmap: Bitmap = imageProxy.use { proxy ->
        val buffer = proxy.planes[0].buffer
        val bytes = ByteArray(buffer.remaining())
        buffer.get(bytes)
        val raw = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
            ?: throw IllegalStateException("Failed to decode captured frame")
        if (proxy.imageInfo.rotationDegrees != 0) {
            val matrix = Matrix().apply { postRotate(proxy.imageInfo.rotationDegrees.toFloat()) }
            Bitmap.createBitmap(raw, 0, 0, raw.width, raw.height, matrix, true)
        } else {
            raw
        }
    }

    val jpegBytes: ByteArray = ByteArrayOutputStream().use { out ->
        bitmap.compress(Bitmap.CompressFormat.JPEG, 90, out)
        out.toByteArray()
    }
    val width = bitmap.width.toUInt()
    val height = bitmap.height.toUInt()

    CaptureResult(
        jpegBytes = jpegBytes,
        width = width,
        height = height,
    )
}
