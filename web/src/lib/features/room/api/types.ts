import type { NDKEvent, NDKKind as NDKKindType } from '@nostr-dev-kit/ndk';

export interface RoomMember {
  pubkey: string;
  colorIndex: number; // positional color (1..6), based on member list order
  joinedAt: string;
}

export interface Artifact {
  id: string;
  type: 'book' | 'podcast' | 'article' | 'essay' | 'video';
  title: string;
  author: string;
  cover: string;
  url: string;
  progress: number; // 0-100
  highlightCount: number;
  discussionCount: number;
}

export interface Highlight {
  id: string;
  artifactId: string;
  quote: string;
  authorPubkey: string;
  authorColorIndex: number;   // positional, from room member list order
  createdAt: number;          // raw unix seconds; components format for display
}

export interface Note {
  id: string;
  authorPubkey: string;
  authorColorIndex: number;
  content: string;
  createdAt: number;
}

export interface UpNextItem {
  id: string;
  title: string;
  type: 'book' | 'podcast' | 'article';
  voterCount: number;
  voterColors: number[];
}

export interface Room {
  id: string;
  name: string;
  members: RoomMember[];
  pinnedArtifact?: Artifact;
  artifacts: Artifact[];
  highlights: Highlight[];
  upNext: UpNextItem[];
  notes: Note[];
}

// kind:999 — made-up pin/vote kind (see decision D-01 in room-ui.md)
export const KIND_PIN = 999 as NDKKindType;

const ARTIFACT_TYPE_TAG_VALUES: Artifact['type'][] = [
  'book',
  'podcast',
  'article',
  'essay',
  'video'
];

export function artifactFromThreadEvent(event: NDKEvent): Artifact {
  const title = event.tagValue('title') || event.tagValue('name') || 'Untitled';
  const author = event.tagValue('author') || event.tagValue('summary') || '';
  const url = event.tagValue('r') || event.tagValue('url') || '';
  const typeRaw = event.tagValue('type') || '';
  const type: Artifact['type'] = ARTIFACT_TYPE_TAG_VALUES.includes(typeRaw as Artifact['type'])
    ? (typeRaw as Artifact['type'])
    : 'article';

  return {
    id: event.id,
    type,
    title,
    author,
    cover: event.tagValue('image') || event.tagValue('picture') || '',
    url,
    progress: 0,
    highlightCount: 0,
    discussionCount: 0
  };
}

export function sortByCreatedAtDesc(events: NDKEvent[]): NDKEvent[] {
  return events.sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
}
