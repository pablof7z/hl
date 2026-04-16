import type { NDKEvent, NDKFilter } from '@nostr-dev-kit/ndk';

export type CommentNode = {
  event: NDKEvent;
  children: CommentNode[];
};

export function tagValue(tags: string[][], name: string): string {
  return tags.find((tag) => tag[0] === name)?.[1]?.trim() ?? '';
}

export function commentParentReference(comment: NDKEvent): string {
  return tagValue(comment.tags, 'a') || tagValue(comment.tags, 'e') || tagValue(comment.tags, 'i');
}

function sortCommentNodes(
  nodes: CommentNode[],
  direction: 'asc' | 'desc'
): CommentNode[] {
  const sorted = [...nodes].sort((left, right) =>
    direction === 'asc'
      ? (left.event.created_at ?? 0) - (right.event.created_at ?? 0)
      : (right.event.created_at ?? 0) - (left.event.created_at ?? 0)
  );

  for (const node of sorted) {
    node.children = sortCommentNodes(node.children, 'asc');
  }

  return sorted;
}

export function targetReferences(target: NDKEvent): Set<string> {
  const references = new Set<string>();
  const tagId = target.tagId();

  if (tagId) references.add(tagId);
  if (target.id) references.add(target.id);

  return references;
}

export function buildReferenceFilters(
  references: Set<string>,
  kinds: number[],
  options: {
    addressTag: string;
    idTag: string;
    limit: number;
  }
): NDKFilter[] {
  const ids: string[] = [];
  const addresses: string[] = [];

  for (const reference of references) {
    if (reference.includes(':')) addresses.push(reference);
    else ids.push(reference);
  }

  const filters: NDKFilter[] = [];

  if (addresses.length > 0) {
    const filter = { kinds, limit: options.limit } as NDKFilter & Record<`#${string}`, string[]>;
    filter[`#${options.addressTag}`] = addresses;
    filters.push(filter);
  }

  if (ids.length > 0) {
    const filter = { kinds, limit: options.limit } as NDKFilter & Record<`#${string}`, string[]>;
    filter[`#${options.idTag}`] = ids;
    filters.push(filter);
  }

  return filters;
}

export function buildArticleCommentTree(
  commentEvents: NDKEvent[],
  articleAddress: string,
  articleEventId: string
): CommentNode[] {
  const rootReferences = new Set([articleAddress, articleEventId].filter(Boolean));
  const nodes = commentEvents.map((comment) => ({
    event: comment,
    parentReference: commentParentReference(comment),
    children: [] as CommentNode[]
  }));
  const nodesByReference = new Map<string, CommentNode>();

  for (const node of nodes) {
    nodesByReference.set(node.event.id, node);
    nodesByReference.set(node.event.tagId(), node);
  }

  const roots: CommentNode[] = [];

  for (const node of nodes) {
    if (!node.parentReference || rootReferences.has(node.parentReference)) {
      roots.push(node);
      continue;
    }

    const parent = nodesByReference.get(node.parentReference);
    if (!parent || parent === node) {
      roots.push(node);
      continue;
    }

    parent.children.push(node);
  }

  return sortCommentNodes(roots, 'desc');
}
