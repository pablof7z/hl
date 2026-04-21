import { Buffer } from 'node:buffer';
import { normalizeArtifactUrl, type ArtifactRecord } from '$lib/ndk/artifacts';
import type { PodcastArtifactData, PodcastTranscriptSegment } from '$lib/features/podcasts/types';
import { decodeHtmlEntities } from '$lib/utils/html';

const FETCH_TIMEOUT_MS = 8000;
const MAX_HTML_CHARS = 350_000;
const MAX_FEED_CHARS = 750_000;
const MAX_TRANSCRIPT_CHARS = 750_000;

type SchemaNode = Record<string, unknown>;

type PodcastPageMetadata = {
  canonicalUrl: string;
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
  podcastGuid: string;
  catalogId: string;
  catalogKind: string;
  audioRestrictedReason: string;
  inlineTranscriptSegments: PodcastTranscriptSegment[];
};

type PodcastFeed = {
  title: string;
  image: string;
  transcriptUrl: string;
  items: PodcastFeedItem[];
};

type PodcastFeedItem = {
  guid: string;
  title: string;
  link: string;
  description: string;
  audioUrl: string;
  transcriptUrl: string;
  publishedAt: string;
  durationSeconds: number | null;
  image: string;
};

export function extractPodcastMetadataFromHtml(html: string, responseUrl: string): PodcastPageMetadata {
  const canonicalUrl =
    resolveUrl(linkHref(html, 'canonical'), responseUrl) ||
    resolveUrl(metaContent(html, 'property', 'og:url'), responseUrl) ||
    responseUrl;
  const jsonLdNodes = parseJsonLdNodes(html);
  const podcastNodes = jsonLdNodes.filter((node) => {
    const types = schemaTypes(node);
    return (
      types.includes('podcastepisode') ||
      types.includes('podcastseries') ||
      types.includes('audioobject')
    );
  });
  const spotify = extractSpotifyEpisodeData(html, canonicalUrl);
  const apple = extractAppleEpisodeData(html, canonicalUrl);
  const genericSchema = extractSchemaEpisodeData(podcastNodes, canonicalUrl);
  const providerHost = hostnameLabel(canonicalUrl);
  const inlineTranscriptSegments = extractInlineTranscriptSegments(html);
  const transcriptUrl = firstText([
    apple.transcriptUrl,
    genericSchema.transcriptUrl,
    spotify.transcriptUrl,
    findTranscriptLink(html, canonicalUrl)
  ]);
  const showTitle = firstText([
    apple.showTitle,
    spotify.showTitle,
    genericSchema.showTitle,
    metaContent(html, 'property', 'og:site_name')
  ]);
  const episodeTitle = firstText([
    apple.episodeTitle,
    spotify.episodeTitle,
    genericSchema.episodeTitle,
    metaContent(html, 'property', 'og:title'),
    metaContent(html, 'name', 'twitter:title'),
    textContent(html, 'title')
  ]);
  const description = firstText([
    apple.description,
    spotify.description,
    genericSchema.description,
    metaContent(html, 'name', 'description'),
    metaContent(html, 'property', 'og:description'),
    metaContent(html, 'name', 'twitter:description')
  ]);
  const image = firstText([
    apple.image,
    spotify.image,
    genericSchema.image,
    resolveUrl(metaContent(html, 'property', 'og:image'), canonicalUrl),
    resolveUrl(metaContent(html, 'name', 'twitter:image'), canonicalUrl)
  ]);
  const publishedAt = firstText([
    apple.publishedAt,
    spotify.publishedAt,
    genericSchema.publishedAt,
    metaContent(html, 'property', 'article:published_time'),
    metaContent(html, 'name', 'music:release_date')
  ]);
  const durationSecondsCandidate = firstNumber(
    providerHost.includes('spotify.com')
      ? [
          spotify.durationSeconds,
          genericSchema.durationSeconds,
          numericString(metaContent(html, 'name', 'music:duration'))
        ]
      : providerHost.includes('podcasts.apple.com')
        ? [
            apple.durationSeconds,
            genericSchema.durationSeconds,
            numericString(metaContent(html, 'name', 'music:duration'))
          ]
        : [
            apple.durationSeconds,
            spotify.durationSeconds,
            genericSchema.durationSeconds,
            numericString(metaContent(html, 'name', 'music:duration'))
          ]
  );
  const durationSeconds =
    durationSecondsCandidate != null && durationSecondsCandidate > 0 ? durationSecondsCandidate : null;
  const feedUrl = firstText([
    apple.feedUrl,
    genericSchema.feedUrl,
    resolveUrl(linkHrefByType(html, 'application/rss+xml'), canonicalUrl),
    resolveUrl(linkHrefByType(html, 'application/atom+xml'), canonicalUrl),
    resolveUrl(metaContent(html, 'name', 'itunes:new-feed-url'), canonicalUrl)
  ]);
  const podcastGuid = firstText([
    apple.podcastGuid,
    genericSchema.podcastGuid,
    metaContent(html, 'name', 'podcast:guid'),
    metaContent(html, 'property', 'podcast:guid'),
    extractPodcastGuidFromText(html)
  ]);
  const audioUrl = firstText([
    apple.audioUrl,
    genericSchema.audioUrl,
    extractHtmlAudioSource(html, canonicalUrl)
  ]);
  const audioPreviewUrl = firstText([
    spotify.audioPreviewUrl,
    hostnameLabel(canonicalUrl).includes('spotify.com')
      ? resolveUrl(metaContent(html, 'property', 'og:audio'), canonicalUrl)
      : ''
  ]);
  const identity = resolvePodcastCatalogIdentity(canonicalUrl, podcastGuid);

  return {
    canonicalUrl,
    episodeTitle: cleanTitle(episodeTitle, canonicalUrl),
    showTitle,
    description: normalizeWhitespace(stripHtml(description)),
    image,
    publishedAt,
    durationSeconds,
    audioUrl,
    audioPreviewUrl,
    transcriptUrl,
    feedUrl,
    podcastGuid,
    catalogId: identity.catalogId,
    catalogKind: identity.catalogKind,
    audioRestrictedReason:
      !audioUrl && hostnameLabel(canonicalUrl).includes('spotify.com')
        ? 'Spotify does not expose a full episode stream to this page.'
        : '',
    inlineTranscriptSegments
  };
}

