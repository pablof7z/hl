package com.highlighter.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import kotlinx.coroutines.delay
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import com.highlighter.app.PendingShare
import com.highlighter.app.ui.auth.CreateAccountScreen
import com.highlighter.app.ui.auth.LoginScreen
import com.highlighter.app.ui.auth.OnboardingInterestsScreen
import com.highlighter.app.ui.auth.WelcomeScreen
import com.highlighter.app.ui.bookmarks.CurationMenuSheet
import com.highlighter.app.ui.comments.CommentsPanel
import com.highlighter.app.ui.components.ToastBanner
import com.highlighter.app.ui.profile.EditProfileScreen
import com.highlighter.app.ui.profile.ProfilePanel
import com.highlighter.app.ui.reader.ArticleReaderPanel
import com.highlighter.app.ui.rooms.RoomDetailPanel
import com.highlighter.app.ui.rooms.RoomInvitePanel
import com.highlighter.app.ui.share.ShareComposerPanel
import com.highlighter.app.ui.whatsnew.WhatsNewDialog
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterAppState

/** Logged-out auth routing target (only meaningful while signed out). */
private enum class AuthRoute { WELCOME, LOGIN, CREATE_ACCOUNT }

/**
 * Root gating, mirroring the iOS `RootSceneView`:
 *  - loggedIn && onboarding complete  -> [MainScaffold]
 *  - loggedIn (onboarding incomplete) -> [OnboardingInterestsScreen]
 *  - !loggedIn && onboarding complete -> Login (with Welcome/CreateAccount)
 *  - else                             -> Welcome (fresh install)
 *
 * Core-state-driven overlays (comments, room invite, room detail, article
 * reader, profile, feedback thread) layer on top of everything via the
 * [overlays] block. The toast host and What's New dialog also live here so
 * they sit above the active scene rather than inside a scroll.
 */
@Composable
internal fun RootScene(
    state: HighlighterAppState,
    pendingShare: PendingShare?,
    dispatch: (HighlighterAppAction) -> Unit,
    onDismissShare: () -> Unit,
) {
    val loggedIn = state.chrome.currentUser != null
    val onboardingComplete = state.onboarding.isComplete

    // Logged-out routing between Welcome / Login / Create account. Defaults to
    // Login when onboarding is already complete (returning user), else Welcome.
    var authRoute by remember(loggedIn, onboardingComplete) {
        mutableStateOf(if (onboardingComplete) AuthRoute.LOGIN else AuthRoute.WELCOME)
    }

    Box(modifier = Modifier.fillMaxSize().semantics { testTagsAsResourceId = true }) {
        Crossfade(
            targetState = Triple(loggedIn, onboardingComplete, authRoute),
            animationSpec = tween(250),
            label = "rootScene",
        ) { (isLoggedIn, isOnboarded, route) ->
            when {
                isLoggedIn && isOnboarded ->
                    MainScaffold(state = state, dispatch = dispatch)

                isLoggedIn ->
                    OnboardingInterestsScreen(onboarding = state.onboarding, dispatch = dispatch)

                else -> when (route) {
                    AuthRoute.WELCOME -> WelcomeScreen(
                        onCreateAccount = { authRoute = AuthRoute.CREATE_ACCOUNT },
                        onSignIn = { authRoute = AuthRoute.LOGIN },
                    )
                    AuthRoute.LOGIN -> LoginScreen(
                        auth = state.auth,
                        onBack = { authRoute = AuthRoute.WELCOME },
                        dispatch = dispatch,
                    )
                    AuthRoute.CREATE_ACCOUNT -> CreateAccountScreen(
                        createAccount = state.createAccount,
                        onBack = { authRoute = AuthRoute.WELCOME },
                        dispatch = dispatch,
                    )
                }
            }
        }

        // Edit-profile presentation is host-held (the Rust snapshot carries no
        // open flag) — mirrors iOS, where the sheet's lifecycle dispatches
        // open/close. The flag is reset whenever the profile overlay closes.
        var editProfileOpen by remember(state.profileView.pubkeyHex) { mutableStateOf(false) }

        // Core-state-driven overlays — full-screen, layered above the scene.
        Overlays(
            state = state,
            dispatch = dispatch,
            onEditProfile = {
                dispatch(HighlighterAppAction.OpenEditProfile(state.profileView.profile))
                editProfileOpen = true
            },
        )

        // Edit profile — host-held overlay above the profile overlay. Closes on
        // back/cancel (dispatching CloseEditProfile) and auto-dismisses once the
        // core reports a saved profile.
        if (editProfileOpen) {
            val closeEditProfile = {
                dispatch(HighlighterAppAction.CloseEditProfile)
                editProfileOpen = false
            }
            BackHandler { closeEditProfile() }
            // Save succeeded — clear the result projection and dismiss.
            LaunchedEffect(state.editProfile.savedProfile) {
                if (state.editProfile.savedProfile != null) {
                    dispatch(HighlighterAppAction.ClearEditProfileResult)
                    editProfileOpen = false
                }
            }
            EditProfileScreen(
                draft = state.editProfile,
                dispatch = dispatch,
                onClose = closeEditProfile,
            )
        }

        // Share composer (host-held overlay state) — full-screen above all else.
        if (pendingShare != null) {
            BackHandler { onDismissShare() }
            DestinationScaffold(title = "Share", onBack = onDismissShare) { _ ->
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(18.dp),
                ) {
                    item {
                        ShareComposerPanel(
                            share = pendingShare,
                            composer = state.shareComposer,
                            communities = state.chrome.joinedCommunities,
                            dispatch = dispatch,
                            onClose = onDismissShare,
                        )
                    }
                }
            }
        }

        // Toast host — positioned below the top app bar so it never overlaps
        // the status line, avatar, or gear. Uses WindowInsets.statusBars to
        // skip the system status bar height, plus the standard M3 TopAppBar
        // height (64.dp), then an 8.dp gap, matching how iOS places its
        // ShareToastBanner below the navigation bar.
        state.toast?.let { toast ->
            // Auto-expire: keyed on the toast object so each new toast resets
            // the timer. After 4 s the banner clears itself without user action.
            LaunchedEffect(toast) {
                delay(4_000)
                dispatch(HighlighterAppAction.ClearToast)
            }
            val statusBarTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(
                        top = statusBarTop + 64.dp + 8.dp,
                        start = 16.dp,
                        end = 16.dp,
                    ),
                contentAlignment = Alignment.TopCenter,
            ) {
                ToastBanner(
                    message = toast.message,
                    onClearToast = { dispatch(HighlighterAppAction.ClearToast) },
                )
            }
        }
    }

    // What's New is a dialog on top of the whole app, core-state driven.
    if (state.whatsNew.entries.isNotEmpty()) {
        WhatsNewDialog(entries = state.whatsNew.entries, dispatch = dispatch)
    }

    // Curation (collections) picker — a bottom sheet above the whole app,
    // core-state driven (open while its article address is non-blank).
    CurationMenuSheet(menu = state.curationMenu, dispatch = dispatch)
}

