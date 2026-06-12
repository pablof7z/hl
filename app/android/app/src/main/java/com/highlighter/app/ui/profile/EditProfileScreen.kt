package com.highlighter.app.ui.profile

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.DestinationScaffold
import com.highlighter.app.ui.components.AvatarImage
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.util.readPickedImage
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterEditProfileImageTarget
import uniffi.highlighter_core.HighlighterEditProfileSnapshot

/**
 * Full-screen Edit Profile destination for the current user. The Rust core owns
 * every field value, the per-image upload flags, the saving flag, the error
 * message, and the saved-profile projection — this screen is a pure projection
 * of [HighlighterEditProfileSnapshot] that dispatches `SetEditProfile*`,
 * `UploadEditProfileImage`, and `SubmitEditProfile`.
 *
 * Presentation is host-held (see `RootScene`): the parent flips a boolean and
 * dispatches `OpenEditProfile(seed:)` on enter; this screen reports back via
 * [onClose] (which dispatches `CloseEditProfile`) for back/cancel, and via
 * [onSaved] when `savedProfile` flips non-null so the host can clear the result
 * and dismiss. Mirrors the iOS `EditProfileSheet`.
 */
@Composable
internal fun EditProfileScreen(
    draft: HighlighterEditProfileSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
    onClose: () -> Unit,
) {
    val context = LocalContext.current

    val picturePicker = rememberLauncherForActivityResult(
        ActivityResultContracts.PickVisualMedia(),
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        val image = readPickedImage(context, uri)
        if (image == null) {
            dispatch(HighlighterAppAction.EditProfileCapabilityFailed("Couldn't read that image."))
            return@rememberLauncherForActivityResult
        }
        dispatch(
            HighlighterAppAction.UploadEditProfileImage(
                HighlighterEditProfileImageTarget.PICTURE,
                image.bytes,
                image.mime,
                image.width,
                image.height,
                "",
            ),
        )
    }

    val bannerPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.PickVisualMedia(),
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        val image = readPickedImage(context, uri)
        if (image == null) {
            dispatch(HighlighterAppAction.EditProfileCapabilityFailed("Couldn't read that image."))
            return@rememberLauncherForActivityResult
        }
        dispatch(
            HighlighterAppAction.UploadEditProfileImage(
                HighlighterEditProfileImageTarget.BANNER,
                image.bytes,
                image.mime,
                image.width,
                image.height,
                "",
            ),
        )
    }

    val isBusy = draft.isSaving || draft.isPictureUploading || draft.isBannerUploading

    DestinationScaffold(title = "Edit profile", onBack = onClose) { _ ->
        Box(modifier = Modifier.fillMaxSize()) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 18.dp, vertical = 18.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                // Banner
                BannerPlate(draft = draft, dispatch = dispatch) {
                    bannerPicker.launch(
                        PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly),
                    )
                }

                // Avatar
                AvatarPlate(draft = draft, dispatch = dispatch) {
                    picturePicker.launch(
                        PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly),
                    )
                }

                ProfileField(
                    label = "Display name",
                    value = draft.displayName,
                    placeholder = "How you want to be addressed",
                    onValueChange = { dispatch(HighlighterAppAction.SetEditProfileDisplayName(it)) },
                )
                ProfileField(
                    label = "Username",
                    value = draft.name,
                    placeholder = "lowercase, no spaces",
                    onValueChange = { dispatch(HighlighterAppAction.SetEditProfileName(it)) },
                    capitalization = KeyboardCapitalization.None,
                )
                ProfileField(
                    label = "About",
                    value = draft.about,
                    placeholder = "A line or two — what do you read?",
                    onValueChange = { dispatch(HighlighterAppAction.SetEditProfileAbout(it)) },
                    singleLine = false,
                    minLines = 3,
                    maxLines = 8,
                )
                ProfileField(
                    label = "NIP-05",
                    value = draft.nip05,
                    placeholder = "you@example.com",
                    onValueChange = { dispatch(HighlighterAppAction.SetEditProfileNip05(it)) },
                    capitalization = KeyboardCapitalization.None,
                    keyboardType = KeyboardType.Email,
                )
                ProfileField(
                    label = "Website",
                    value = draft.website,
                    placeholder = "https://…",
                    onValueChange = { dispatch(HighlighterAppAction.SetEditProfileWebsite(it)) },
                    capitalization = KeyboardCapitalization.None,
                    keyboardType = KeyboardType.Uri,
                )
                ProfileField(
                    label = "Lightning address",
                    value = draft.lud16,
                    placeholder = "you@walletofsatoshi.com",
                    onValueChange = { dispatch(HighlighterAppAction.SetEditProfileLud16(it)) },
                    capitalization = KeyboardCapitalization.None,
                    keyboardType = KeyboardType.Email,
                )

                draft.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
                    Text(
                        text = message,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                    TextButton(onClick = { dispatch(HighlighterAppAction.ClearEditProfileError) }) {
                        Text("Dismiss")
                    }
                }

                Spacer(modifier = Modifier.height(8.dp))

                Button(
                    onClick = { dispatch(HighlighterAppAction.SubmitEditProfile) },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(14.dp),
                    enabled = !isBusy,
                ) {
                    if (draft.isSaving) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(20.dp),
                            strokeWidth = 2.dp,
                            color = MaterialTheme.colorScheme.onPrimary,
                        )
                    } else {
                        Text("Save")
                    }
                }

                Spacer(modifier = Modifier.height(24.dp))
            }
        }
    }
}