export async function fetchPodcastExperienceForArtifact(
  artifact: Pick<
    ArtifactRecord,
    | 'url'
    | 'title'
    | 'description'
    | 'image'
    | 'publishedAt'
    | 'durationSeconds'
    | 'audioUrl'
    | 'audioPreviewUrl'
    | 'transcriptUrl'
    | 'feedUrl'
    | 'podcastGuid'
    | 'podcastShowTitle'
    | 'catalogId'
    | 'catalogKind'
    | 'domain'
  >
): Promise<PodcastArtifactData> {
  const normalizedUrl = normalizeArtifactUrl(artifact.url) ?? artifact.url;
  let mergedUrl = normalizedUrl;
  let episodeTitle = cleanTitle(artifact.title, normalizedUrl);
  let showTitle = cleanText(artifact.podcastShowTitle);
  let description = normalizeWhitespace(cleanText(artifact.description));
  let image = cleanText(artifact.image);
  let publishedAt = cleanText(artifact.publishedAt);
  let durationSeconds = sanitizeNumber(artifact.durationSeconds);
  let audioUrl = cleanText(artifact.audioUrl);
  let audioPreviewUrl = cleanText(artifact.audioPreviewUrl);
  let transcriptUrl = cleanText(artifact.transcriptUrl);
  let feedUrl = cleanText(artifact.feedUrl);
  let podcastGuid = cleanText(artifact.podcastGuid);
  let audioRestrictedReason = '';
  let inlineTranscriptSegments: PodcastTranscriptSegment[] = [];

  if (normalizedUrl) {
    const html = await fetchText(normalizedUrl, MAX_HTML_CHARS).catch(() => '');
    if (html) {
      const htmlMetadata = extractPodcastMetadataFromHtml(html, normalizedUrl);
      mergedUrl = htmlMetadata.canonicalUrl || mergedUrl;
      episodeTitle = preferText(episodeTitle, htmlMetadata.episodeTitle, artifact.domain);
      showTitle = preferText(showTitle, htmlMetadata.showTitle, '');
      description = preferText(description, htmlMetadata.description, artifact.domain);
      image = preferUrl(image, htmlMetadata.image);
      publishedAt = preferValue(publishedAt, htmlMetadata.publishedAt);
      durationSeconds = preferNumber(durationSeconds, htmlMetadata.durationSeconds);
      audioUrl = preferUrl(audioUrl, htmlMetadata.audioUrl);
      audioPreviewUrl = preferUrl(audioPreviewUrl, htmlMetadata.audioPreviewUrl);
      transcriptUrl = preferUrl(transcriptUrl, htmlMetadata.transcriptUrl);
      feedUrl = preferUrl(feedUrl, htmlMetadata.feedUrl);
      podcastGuid = preferValue(podcastGuid, htmlMetadata.podcastGuid);
      audioRestrictedReason = preferValue(audioRestrictedReason, htmlMetadata.audioRestrictedReason);
      inlineTranscriptSegments = htmlMetadata.inlineTranscriptSegments;
    }
  }

  if (feedUrl) {
    const feedXml = await fetchText(feedUrl, MAX_FEED_CHARS).catch(() => '');
    if (feedXml) {
      const feed = parsePodcastFeed(feedXml);
      const matchedItem = selectFeedItem(feed, {
        url: mergedUrl,
        title: episodeTitle,
        publishedAt,
        podcastGuid,
        catalogId: artifact.catalogId
      });

      if (matchedItem) {
        episodeTitle = preferText(episodeTitle, matchedItem.title, artifact.domain);
        showTitle = preferText(showTitle, feed.title, '');
        description = preferText(description, matchedItem.description, artifact.domain);
        image = preferUrl(image, matchedItem.image || feed.image);
        publishedAt = preferValue(publishedAt, matchedItem.publishedAt);
        durationSeconds = preferNumber(durationSeconds, matchedItem.durationSeconds);
        audioUrl = preferUrl(audioUrl, matchedItem.audioUrl);
        transcriptUrl = preferUrl(transcriptUrl, matchedItem.transcriptUrl || feed.transcriptUrl);
        podcastGuid = preferValue(podcastGuid, matchedItem.guid);
      } else {
        showTitle = preferText(showTitle, feed.title, '');
        image = preferUrl(image, feed.image);
        transcriptUrl = preferUrl(transcriptUrl, feed.transcriptUrl);
      }
    }
  }

  const transcriptSegments =
    transcriptUrl
      ? await fetchAndParseTranscript(transcriptUrl).catch(() => [])
      : [];
  const resolvedTranscriptSegments =
    transcriptSegments.length > 0 ? transcriptSegments : inlineTranscriptSegments;

  if (!audioUrl && audioPreviewUrl && !audioRestrictedReason) {
    audioRestrictedReason = 'Only a short preview is exposed for this episode source.';
  }

  return {
    episodeTitle: episodeTitle || 'Untitled episode',
    showTitle,
    description,
    image,
    publishedAt,
    durationSeconds,
    audioUrl,
    audioPreviewUrl,
    transcriptUrl,
    feedUrl,
    transcriptSource: transcriptUrl || (resolvedTranscriptSegments.length > 0 ? mergedUrl : ''),
    transcriptAvailable: resolvedTranscriptSegments.length > 0,
    playbackAvailable: Boolean(audioUrl),
    audioRestrictedReason,
    transcriptSegments: resolvedTranscriptSegments
  };
}

