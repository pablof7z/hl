import type { NostrEvent } from '@nostr-dev-kit/ndk';
import type { PageServerLoad } from './$types';
import { fetchHighlightComments, fetchHighlightWithAuthor } from '$lib/server/nostr';
import { profileIdentifier } from '$lib/ndk/format';
import { buildHighlightSeo, buildMissingSeo } from '$lib/seo';

export const load: PageServerLoad = async ({ params, setHeaders, url }) => {
  setHeaders({
    'cache-control': 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600'
  });

  try {
    const {
      event,
      author,
      profile,
      sourceTitle,
      sourceAuthorPubkey,
      source,
      pageImageUrl
    } = await fetchHighlightWithAuthor(params.nevent);

    if (!event || !author) {
      return {
        missing: true,
        seo: buildMissingSeo(url, 'Highlight not found')
      };
    }

    const comments = event.id ? await fetchHighlightComments(event.id) : [];

    return {
      missing: false,
      event: event.rawEvent() as NostrEvent,
      authorPubkey: author.pubkey,
      authorIdentifier: profileIdentifier(profile, author.npub),
      authorNpub: author.npub,
      profile,
      sourceTitle,
      sourceAuthorPubkey,
      source,
      pageImageUrl,
      comments: comments.map((c) => c.rawEvent() as NostrEvent),
      seo: buildHighlightSeo({
        url,
        identifier: params.nevent,
        event: event.rawEvent() as NostrEvent,
        authorPubkey: author.pubkey,
        profile,
        sourceTitle
      })
    };
  } catch (error) {
    console.warn('Highlight SSR load failed', error);

    return {
      missing: true,
      seo: buildMissingSeo(url, 'Highlight unavailable')
    };
  }
};
