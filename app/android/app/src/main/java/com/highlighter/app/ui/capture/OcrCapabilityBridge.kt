package com.highlighter.app.ui.capture

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Rect
import android.util.Log
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.Text
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import uniffi.highlighter_core.CapabilityRequest
import uniffi.highlighter_core.CapabilityResult
import uniffi.highlighter_core.OcrLine
import uniffi.highlighter_core.OcrOp
import uniffi.highlighter_core.OcrRect
import uniffi.highlighter_core.OcrResult
import uniffi.highlighter_core.OcrWord
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

private const val TAG = "OcrCapabilityBridge"

internal object OcrCapabilityBridge {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    fun handleCapabilityRequest(
        request: CapabilityRequest,
        provideResult: (CapabilityResult) -> Unit,
    ): Boolean {
        val op = (request as? CapabilityRequest.Ocr)?.v1 ?: return false
        when (op) {
            is OcrOp.RecognizeText -> recognize(op.imageHandle, provideResult)
        }
        return true
    }

    private fun recognize(
        imageHandle: String,
        provideResult: (CapabilityResult) -> Unit,
    ) {
        scope.launch {
            val result = runCatching {
                val bitmap = BitmapFactory.decodeFile(imageHandle)
                    ?: throw IllegalStateException("Could not load capture image")
                recognizeLines(bitmap)
            }.fold(
                onSuccess = { OcrResult.Lines(it) },
                onFailure = { error ->
                    Log.e(TAG, "OCR failed", error)
                    OcrResult.Error(error.message ?: "OCR failed")
                },
            )
            provideResult(CapabilityResult.Ocr(result))
        }
    }
}

private suspend fun recognizeLines(bitmap: Bitmap): List<OcrLine> =
    suspendCancellableCoroutine { cont ->
        val image = InputImage.fromBitmap(bitmap, 0)
        val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
        recognizer.process(image)
            .addOnSuccessListener { visionText ->
                cont.resume(visionText.toOcrLines(bitmap.width, bitmap.height))
            }
            .addOnFailureListener { e ->
                cont.resumeWithException(e)
            }
    }

private fun Text.toOcrLines(imageWidth: Int, imageHeight: Int): List<OcrLine> =
    textBlocks.flatMap { block ->
        block.lines.mapNotNull { line ->
            val box = line.boundingBox ?: return@mapNotNull null
            OcrLine(
                text = line.text,
                bbox = box.toOcrRect(imageWidth, imageHeight),
                confidence = 1.0f,
                words = line.elements.mapNotNull { element ->
                    val elementBox = element.boundingBox ?: return@mapNotNull null
                    OcrWord(
                        text = element.text,
                        bbox = elementBox.toOcrRect(imageWidth, imageHeight),
                        confidence = 1.0f,
                    )
                },
            )
        }
    }

private fun Rect.toOcrRect(imageWidth: Int, imageHeight: Int): OcrRect {
    val width = imageWidth.coerceAtLeast(1).toDouble()
    val height = imageHeight.coerceAtLeast(1).toDouble()
    return OcrRect(
        x = left.toDouble() / width,
        y = (imageHeight - bottom).toDouble() / height,
        w = this.width().coerceAtLeast(0).toDouble() / width,
        h = this.height().coerceAtLeast(0).toDouble() / height,
    )
}
