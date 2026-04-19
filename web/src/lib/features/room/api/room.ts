export interface RoomMember {
  pubkey: string;
  name: string;
  color: number;
}

export interface Artifact {
  id: string;
  type: 'book' | 'podcast' | 'article';
  title: string;
  author: string;
  coverUrl: string;
  url: string;
  progress: number; // 0-100
  highlightCount: number;
}

export interface Highlight {
  id: string;
  artifactId: string;
  text: string;
  note?: string;
  author: string;
  createdAt: string;
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
}

export async function getRoom(slug: string): Promise<Room | null> {
  // TODO: wire to real API
  // For now, return null to indicate "not yet connected"
  void slug;
  return null;
}
