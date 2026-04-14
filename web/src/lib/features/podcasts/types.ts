export type PodcastTranscriptSegment = {
  id: string;
  startSeconds: number | null;
  endSeconds: number | null;
  speaker: string;
  text: string;
};

export type PodcastArtifactData = {
  episodeTitle: string;
  showTitle: string;
  description: string;
  image: string;
  publishedAt: string;
  durationSeconds: number | null;
  audioUrl: string;
  audioPreviewUrl: string;
  transcriptUrl: string;
  feedUrl: string;
  transcriptSource: string;
  transcriptAvailable: boolean;
  playbackAvailable: boolean;
  audioRestrictedReason: string;
  transcriptSegments: PodcastTranscriptSegment[];
};