/**
 * The core-state-driven overlay stack. Each becomes a full-screen destination
 * with its own back arrow when its state key is non-blank. System back is
 * routed to the innermost open overlay in the same order the iOS/legacy back
 * chain used: comments -> roomInvite -> roomDetail -> articleReader ->
 * profile -> feedbackThread.
 */
@Composable
private fun Overlays(
    state: HighlighterAppState,
    dispatch: (HighlighterAppAction) -> Unit,
    onEditProfile: () -> Unit,
) {
    // System back closes the innermost open overlay first.
    val backAction = when {
        state.comments.rootTagValue.isNotBlank() -> HighlighterAppAction.CloseComments
        state.roomInvite.groupId.isNotBlank() -> HighlighterAppAction.CloseRoomInvite
        state.roomDetail.groupId.isNotBlank() -> HighlighterAppAction.CloseRoom
        state.articleReader.address.isNotBlank() -> HighlighterAppAction.CloseArticleReader
        state.profileView.pubkeyHex.isNotBlank() -> HighlighterAppAction.CloseProfile
        state.feedback.selectedRootEventId != null -> HighlighterAppAction.CloseFeedbackThread
        else -> null
    }
    if (backAction != null) {
        BackHandler { dispatch(backAction) }
    }

    // Render outermost-first so the innermost open overlay paints on top.
    if (state.profileView.pubkeyHex.isNotBlank()) {
        OverlayDestination(title = "Profile", onBack = { dispatch(HighlighterAppAction.CloseProfile) }) {
            ProfilePanel(
                profile = state.profileView,
                dispatch = dispatch,
                onEditProfile = onEditProfile,
            )
        }
    }
    if (state.articleReader.address.isNotBlank()) {
        OverlayDestination(title = "Reader", onBack = { dispatch(HighlighterAppAction.CloseArticleReader) }) {
            ArticleReaderPanel(snapshot = state.articleReader, dispatch = dispatch)
        }
    }
    if (state.roomDetail.groupId.isNotBlank()) {
        val roomName = state.chrome.joinedCommunities
            .firstOrNull { it.id == state.roomDetail.groupId }
            ?.name?.takeIf { it.isNotBlank() }
            ?: "Room"
        RoomDetailPanel(
            room = state.roomDetail,
            roomName = roomName,
            onBack = { dispatch(HighlighterAppAction.CloseRoom) },
            dispatch = dispatch,
        )
    }
    if (state.roomInvite.groupId.isNotBlank()) {
        OverlayDestination(title = "Invite", onBack = { dispatch(HighlighterAppAction.CloseRoomInvite) }) {
            RoomInvitePanel(invite = state.roomInvite, dispatch = dispatch)
        }
    }
    if (state.comments.rootTagValue.isNotBlank()) {
        OverlayDestination(title = "Comments", onBack = { dispatch(HighlighterAppAction.CloseComments) }) {
            CommentsPanel(comments = state.comments, dispatch = dispatch)
        }
    }
}

@Composable
private fun OverlayDestination(
    title: String,
    onBack: () -> Unit,
    content: @Composable () -> Unit,
) {
    DestinationScaffold(title = title, onBack = onBack) { _ ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(0.dp),
            contentPadding = PaddingValues(18.dp),
        ) {
            item { content() }
        }
    }
}