async function fetchAndParseTranscript(transcriptUrl: string): Promise<PodcastTranscriptSegment[]> {
  const response = await fetch(transcriptUrl, {
    headers: {
      accept: 'text/plain,text/html,text/vtt,application/json,application/xml,text/xml',
      'user-agent': 'HighlighterBot/0.2 (+https://beta.highlighter.com)'
    },
    redirect: 'follow',
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS)
  });

  if (!response.ok) {
    throw new Error(`Transcript responded with ${response.status}.`);
  }

  const body = (await response.text()).slice(0, MAX_TRANSCRIPT_CHARS);
  const contentType = response.headers.get('content-type') ?? '';
  return parseTranscriptDocument(body, response.url || transcriptUrl, contentType);
}

async function fetchText(url: string, maxChars: number): Promise<string> {
  const response = await fetch(url, {
    headers: {
      accept: 'text/html,application/xhtml+xml,application/xml,text/xml,text/plain',
      'user-agent': 'HighlighterBot/0.2 (+https://beta.highlighter.com)'
    },
    redirect: 'follow',
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS)
  });

  if (!response.ok) {
    throw new Error(`Source responded with ${response.status}.`);
  }

  return (await response.text()).slice(0, maxChars);
}

function selectFeedItem(
  feed: PodcastFeed,
  input: {
    url: string;
    title: string;
    publishedAt: string;
    podcastGuid: string;
    catalogId: string;
  }
): PodcastFeedItem | undefined {
  const normalizedUrl = normalizeArtifactUrl(input.url) ?? input.url;
  const normalizedTitle = comparisonText(input.title);
  const normalizedDay = input.publishedAt ? isoDateDay(input.publishedAt) : '';
  const guidCandidate = cleanText(input.podcastGuid) || guidFromCatalogId(input.catalogId);

  let bestScore = 0;
  let bestItem: PodcastFeedItem | undefined;

  for (const item of feed.items) {
    let score = 0;

    if (guidCandidate && guidCandidate === cleanText(item.guid)) score += 120;
    if (normalizedUrl && normalizedUrl === (normalizeArtifactUrl(item.link) ?? item.link)) score += 80;
    if (normalizedTitle && normalizedTitle === comparisonText(item.title)) score += 48;
    if (normalizedTitle && comparisonText(item.title).includes(normalizedTitle)) score += 16;
    if (normalizedDay && normalizedDay === isoDateDay(item.publishedAt)) score += 12;

    if (score > bestScore) {
      bestScore = score;
      bestItem = item;
    }
  }

  return bestScore >= 40 ? bestItem : undefined;
}

function parsePodcastFeed(xml: string): PodcastFeed {
  const channel = /<channel\b[^>]*>([\s\S]*?)<\/channel>/i.exec(xml)?.[1] ?? xml;
  const items = Array.from(channel.matchAll(/<item\b[^>]*>([\s\S]*?)<\/item>/gi)).map((match) =>
    parsePodcastFeedItem(match[1] ?? '')
  );

  return {
    title: firstText([xmlTagText(channel, 'title')]),
    image: firstText([
      xmlAttributeValue(channel, 'itunes:image', 'href'),
      xmlTagText(channel, 'image', 'url')
    ]),
    transcriptUrl: firstText([xmlAttributeValue(channel, 'podcast:transcript', 'url')]),
    items
  };
}

function parsePodcastFeedItem(xml: string): PodcastFeedItem {
  return {
    guid: cleanText(xmlTagText(xml, 'guid')),
    title: firstText([xmlTagText(xml, 'title')]),
    link: firstText([xmlTagText(xml, 'link')]),
    description: firstText([
      xmlTagText(xml, 'content:encoded'),
      xmlTagText(xml, 'description'),
      xmlTagText(xml, 'itunes:summary')
    ]),
    audioUrl: firstText([xmlAttributeValue(xml, 'enclosure', 'url')]),
    transcriptUrl: firstText([xmlAttributeValue(xml, 'podcast:transcript', 'url')]),
    publishedAt: firstText([xmlTagText(xml, 'pubDate'), xmlTagText(xml, 'dc:date')]),
    durationSeconds: firstNumber([parseDurationSeconds(xmlTagText(xml, 'itunes:duration'))]),
    image: firstText([
      xmlAttributeValue(xml, 'itunes:image', 'href'),
      xmlTagText(xml, 'image', 'url')
    ]),
  };
}

function parseTranscriptDocument(body: string, sourceUrl: string, contentType: string): PodcastTranscriptSegment[] {
  const normalizedType = contentType.toLowerCase();
  const normalizedUrl = sourceUrl.toLowerCase();

  if (normalizedType.includes('json') || normalizedUrl.endsWith('.json')) {
    return parseJsonTranscript(body);
  }

  if (normalizedType.includes('vtt') || normalizedUrl.endsWith('.vtt')) {
    return parseVttTranscript(body);
  }

  if (normalizedUrl.endsWith('.srt')) {
    return parseSrtTranscript(body);
  }

  if (normalizedType.includes('html') || normalizedUrl.endsWith('.html') || normalizedUrl.endsWith('.htm')) {
    return extractInlineTranscriptSegments(body);
  }

  return parsePlainTranscript(body);
}

