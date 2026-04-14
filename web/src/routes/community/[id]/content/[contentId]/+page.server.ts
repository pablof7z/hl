import type { NDKUserProfile, NostrEvent } from '@nostr-dev-kit/ndk';
import type { PodcastArtifactData } from '$lib/features/podcasts/types';
import type { PageServerLoad } from './$types';
import { profileIdentifier } from '$lib/ndk/format';
import {
  fetchArtifactForGroup,
  fetchNostrArticleForArtifact,
  fetchPodcastExperienceForArtifact
} from '$lib/server/artifacts';
import { fetchCommunityById } from '$lib/server/communities';

export const load: PageServerLoad = async ({ params, setHeaders }) => {
  setHeaders({
    'cache-control': 'public, max-age=30, s-maxage=120, stale-while-revalidate=600'
  });

  const [community, artifact] = await Promise.all([
    fetchCommunityById(params.id),
    fetchArtifactForGroup(params.id, params.contentId)
  ]);

  let articleEvent: NostrEvent | undefined;
  let articleAuthorPubkey = '';
  let articleAuthorIdentifier = '';
  let articleAuthorNpub = '';
  let articleProfile: NDKUserProfile | undefined;
  let podcast: PodcastArtifactData | undefined;

  if (artifact) {
    try {
      const { event, author, profile } = await fetchNostrArticleForArtifact(artifact);
      articleEvent = event?.rawEvent() as NostrEvent | undefined;
      articleAuthorPubkey = author?.pubkey ?? '';
      articleAuthorIdentifier = author ? profileIdentifier(profile, author.npub) : '';
      articleAuthorNpub = author?.npub ?? '';
      articleProfile = profile;
    } catch (error) {
      console.warn('Community artifact article SSR load failed', {
        groupId: params.id,
        contentId: params.contentId,
        error
      });
    }

    try {
      podcast = await fetchPodcastExperienceForArtifact(artifact);
    } catch (error) {
      console.warn('Community artifact podcast SSR load failed', {
        groupId: params.id,
        contentId: params.contentId,
        error
      });
    }
  }

  return {
    community,
    artifact,
    podcast,
    articleEvent,
    articleAuthorPubkey,
    articleAuthorIdentifier,
    articleAuthorNpub,
    articleProfile,
    groupId: params.id,
    contentId: params.contentId,
    missing: !community || !artifact
  };
};