@Composable
private fun BannerPlate(
    draft: HighlighterEditProfileSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
    onPick: () -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(140.dp),
    ) {
        RemoteImage(
            url = draft.banner.takeIf { it.isNotBlank() },
            contentDescription = "Profile banner",
            modifier = Modifier.fillMaxSize(),
            shape = CoverShape,
        )
        Row(
            modifier = Modifier
                .align(Alignment.BottomEnd)
                .padding(10.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (draft.isBannerUploading) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp,
                )
            }
            if (draft.banner.isNotBlank()) {
                OutlinedButton(
                    onClick = { dispatch(HighlighterAppAction.SetEditProfileBanner("")) },
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text("Remove banner")
                }
            }
            OutlinedButton(
                onClick = onPick,
                shape = RoundedCornerShape(8.dp),
                enabled = !draft.isBannerUploading,
            ) {
                Text(if (draft.banner.isBlank()) "Add banner" else "Replace")
            }
        }
    }
}

@Composable
private fun AvatarPlate(
    draft: HighlighterEditProfileSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
    onPick: () -> Unit,
) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(contentAlignment = Alignment.Center) {
            AvatarImage(
                url = draft.picture.takeIf { it.isNotBlank() },
                name = draft.displayName.ifBlank { draft.name.ifBlank { "?" } },
                size = 88.dp,
            )
            if (draft.isPictureUploading) {
                CircularProgressIndicator(
                    modifier = Modifier.size(28.dp),
                    strokeWidth = 3.dp,
                )
            }
        }
        Spacer(modifier = Modifier.width(14.dp))
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            OutlinedButton(
                onClick = onPick,
                shape = RoundedCornerShape(8.dp),
                enabled = !draft.isPictureUploading,
            ) {
                Text(if (draft.picture.isBlank()) "Add photo" else "Replace photo")
            }
            if (draft.picture.isNotBlank()) {
                TextButton(onClick = { dispatch(HighlighterAppAction.SetEditProfilePicture("")) }) {
                    Text("Remove")
                }
            }
        }
    }
}

@Composable
private fun ProfileField(
    label: String,
    value: String,
    placeholder: String,
    onValueChange: (String) -> Unit,
    singleLine: Boolean = true,
    minLines: Int = 1,
    maxLines: Int = 1,
    capitalization: KeyboardCapitalization = KeyboardCapitalization.Sentences,
    keyboardType: KeyboardType = KeyboardType.Text,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = Modifier.fillMaxWidth(),
        label = { Text(label) },
        placeholder = { Text(placeholder) },
        singleLine = singleLine,
        minLines = minLines,
        maxLines = if (singleLine) 1 else maxLines,
        keyboardOptions = KeyboardOptions(
            capitalization = capitalization,
            keyboardType = keyboardType,
        ),
    )
}