function parseJsonTranscript(source: string): PodcastTranscriptSegment[] {
  try {
    const parsed = JSON.parse(source);
    const segments = findJsonTranscriptSegments(parsed);
    if (segments.length > 0) {
      return segments;
    }
  } catch {
    return [];
  }

  return [];
}

function findJsonTranscriptSegments(value: unknown): PodcastTranscriptSegment[] {
  if (!value) return [];

  if (Array.isArray(value)) {
    const directSegments = value
      .map((item, index) => jsonTranscriptSegment(item, index))
      .filter((segment): segment is PodcastTranscriptSegment => Boolean(segment));
    if (directSegments.length > 0) return directSegments;

    for (const item of value) {
      const nested = findJsonTranscriptSegments(item);
      if (nested.length > 0) return nested;
    }

    return [];
  }

  if (typeof value !== 'object') {
    return [];
  }

  const record = value as Record<string, unknown>;
  for (const key of ['segments', 'results', 'items', 'captions', 'transcript']) {
    const nested = findJsonTranscriptSegments(record[key]);
    if (nested.length > 0) return nested;
  }

  return [];
}

function jsonTranscriptSegment(value: unknown, index: number): PodcastTranscriptSegment | undefined {
  if (!value || typeof value !== 'object') return undefined;

  const record = value as Record<string, unknown>;
  const text = firstText([
    stringValue(record.text),
    stringValue(record.value),
    stringValue(record.caption),
    stringValue(record.body)
  ]);
  if (!text) return undefined;

  return {
    id: firstText([stringValue(record.id), `json-${index}`]),
    startSeconds: firstNumber([
      numericUnknown(record.start),
      numericUnknown(record.startTime),
      numericUnknown(record.start_time),
      numericUnknown(record.offset)
    ]),
    endSeconds: firstNumber([
      numericUnknown(record.end),
      numericUnknown(record.endTime),
      numericUnknown(record.end_time)
    ]),
    speaker: firstText([
      stringValue(record.speaker),
      stringValue(record.speakerName),
      stringValue(record.speaker_name)
    ]),
    text: normalizeWhitespace(text)
  };
}

function parseVttTranscript(source: string): PodcastTranscriptSegment[] {
  const blocks = source.replace(/\r/g, '').split(/\n{2,}/);
  const segments: PodcastTranscriptSegment[] = [];

  for (const block of blocks) {
    const lines = block
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean);
    if (lines.length < 2) continue;

    const timeLineIndex = lines.findIndex((line) => /-->/.test(line));
    if (timeLineIndex < 0) continue;

    const [rawStart, rawEnd] = lines[timeLineIndex].split(/\s+-->\s+/);
    const text = normalizeWhitespace(lines.slice(timeLineIndex + 1).join(' '));
    if (!text) continue;

    segments.push({
      id: `vtt-${segments.length}`,
      startSeconds: parseTimestamp(rawStart),
      endSeconds: parseTimestamp(rawEnd),
      speaker: extractSpeaker(text),
      text: stripSpeakerPrefix(text)
    });
  }

  return normalizeTranscriptSegments(segments);
}

function parseSrtTranscript(source: string): PodcastTranscriptSegment[] {
  const blocks = source.replace(/\r/g, '').split(/\n{2,}/);
  const segments: PodcastTranscriptSegment[] = [];

  for (const block of blocks) {
    const lines = block
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean);
    if (lines.length < 2) continue;

    const timeLine = lines.find((line) => /-->/.test(line));
    if (!timeLine) continue;

    const [rawStart, rawEnd] = timeLine.split(/\s+-->\s+/);
    const text = normalizeWhitespace(lines.slice(lines.indexOf(timeLine) + 1).join(' '));
    if (!text) continue;

    segments.push({
      id: `srt-${segments.length}`,
      startSeconds: parseTimestamp(rawStart),
      endSeconds: parseTimestamp(rawEnd),
      speaker: extractSpeaker(text),
      text: stripSpeakerPrefix(text)
    });
  }

  return normalizeTranscriptSegments(segments);
}

function parsePlainTranscript(source: string): PodcastTranscriptSegment[] {
  const lines = source
    .replace(/\r/g, '')
    .split('\n')
    .map((line) => normalizeWhitespace(line))
    .filter(Boolean);
  const segments: PodcastTranscriptSegment[] = [];

  for (const line of lines) {
    const timestampMatch = /^\[?(\d{1,2}:\d{2}(?::\d{2})?(?:[.,]\d{1,3})?)\]?\s*(.+)$/.exec(line);
    if (timestampMatch) {
      const text = normalizeWhitespace(timestampMatch[2] ?? '');
      if (!text) continue;

      segments.push({
        id: `text-${segments.length}`,
        startSeconds: parseTimestamp(timestampMatch[1] ?? ''),
        endSeconds: null,
        speaker: extractSpeaker(text),
        text: stripSpeakerPrefix(text)
      });
      continue;
    }

    if (line.length > 0) {
      segments.push({
        id: `text-${segments.length}`,
        startSeconds: null,
        endSeconds: null,
        speaker: extractSpeaker(line),
        text: stripSpeakerPrefix(line)
      });
    }
  }

  return normalizeTranscriptSegments(segments);
}

