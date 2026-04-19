export interface RoomMember {
  pubkey: string;
  name: string;
  colorIndex: number; // renamed from color — matches component expectations
  joinedAt: string;
}

export interface Artifact {
  id: string;
  type: 'book' | 'podcast' | 'article' | 'essay' | 'video';
  title: string;
  author: string;
  cover: string; // renamed from coverUrl — matches component expectations
  url: string;
  progress: number; // 0-100
  highlightCount: number;
  discussionCount: number; // added — displayed by ArtifactCard
}

export interface Highlight {
  id: string;
  artifactId: string;
  quote: string;          // renamed from text
  memberName: string;     // renamed from author
  memberColorIndex: number;
  timestamp: string;      // renamed from createdAt
}

export interface Note {
  id: string;
  memberColorIndex: number;
  memberName: string;
  content: string;
  timestamp: string;
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

export async function getRoom(slug: string): Promise<Room | null> {
  // TODO: wire to real API
  // For now, return null to indicate "not yet connected"
  void slug;
  return null;
}
