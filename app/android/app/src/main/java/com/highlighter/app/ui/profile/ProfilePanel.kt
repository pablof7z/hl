package com.highlighter.app.ui.profile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.AvatarImage
import com.highlighter.app.ui.components.Chip
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.search.ArticleSearchRow
import com.highlighter.app.ui.search.CommunitySearchRow
import com.highlighter.app.ui.search.HighlightSearchRow
import com.highlighter.app.ui.search.SearchGroupHeader
import com.highlighter.app.util.displayName
import uniffi.highlighter_core.CommunitySummary
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterProfileViewSnapshot

@Composable
internal fun ProfilePanel(
    profile: HighlighterProfileViewSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
    onEditProfile: () -> Unit = {},
) {
    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SectionHeader("Profile", profile.pubkeyHex.take(8))
        }
        profile.profile?.banner?.takeIf { it.isNotBlank() }?.let { banner ->
            Spacer(modifier = Modifier.height(10.dp))
            RemoteImage(
                url = banner,
                contentDescription = "Profile banner",
                modifier = Modifier
                    .fillMaxWidth()
                    .height(110.dp),
                shape = CoverShape,
            )
        }
        Spacer(modifier = Modifier.height(10.dp))
        Row(verticalAlignment = Alignment.Top) {
            AvatarImage(
                url = profile.profile?.picture,
                name = profile.displayName(),
                size = 56.dp,
            )
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = profile.displayName(),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                profile.profile?.nip05?.takeIf { it.isNotBlank() }?.let { nip05 ->
                    Spacer(modifier = Modifier.height(3.dp))
                    Text(
                        text = nip05.removePrefix("_@"),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                profile.profile?.about?.takeIf { it.isNotBlank() }?.let { about ->
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        text = about,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 4,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
        Spacer(modifier = Modifier.height(12.dp))
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            ProfileStat("Writing", profile.articleCount.toString(), Modifier.weight(1f))
            ProfileStat("Highlights", profile.highlightCount.toString(), Modifier.weight(1f))
            ProfileStat("Rooms", profile.communityCount.toString(), Modifier.weight(1f))
        }
        Spacer(modifier = Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.RefreshProfile) },
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
                enabled = !profile.isLoading && !profile.isMutatingFollow,
            ) {
                Text(if (profile.isLoading) "Refreshing" else "Refresh")
            }
            if (profile.isOwnProfile) {
                Button(
                    onClick = onEditProfile,
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text("Edit profile")
                }
            } else if (profile.viewerPubkeyHex != null) {
                Button(
                    onClick = { dispatch(HighlighterAppAction.ToggleProfileFollow) },
                    shape = RoundedCornerShape(8.dp),
                    enabled = !profile.isMutatingFollow,
                ) {
                    Text(
                        when {
                            profile.isMutatingFollow -> "Saving"
                            profile.isFollowing -> "Following"
                            else -> "Follow"
                        },
                    )
                }
            }
        }
        profile.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
        }
        if (profile.articles.isNotEmpty()) {
            SearchGroupHeader("Writing", profile.articleCount.toString())
            profile.articles.take(3).forEach { article ->
                ArticleSearchRow(article, dispatch)
            }
        }
        if (profile.highlights.isNotEmpty()) {
            SearchGroupHeader("Highlights", profile.highlightCount.toString())
            profile.highlights.take(3).forEach { highlight ->
                HighlightSearchRow(highlight)
            }
        }
        if (profile.communities.isNotEmpty()) {
            SearchGroupHeader("Communities", profile.communityCount.toString())
            profile.communities.take(3).forEach { community ->
                CommunitySearchRow(community)
            }
        }
        if (profile.isLoading &&
            profile.articles.isEmpty() &&
            profile.highlights.isEmpty() &&
            profile.communities.isEmpty()
        ) {
            Spacer(modifier = Modifier.height(10.dp))
            Text(
                text = "Loading profile",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun ProfileStat(label: String, value: String, modifier: Modifier = Modifier) {
    Column(modifier = modifier) {
        Text(text = value, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
@OptIn(ExperimentalLayoutApi::class)
internal fun CommunityRow(
    community: CommunitySummary,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Panel {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { dispatch(HighlighterAppAction.OpenRoom(community.id)) },
            verticalAlignment = Alignment.Top,
        ) {
            AvatarImage(
                url = community.picture,
                name = community.name.ifBlank { "#" },
                size = 40.dp,
            )
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = community.name.ifBlank { community.id },
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (community.about.isNotBlank()) {
                    Spacer(modifier = Modifier.height(3.dp))
                    Text(
                        text = community.about,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Spacer(modifier = Modifier.height(8.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Chip(community.access)
                    Chip(community.visibility)
                    community.memberCount?.let { Chip("$it members") }
                }
            }
        }
    }
}