function extractInlineTranscriptSegments(html: string): PodcastTranscriptSegment[] {
  const transcriptSection =
    /<(h[1-6])[^>]*>\s*Transcript\s*<\/\1>([\s\S]{0,120000})/i.exec(html)?.[2] ?? html;
  const paragraphMatches = Array.from(transcriptSection.matchAll(/<(p|li|div)[^>]*>([\s\S]*?)<\/\1>/gi));
  const segments: PodcastTranscriptSegment[] = [];

  for (const match of paragraphMatches) {
    const text = normalizeWhitespace(stripHtml(match[2] ?? ''));
    if (!text || text.length < 8) continue;

    const timestampMatch = /^(\[?\d{1,2}:\d{2}(?::\d{2})?(?:[.,]\d{1,3})?\]?)\s*(.+)$/.exec(text);
    const segmentText = timestampMatch ? normalizeWhitespace(timestampMatch[2] ?? '') : text;
    if (!segmentText) continue;

    segments.push({
      id: `inline-${segments.length}`,
      startSeconds: timestampMatch ? parseTimestamp(timestampMatch[1] ?? '') : null,
      endSeconds: null,
      speaker: extractSpeaker(segmentText),
      text: stripSpeakerPrefix(segmentText)
    });
  }

  return normalizeTranscriptSegments(segments).slice(0, 500);
}

function normalizeTranscriptSegments(segments: PodcastTranscriptSegment[]): PodcastTranscriptSegment[] {
  return segments
    .map((segment, index, list) => {
      const nextTimedSegment = list.slice(index + 1).find((candidate) => candidate.startSeconds != null);
      const endSeconds =
        segment.endSeconds != null
          ? segment.endSeconds
          : segment.startSeconds != null && nextTimedSegment?.startSeconds != null
            ? nextTimedSegment.startSeconds
            : null;

      return {
        id: segment.id || `segment-${index}`,
        startSeconds: segment.startSeconds,
        endSeconds,
        speaker: cleanText(segment.speaker),
        text: normalizeWhitespace(segment.text)
      };
    })
    .filter((segment) => Boolean(segment.text));
}

function extractSchemaEpisodeData(nodes: SchemaNode[], responseUrl: string) {
  const episodeNode = nodes.find((node) => schemaTypes(node).includes('podcastepisode')) ?? nodes[0];
  if (!episodeNode) {
    return {
      episodeTitle: '',
      showTitle: '',
      description: '',
      image: '',
      publishedAt: '',
      durationSeconds: null as number | null,
      audioUrl: '',
      transcriptUrl: '',
      feedUrl: '',
      podcastGuid: ''
    };
  }

  return {
    episodeTitle: firstText([textValue(episodeNode.name), textValue(episodeNode.headline)]),
    showTitle: firstText([
      textValue(nodeValue(episodeNode.partOfSeries, 'name')),
      textValue(nodeValue(episodeNode.partOfSeries, 'title'))
    ]),
    description: firstText([textValue(episodeNode.description)]),
    image: firstText([
      resolveUrl(urlValue(episodeNode.image), responseUrl),
      resolveUrl(urlValue(episodeNode.thumbnailUrl), responseUrl)
    ]),
    publishedAt: firstText([textValue(episodeNode.datePublished)]),
    durationSeconds: firstNumber([
      parseDurationSeconds(textValue(episodeNode.duration)),
      numericUnknown(episodeNode.duration)
    ]),
    audioUrl: firstText([
      resolveUrl(urlValue(nodeValue(episodeNode.associatedMedia, 'contentUrl')), responseUrl),
      resolveUrl(urlValue(nodeValue(episodeNode.audio, 'contentUrl')), responseUrl)
    ]),
    transcriptUrl: firstText([
      resolveUrl(urlValue(nodeValue(episodeNode.transcript, 'url')), responseUrl)
    ]),
    feedUrl: '',
    podcastGuid: firstText([
      textValue(episodeNode.identifier),
      textValue(nodeValue(episodeNode.identifier, 'value'))
    ])
  };
}

