import type { ArtifactRecord } from '$lib/ndk/artifacts';
import type { HydratedHighlight } from '$lib/ndk/highlights';

export type HighlightSourceGroup = {
  referenceKey: string;
  artifact?: ArtifactRecord;
  highlights: HydratedHighlight[];
  latestActivityAt: number;
};

export function groupHighlightsBySource(
  highlights: HydratedHighlight[],
  artifactsByReference: Map<string, ArtifactRecord>
): HighlightSourceGroup[] {
  const groups = new Map<string, HighlightSourceGroup>();

  for (const highlight of highlights) {
    const referenceKey = highlight.sourceReferenceKey || `event:${highlight.eventId}`;
    const activityAt = highlight.latestSharedAt ?? highlight.createdAt ?? 0;
    const existing = groups.get(referenceKey);

    if (existing) {
      existing.highlights.push(highlight);
      existing.latestActivityAt = Math.max(existing.latestActivityAt, activityAt);
      continue;
    }

    groups.set(referenceKey, {
      referenceKey,
      artifact: artifactsByReference.get(highlight.sourceReferenceKey),
      highlights: [highlight],
      latestActivityAt: activityAt
    });
  }

  return [...groups.values()]
    .map((group) => ({
      ...group,
      highlights: [...group.highlights].toSorted(
        (left, right) =>
          (right.latestSharedAt ?? right.createdAt ?? 0) -
          (left.latestSharedAt ?? left.createdAt ?? 0)
      )
    }))
    .toSorted((left, right) => right.latestActivityAt - left.latestActivityAt);
}