function extractAppleEpisodeData(html: string, responseUrl: string) {
  return {
    episodeTitle: firstText([
      metaContent(html, 'name', 'apple:title'),
      regexCapture(html, /"title":"([^"]+)"/i)
    ]),
    showTitle: firstText([
      regexCapture(html, /"showOffer":\{"title":"([^"]+)"/i),
      regexCapture(html, /"podcastOffer":\{"title":"([^"]+)"/i)
    ]),
    description: firstText([
      metaContent(html, 'name', 'apple:description'),
      regexCapture(html, /"summary":"([^"]+)"/i)
    ]),
    image: firstText([
      resolveUrl(metaContent(html, 'property', 'og:image'), responseUrl),
      resolveUrl(regexCapture(html, /"thumbnailUrl":"([^"]+)"/i), responseUrl)
    ]),
    publishedAt: firstText([
      regexCapture(html, /"releaseDate":"([^"]+)"/i),
      regexCapture(html, /"datePublished":"([^"]+)"/i)
    ]),
    durationSeconds: firstNumber([
      numericString(regexCapture(html, /"duration":([0-9]{2,})/i)),
      parseDurationSeconds(regexCapture(html, /"duration":"([^"]+)"/i))
    ]),
    audioUrl: firstText([resolveUrl(regexCapture(html, /"streamUrl":"([^"]+)"/i), responseUrl)]),
    transcriptUrl: '',
    feedUrl: firstText([resolveUrl(regexCapture(html, /"feedUrl":"([^"]+)"/i), responseUrl)]),
    podcastGuid: firstText([
      regexCapture(html, /"guid":"([^"]+)"/i),
      metaContent(html, 'name', 'podcast:guid')
    ])
  };
}

function extractSpotifyEpisodeData(html: string, responseUrl: string) {
  const episodeId =
    /\/episode\/([a-zA-Z0-9]+)/i.exec(responseUrl)?.[1] ??
    /spotify:\/\/episode\/([a-zA-Z0-9]+)/i.exec(metaContent(html, 'name', 'al:ios:url'))?.[1] ??
    '';
  const entityData = decodeSpotifyEntityData(html);
  const primaryEpisode = episodeId ? findSpotifyEpisode(entityData, episodeId) : undefined;
  const showData = primaryEpisode?.podcastV2?.data;
  const durationSeconds = firstNumber([
    numericString(regexCapture(html, /music:duration["'][^>]*content=["']([0-9]+)["']/i)),
    numericString(metaContent(html, 'name', 'music:duration')),
    millisecondsToSeconds(numericUnknown(primaryEpisode?.duration?.totalMilliseconds))
  ]);

  return {
    episodeTitle: firstText([
      stringValue(primaryEpisode?.name),
      metaContent(html, 'property', 'og:title')
    ]),
    showTitle: firstText([
      stringValue(showData?.name),
      cleanSpotifyShowFromTitle(textContent(html, 'title'))
    ]),
    description: firstText([
      stringValue(primaryEpisode?.description),
      metaContent(html, 'name', 'description')
    ]),
    image: firstText([
      spotifyCoverUrl(primaryEpisode?.coverArt),
      resolveUrl(metaContent(html, 'property', 'og:image'), responseUrl)
    ]),
    publishedAt: firstText([
      stringValue(primaryEpisode?.releaseDate?.isoString),
      regexCapture(html, /music:release_date["'][^>]*content=["']([^"']+)["']/i),
      metaContent(html, 'name', 'music:release_date')
    ]),
    durationSeconds: durationSeconds != null && durationSeconds > 0 ? durationSeconds : null,
    audioPreviewUrl: firstText([
      resolveUrl(stringValue(primaryEpisode?.audio?.items?.[0]?.url), responseUrl),
      resolveUrl(metaContent(html, 'property', 'og:audio'), responseUrl)
    ]),
    transcriptUrl: ''
  };
}

function findSpotifyEpisode(value: unknown, episodeId: string): Record<string, any> | undefined {
  if (!value || typeof value !== 'object') return undefined;

  const queue: unknown[] = [value];
  while (queue.length > 0) {
    const current = queue.shift();
    if (!current || typeof current !== 'object') continue;

    if (Array.isArray(current)) {
      queue.push(...current);
      continue;
    }

    const record = current as Record<string, any>;
    const recordId = stringValue(record.id);
    const recordUri = stringValue(record.uri);
    if (recordId === episodeId || recordUri === `spotify:episode:${episodeId}`) {
      return record;
    }

    for (const nested of Object.values(record)) {
      if (nested && typeof nested === 'object') {
        queue.push(nested);
      }
    }
  }

  return undefined;
}

function decodeSpotifyEntityData(html: string): unknown {
  const encoded = /<script[^>]*id=["']entitySSRData["'][^>]*>([^<]+)<\/script>/i.exec(html)?.[1]?.trim();
  if (!encoded) return undefined;

  try {
    return JSON.parse(Buffer.from(encoded, 'base64').toString('utf8'));
  } catch {
    return undefined;
  }
}

function spotifyCoverUrl(value: unknown): string {
  if (!value || typeof value !== 'object') return '';
  const sources = Array.isArray((value as Record<string, unknown>).sources)
    ? ((value as Record<string, unknown>).sources as Array<Record<string, unknown>>)
    : [];

  const largest = sources
    .map((source) => ({
      url: stringValue(source.url),
      width: numericUnknown(source.width) ?? 0
    }))
    .filter((source) => source.url)
    .sort((left, right) => right.width - left.width)[0];

  return largest?.url ?? '';
}

function resolvePodcastCatalogIdentity(url: string, podcastGuid: string): { catalogId: string; catalogKind: string } {
  const normalizedGuid = cleanText(podcastGuid);
  if (normalizedGuid) {
    return {
      catalogId: `podcast:guid:${normalizedGuid}`,
      catalogKind: 'podcast:guid'
    };
  }

  const spotifyEpisodeId = /open\.spotify\.com\/episode\/([a-zA-Z0-9]+)/i.exec(url)?.[1];
  if (spotifyEpisodeId) {
    return {
      catalogId: `spotify:episode:${spotifyEpisodeId}`,
      catalogKind: 'spotify:episode'
    };
  }

  const appleEpisodeId =
    /[?&]i=([0-9]+)/i.exec(url)?.[1];
  if (appleEpisodeId) {
    return {
      catalogId: `apple:podcast-episode:${appleEpisodeId}`,
      catalogKind: 'apple:podcast-episode'
    };
  }

  const overcastEpisodeId = /overcast\.fm\/\+([a-zA-Z0-9]+)/i.exec(url)?.[1];
  if (overcastEpisodeId) {
    return {
      catalogId: `overcast:episode:${overcastEpisodeId}`,
      catalogKind: 'overcast:episode'
    };
  }

  return {
    catalogId: url,
    catalogKind: 'web'
  };
}

function guidFromCatalogId(value: string): string {
  const normalized = cleanText(value);
  return normalized.startsWith('podcast:guid:') ? normalized.slice('podcast:guid:'.length) : '';
}

function extractPodcastGuidFromText(value: string): string {
  const match =
    /podcast(?::item)?:guid:([a-z0-9-]{8,})/i.exec(value) ||
    /\b([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b/i.exec(value);
  return cleanText(match?.[1]);
}

function findTranscriptLink(html: string, baseUrl: string): string {
  for (const match of html.matchAll(/<a[^>]*href=["']([^"']+)["'][^>]*>([\s\S]*?)<\/a>/gi)) {
    const href = resolveUrl(decodeHtml(match[1] ?? ''), baseUrl);
    const label = normalizeWhitespace(stripHtml(match[2] ?? ''));
    if (!href) continue;
    if (/\b(transcript|captions|read transcript)\b/i.test(label)) {
      return href;
    }
  }

  return '';
}

function extractHtmlAudioSource(html: string, baseUrl: string): string {
  return firstText([
    resolveUrl(regexCapture(html, /<audio[^>]*src=["']([^"']+)["']/i), baseUrl),
    resolveUrl(regexCapture(html, /<source[^>]*src=["']([^"']+)["'][^>]*type=["']audio\//i), baseUrl)
  ]);
}

function parseJsonLdNodes(html: string): SchemaNode[] {
  const matches = html.matchAll(
    /<script[^>]*type=["']application\/ld\+json["'][^>]*>([\s\S]*?)<\/script>/gi
  );
  const nodes: SchemaNode[] = [];

  for (const match of matches) {
    const source = cleanJsonLdSource(match[1] ?? '');
    if (!source) continue;

    try {
      nodes.push(...flattenJsonLd(JSON.parse(source)));
    } catch {
      continue;
    }
  }

  return nodes;
}

function flattenJsonLd(value: unknown): SchemaNode[] {
  if (!value) return [];

  if (Array.isArray(value)) {
    return value.flatMap((item) => flattenJsonLd(item));
  }

  if (typeof value !== 'object') {
    return [];
  }

  const node = value as SchemaNode;
  return [node, ...flattenJsonLd(node['@graph']), ...flattenJsonLd(node.mainEntity)];
}

function schemaTypes(node: SchemaNode): string[] {
  const value = node['@type'];
  const items = Array.isArray(value) ? value : [value];

  return items
    .map((item) => textValue(item).toLowerCase())
    .filter(Boolean)
    .map((item) => item.split('/').at(-1) ?? item);
}

function cleanJsonLdSource(value: string): string {
  return value.replace(/^\s*<!--/, '').replace(/-->\s*$/, '').trim();
}

function urlValue(value: unknown): string {
  if (!value) return '';
  if (typeof value === 'string') return decodeHtml(value);
  if (Array.isArray(value)) return firstText(value.map((item) => urlValue(item)));
  if (typeof value !== 'object') return '';

  const record = value as Record<string, unknown>;
  return firstText([stringValue(record.url), stringValue(record.contentUrl), stringValue(record['@id'])]);
}

function nodeValue(value: unknown, key: string): unknown {
  if (!value || typeof value !== 'object') return undefined;
  return (value as Record<string, unknown>)[key];
}

function stringValue(value: unknown): string {
  if (typeof value === 'string') return decodeHtml(value);
  if (typeof value === 'number') return String(value);
  return '';
}

function textValue(value: unknown): string {
  return normalizeWhitespace(stringValue(value));
}

function numericUnknown(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) ? parsed : null;
  }

  return null;
}

function numericString(value: string): number | null {
  const parsed = Number(cleanText(value));
  return Number.isFinite(parsed) ? parsed : null;
}

function millisecondsToSeconds(value: number | null): number | null {
  return value == null || value <= 0 ? null : Math.round(value / 1000);
}

function parseTimestamp(value: string): number | null {
  const match = /(\d{1,2}):(\d{2})(?::(\d{2}))?(?:[.,](\d{1,3}))?/.exec(value.trim());
  if (!match) return null;

  const first = Number(match[1]);
  const second = Number(match[2]);
  const third = match[3] ? Number(match[3]) : null;
  const milliseconds = match[4] ? Number(match[4].padEnd(3, '0')) : 0;

  if (third == null) {
    return first * 60 + second + milliseconds / 1000;
  }

  return first * 3600 + second * 60 + third + milliseconds / 1000;
}

function parseDurationSeconds(value: string): number | null {
  const normalized = cleanText(value);
  if (!normalized) return null;

  if (/^pt/i.test(normalized)) {
    const hours = Number(/(\d+)h/i.exec(normalized)?.[1] ?? 0);
    const minutes = Number(/(\d+)m/i.exec(normalized)?.[1] ?? 0);
    const seconds = Number(/(\d+)s/i.exec(normalized)?.[1] ?? 0);
    return hours * 3600 + minutes * 60 + seconds;
  }

  if (/^\d+$/.test(normalized)) {
    return Number(normalized);
  }

  const timestamp = parseTimestamp(normalized);
  return timestamp == null ? null : Math.round(timestamp);
}

function metaContent(
  html: string,
  attribute: 'name' | 'property' | 'itemprop',
  value: string
): string {
  const escaped = escapeRegex(value);
  const patterns = [
    new RegExp(`<meta[^>]*${attribute}=["']${escaped}["'][^>]*content=["']([^"']+)["'][^>]*>`, 'i'),
    new RegExp(`<meta[^>]*content=["']([^"']+)["'][^>]*${attribute}=["']${escaped}["'][^>]*>`, 'i')
  ];

  for (const pattern of patterns) {
    const match = pattern.exec(html);
    if (match?.[1]) return decodeHtml(match[1]);
  }

  return '';
}

function linkHref(html: string, rel: string): string {
  const escaped = escapeRegex(rel);
  const patterns = [
    new RegExp(`<link[^>]*rel=["'][^"']*${escaped}[^"']*["'][^>]*href=["']([^"']+)["'][^>]*>`, 'i'),
    new RegExp(`<link[^>]*href=["']([^"']+)["'][^>]*rel=["'][^"']*${escaped}[^"']*["'][^>]*>`, 'i')
  ];

  for (const pattern of patterns) {
    const match = pattern.exec(html);
    if (match?.[1]) return decodeHtml(match[1]);
  }

  return '';
}

function linkHrefByType(html: string, mimeType: string): string {
  const escaped = escapeRegex(mimeType);
  const patterns = [
    new RegExp(`<link[^>]*type=["']${escaped}["'][^>]*href=["']([^"']+)["'][^>]*>`, 'i'),
    new RegExp(`<link[^>]*href=["']([^"']+)["'][^>]*type=["']${escaped}["'][^>]*>`, 'i')
  ];

  for (const pattern of patterns) {
    const match = pattern.exec(html);
    if (match?.[1]) return decodeHtml(match[1]);
  }

  return '';
}

function textContent(html: string, tag: string): string {
  const match = new RegExp(`<${tag}[^>]*>([\\s\\S]*?)</${tag}>`, 'i').exec(html);
  return match?.[1] ? normalizeWhitespace(stripHtml(match[1])) : '';
}

function xmlTagText(xml: string, tagName: string, childTagName?: string): string {
  const escaped = escapeRegex(tagName);
  const match = new RegExp(`<${escaped}\\b[^>]*>([\\s\\S]*?)</${escaped}>`, 'i').exec(xml);
  if (!match?.[1]) return '';
  if (!childTagName) return normalizeWhitespace(stripHtml(match[1]));
  return xmlTagText(match[1], childTagName);
}

function xmlAttributeValue(xml: string, tagName: string, attributeName: string): string {
  const escapedTag = escapeRegex(tagName);
  const escapedAttribute = escapeRegex(attributeName);
  const match = new RegExp(
    `<${escapedTag}\\b[^>]*${escapedAttribute}=["']([^"']+)["'][^>]*\\/?>`,
    'i'
  ).exec(xml);

  return decodeHtml(match?.[1] ?? '');
}

function resolveUrl(value: string, base: string): string {
  const normalized = cleanText(value);
  if (!normalized) return '';

  try {
    return new URL(normalized, base).toString();
  } catch {
    return '';
  }
}

function preferText(current: string, next: string, domain: string): string {
  const currentValue = cleanText(current);
  const nextValue = cleanText(next);
  if (!nextValue) return currentValue;
  if (!currentValue) return nextValue;
  if (isLikelyGenericText(currentValue, domain) && !isLikelyGenericText(nextValue, domain)) {
    return nextValue;
  }

  return currentValue;
}

function preferUrl(current: string, next: string): string {
  const currentValue = cleanText(current);
  const nextValue = cleanText(next);
  if (!nextValue) return currentValue;
  if (!currentValue) return nextValue;
  return currentValue;
}

function preferValue(current: string, next: string): string {
  const currentValue = cleanText(current);
  const nextValue = cleanText(next);
  return currentValue || nextValue;
}

function preferNumber(current: number | null, next: number | null): number | null {
  return current ?? next;
}

function sanitizeNumber(value: number | null | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null;
}

function comparisonText(value: string): string {
  return normalizeWhitespace(value)
    .toLowerCase()
    .replace(/^www\./, '')
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

function isLikelyGenericText(value: string, domain: string): boolean {
  const normalizedValue = comparisonText(value);
  const normalizedDomain = comparisonText(domain);
  if (!normalizedValue) return true;
  if (normalizedDomain && normalizedValue === normalizedDomain) return true;
  return false;
}

function cleanTitle(value: string, responseUrl: string): string {
  const normalized = normalizeWhitespace(value);
  if (!normalized) return '';

  if (hostnameLabel(responseUrl).includes('spotify.com')) {
    return normalized.replace(/\s*-\s*.+?\|\s*Podcast on Spotify$/i, '').trim();
  }

  if (hostnameLabel(responseUrl).includes('podcasts.apple.com')) {
    return normalized.replace(/\s*[–-]\s*.+?\s*-\s*Apple Podcasts$/i, '').trim();
  }

  return normalized;
}

function cleanSpotifyShowFromTitle(value: string): string {
  const normalized = normalizeWhitespace(value);
  const match = /-\s*(.+?)\s*\|\s*Podcast on Spotify$/i.exec(normalized);
  return normalizeWhitespace(match?.[1] ?? '');
}

function extractSpeaker(value: string): string {
  const match = /^([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,3}|[A-Z]{2,10})\s*:\s+/.exec(value);
  return cleanText(match?.[1]);
}

function stripSpeakerPrefix(value: string): string {
  return normalizeWhitespace(value.replace(/^([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,3}|[A-Z]{2,10})\s*:\s+/, ''));
}

function hostnameLabel(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

function firstText(values: Array<string | undefined>): string {
  for (const value of values) {
    const normalized = cleanText(value);
    if (normalized) return normalized;
  }

  return '';
}

function firstNumber(values: Array<number | null | undefined>): number | null {
  for (const value of values) {
    if (typeof value === 'number' && Number.isFinite(value) && value >= 0) {
      return value;
    }
  }

  return null;
}

function regexCapture(value: string, pattern: RegExp): string {
  return decodeHtml(pattern.exec(value)?.[1] ?? '');
}

function stripHtml(value: string): string {
  return decodeHtml(value.replace(/<[^>]+>/g, ' '));
}

function normalizeWhitespace(value: string): string {
  return value.replace(/\s+/g, ' ').trim();
}

function decodeHtml(value: string): string {
  return decodeHtmlEntities(value);
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function cleanText(value: string | undefined | null): string {
  return typeof value === 'string' ? value.trim() : '';
}

function isoDateDay(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return date.toISOString().slice(0, 10);
}
