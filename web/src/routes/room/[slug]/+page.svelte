<script lang="ts">
  import RoomHeader from '$lib/features/room/components/RoomHeader.svelte';
  import RoomNav from '$lib/features/room/components/RoomNav.svelte';
  import Block from '$lib/features/room/components/Block.svelte';
  import PinnedArtifact from '$lib/features/room/components/PinnedArtifact.svelte';
  import AlsoCard from '$lib/features/room/components/AlsoCard.svelte';
  import ShelfTile from '$lib/features/room/components/ShelfTile.svelte';
  import SeeAllLink from '$lib/features/room/components/SeeAllLink.svelte';
  import HighlightCard from '$lib/features/room/components/HighlightCard.svelte';
  import DiscussionRow from '$lib/features/room/components/DiscussionRow.svelte';
  import ActivityFeed from '$lib/features/room/components/ActivityFeed.svelte';
  import MembersSidebar from '$lib/features/room/components/MembersSidebar.svelte';
  import UpNextVoting from '$lib/features/room/components/UpNextVoting.svelte';
  import CaptureCta from '$lib/features/room/components/CaptureCta.svelte';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const seedMembers = [
    { colorIndex: 1, name: 'DK', initials: 'DK' },
    { colorIndex: 2, name: 'Pablo F', initials: 'PF' },
    { colorIndex: 4, name: 'Miljan', initials: 'MB' },
    { colorIndex: 3, name: 'Bob S', initials: 'BS' },
    { colorIndex: 5, name: 'Steve L', initials: 'SL' },
    { colorIndex: 6, name: 'Max W', initials: 'MW' }
  ];

  const sidebarMembers = [
    {
      colorIndex: 1,
      initials: 'DK',
      name: 'David King',
      handle: 'dk',
      status: 'Reading Postman. 1985 has more to say about 2026 than 2025 did.'
    },
    {
      colorIndex: 2,
      initials: 'PF',
      name: 'Pablo Fernandez',
      handle: 'pablof7z',
      status: 'Highlighting at 3am again.'
    },
    {
      colorIndex: 4,
      initials: 'MB',
      name: 'Miljan Braticevic',
      handle: 'miljan',
      status: 'On chapter 5. Unlocking something.'
    },
    {
      colorIndex: 3,
      initials: 'BS',
      name: 'Bob Scully',
      handle: 'bobscully',
      status: 'Listening to Lyn on the walk to the boardwalk office.'
    },
    {
      colorIndex: 5,
      initials: 'SL',
      name: 'Steve Lee',
      handle: 'moneyball',
      status: 'Re-read the Dergigi twice this morning.'
    },
    {
      colorIndex: 6,
      initials: 'MW',
      name: 'Max Webster',
      handle: 'maxwebster',
      status: 'Trying to convince everyone to vote for the McLuhan.'
    }
  ];

  const upNextItems = [
    { id: 'v1', title: 'Read Write Own', source: 'Chris Dixon · 2024', voteCount: 3 },
    { id: 'v2', title: 'Broken Money', source: 'Lyn Alden · 2023', voteCount: 2 },
    {
      id: 'v3',
      title: 'The Medium is the Massage',
      source: 'Marshall McLuhan · 1967',
      voteCount: 1
    },
    {
      id: 'v4',
      title: 'Building a Bridge to the 18th Century',
      source: 'Neil Postman · 1999 · just proposed',
      voteCount: 0
    }
  ];

  const roomTitle = $derived(data.room?.name ?? 'Signal vs Noise');
  const members = $derived(data.room?.members ?? seedMembers);

  const sections = [
    { id: 'pinned', label: 'Pinned' },
    { id: 'this-week', label: 'This week' },
    { id: 'shelf', label: 'The shelf', count: 24 },
    { id: 'highlights', label: 'Highlights', count: 412 },
    { id: 'discussions', label: 'Discussions', count: 38 },
    { id: 'lately', label: 'Lately' }
  ];

  // ── Pinned seed data (matches mock) ──
  const pinnedStats = [
    { value: '4 of 6', label: 'reading' },
    { value: '31', label: 'highlights' },
    { value: '5', label: 'threads' },
    { value: '14 days', label: 'in the room' }
  ];

  const pinnedReaders = [
    { colorIndex: 1, initials: 'DK', name: 'DK' },
    { colorIndex: 2, initials: 'PF', name: 'Pablo F' },
    { colorIndex: 4, initials: 'MB', name: 'Miljan' },
    { colorIndex: 6, initials: 'MW', name: 'Max W' }
  ];

  const passageSpans = [
    { text: 'Americans no longer talk to each other, ' },
    { text: 'they entertain each other', colorIndex: 1, markedBy: 'DK' },
    { text: '. They do not exchange ideas; ' },
    { text: 'they exchange images', colorIndex: 2, markedBy: 'Pablo F' },
    { text: '. ' },
    { text: 'They do not argue with propositions', colorIndex: 4, markedBy: 'Miljan' },
    { text: '; they argue with ' },
    {
      text: 'good looks, celebrities and commercials',
      colorIndex: 3,
      markedBy: 'Bob S'
    },
    { text: '.' }
  ];

  const seedMessages = [
    {
      id: 'm1',
      colorIndex: 1,
      initials: 'DK',
      name: 'DK',
      handle: 'dk',
      time: 'Mon 9:12',
      body: 'Thirty-nine years later and "television" is the only word you\'d need to swap.'
    },
    {
      id: 'm2',
      colorIndex: 2,
      initials: 'PF',
      name: 'Pablo F',
      handle: 'pablof7z',
      time: 'Mon 9:44',
      body: 'Replace "television" with "algorithmic feed" — the meter doesn\'t break.',
      isReply: true
    },
    {
      id: 'm3',
      colorIndex: 4,
      initials: 'MB',
      name: 'Miljan',
      handle: 'miljan',
      time: 'Mon 10:20',
      body: "Postman saw the trajectory. He didn't see the way out. This is genuinely what we're building Primal against.",
      isReply: true
    },
    {
      id: 'm4',
      colorIndex: 3,
      initials: 'BS',
      name: 'Bob S',
      handle: 'bobscully',
      time: 'Mon 11:05',
      body: 'Stealing "good looks, celebrities and commercials" for the boardwalk deck. No apology.'
    },
    {
      id: 'm5',
      colorIndex: 5,
      initials: 'SL',
      name: 'Steve L',
      handle: 'moneyball',
      time: 'Tue 6:40',
      body: 'Genuine question: is this a book to read, or the operating manual for the last forty years of consumer internet?'
    },
    {
      id: 'm6',
      colorIndex: 6,
      initials: 'MW',
      name: 'Max W',
      handle: 'maxwebster',
      time: 'Tue 8:30',
      body: 'Re-read it last year. Holds up in ways the sequel does not, but the sequel is worth it for one chapter.'
    }
  ];

  const seedHighlights = [
    {
      id: 'h1',
      memberColorIndex: 1,
      memberName: 'DK',
      memberInitials: 'DK',
      location: 'Foreword',
      date: 'Tue',
      replies: 2,
      quote:
        'Orwell feared those who would deprive us of information. Huxley feared those who would give us so much that we would be reduced to passivity and egoism.'
    },
    {
      id: 'h2',
      memberColorIndex: 2,
      memberName: 'Pablo F',
      memberInitials: 'PF',
      location: 'Ch. 1 · pg. 8',
      date: 'Mon',
      quote:
        "The media of communication available to a culture are a dominant influence on the formation of the culture's intellectual and social preoccupations."
    },
    {
      id: 'h3',
      memberColorIndex: 1,
      memberName: 'DK',
      memberInitials: 'DK',
      location: 'Ch. 1 · pg. 11',
      date: 'Mon',
      replies: 1,
      quote:
        'Television has achieved the status of "meta-medium" — an instrument that directs not only our knowledge of the world, but our knowledge of ways of knowing as well.'
    },
    {
      id: 'h4',
      memberColorIndex: 4,
      memberName: 'Miljan',
      memberInitials: 'MB',
      location: 'Ch. 2 · pg. 20',
      date: 'Mon',
      quote: 'A new medium does not add something; it changes everything.'
    },
    {
      id: 'h5',
      memberColorIndex: 6,
      memberName: 'Max W',
      memberInitials: 'MW',
      location: 'Ch. 5 · pg. 72',
      date: 'Sat',
      quote:
        'Every technology has a bias, and every bias favours some kinds of thought over others.'
    }
  ];

  const memberFilters = [
    { colorIndex: 1, initials: 'DK', name: 'DK', count: 12 },
    { colorIndex: 2, initials: 'PF', name: 'Pablo F', count: 7 },
    { colorIndex: 4, initials: 'MB', name: 'Miljan', count: 8 },
    { colorIndex: 6, initials: 'MW', name: 'Max W', count: 4 }
  ];

  const seedNotes = [
    {
      id: 'n1',
      memberColorIndex: 1,
      memberName: 'DK',
      memberInitials: 'DK',
      memberHandle: 'dk',
      title: 'On re-reading Postman in 2026',
      content:
        "This is my third time through Amusing Ourselves to Death. The first was 2009, the second 2016. Both times I read it as history. This time, reading it in 2026 with LLMs reshaping the attention economy in real time, it reads as forecast.\n\nThe move I missed both times: Postman is not arguing that the medium determines the message. He's arguing that the dominant medium determines what a culture can know. Print-based culture knows some things. Television-based culture cannot know them.\n\nCurious if the room agrees.",
      date: 'Tuesday',
      replies: 3
    },
    {
      id: 'n2',
      memberColorIndex: 2,
      memberName: 'Pablo F',
      memberInitials: 'PF',
      memberHandle: 'pablof7z',
      title: "Chapter 5 and the shape of what's next",
      content:
        "Chapter 5 is the chapter I've been thinking about all week. Postman is making the case that the telegraph broke a particular shape of public discourse. But the thing he's describing — the decontextualisation of information — is the exact failure mode we're building Nostr to route around.\n\nA highlight on Nostr is a recontextualisation. The excerpt + the signature + the group it's shared to — that's the \"place\" the information has.",
      date: 'Sunday · 3:12am',
      replies: 1
    },
    {
      id: 'n3',
      memberColorIndex: 4,
      memberName: 'Miljan',
      memberInitials: 'MB',
      memberHandle: 'miljan',
      title: 'Is this a book you disagree with or not?',
      content:
        "I'm five chapters in and I still can't tell if this is a book I agree with. The argument is persuasive but Postman's implied prescription — television bad, print good — collapses into nostalgia whenever he gets specific.\n\nThe real argument, I think, is not about television. It's about what any dominant medium does to thought.",
      date: 'Monday · 7:14am',
      replies: 4
    }
  ];

  const membersTableRows = [
    {
      colorIndex: 1,
      initials: 'DK',
      name: 'DK',
      handle: 'dk',
      progressPct: 100,
      progressLabel: '<b>Ch. 6</b> · finished the book',
      progressState: 'done' as const,
      contribution: { highlights: 12, messages: 4, notes: 1 },
      lastHere: 'Tue 9:00'
    },
    {
      colorIndex: 2,
      initials: 'PF',
      name: 'Pablo F',
      handle: 'pablof7z',
      progressPct: 90,
      progressLabel: '<b>Ch. 6</b> · pg. 86',
      progressState: 'inProgress' as const,
      contribution: { highlights: 7, messages: 2, notes: 1 },
      lastHere: 'Mon 9:44'
    },
    {
      colorIndex: 4,
      initials: 'MB',
      name: 'Miljan',
      handle: 'miljan',
      progressPct: 72,
      progressLabel: '<b>Ch. 5</b> · pg. 68',
      progressState: 'inProgress' as const,
      contribution: { highlights: 8, messages: 3, notes: 1 },
      lastHere: 'Mon 10:20'
    },
    {
      colorIndex: 6,
      initials: 'MW',
      name: 'Max W',
      handle: 'maxwebster',
      progressPct: 54,
      progressLabel: '<b>Ch. 4</b> · pg. 56',
      progressState: 'inProgress' as const,
      contribution: { highlights: 4, messages: 2 },
      lastHere: 'Tue 8:30'
    },
    {
      colorIndex: 3,
      initials: 'BS',
      name: 'Bob S',
      handle: 'bobscully',
      progressPct: 0,
      progressLabel: '<em>not started</em> — "will get to it"',
      progressState: 'none' as const,
      contribution: { messages: 1 },
      lastHere: 'Mon 11:05'
    },
    {
      colorIndex: 5,
      initials: 'SL',
      name: 'Steve L',
      handle: 'moneyball',
      progressPct: 0,
      progressLabel: '<em>not started</em>',
      progressState: 'none' as const,
      contribution: { messages: 1 },
      lastHere: 'Tue 6:40'
    }
  ];
</script>

<svelte:head>
  <title>{roomTitle} · Room</title>
</svelte:head>

<RoomHeader title={roomTitle} {members} />
<RoomNav {sections} />

<div class="room-main">
  <div class="room-content">
    <Block id="pinned" title="Currently pinned." accent="pinned.">
      <PinnedArtifact
        title="Amusing Ourselves to Death"
        subtitle="Neil Postman · 1985 · currently reading: chapter 6"
        coverTitle={'Amusing\nOurselves\nto Death'}
        coverAuthor="Neil Postman"
        coverKicker="A Book · 1985"
        coverVariant="dark"
        stats={pinnedStats}
        readers={pinnedReaders}
        readersNote="Bob & Steve haven't started"
        tabCounts={{ discussions: 5, highlights: 31, notes: 3, members: 6 }}
        passageLabel="Most-discussed passage · chapter 6, §2 · 8 replies"
        {passageSpans}
        threadTitle="Thread · 8 messages"
        threadStarter="DK"
        threadStartedAt="Mon"
        messages={seedMessages}
        highlights={seedHighlights}
        memberFilters={memberFilters}
        notes={seedNotes}
        {membersTableRows}
      />
    </Block>

    <Block id="this-week" title="Also this week." accent="week.">
      <div class="also-grid">
        <AlsoCard
          href="/room/signal-over-noise/artifact/tftc-642"
          type="podcast"
          sharedBy="Bob S"
          when="2h ago"
          artworkLabel="TFTC"
          title="Broken Money, Two Years In"
          source="Marty Bent & Lyn Alden · ep. 642 · 1h 24m"
          excerptStamp="<b>00:43:12 → 00:44:08</b> · marked by Miljan + Steve L"
          excerptQuote={"\u201cThe unit of account is the thing nobody audits. That's why it's the best place to hide a long-running monetary lie.\u201d"}
          reactions={[
            {
              memberColorIndex: 4,
              memberInitials: 'MB',
              memberName: 'Miljan',
              text: "This is the chapter she didn't quite write in the book."
            },
            {
              memberColorIndex: 5,
              memberInitials: 'SL',
              memberName: 'Steve L',
              text: "Ran this past a bank analyst friend yesterday. She didn't have a counter."
            }
          ]}
          engaged={[
            { colorIndex: 3, initials: 'BS' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 6, initials: 'MW' }
          ]}
        />
        <AlsoCard
          href="/room/signal-over-noise/artifact/dergigi-purple"
          type="essay"
          sharedBy="Pablo F"
          when="yesterday"
          artworkVariant="essay"
          title="Purple Text, Orange Highlights"
          source="Dergigi · dergigi.com · 2023 (re-read)"
          excerptStamp="<b>a line worth re-reading</b> · marked by all 6"
          excerptQuote={"\u201cReading is a solitary act that yearns to be a social one.\u201d"}
          reactions={[
            {
              memberColorIndex: 2,
              memberInitials: 'PF',
              memberName: 'Pablo F',
              text: 'Gigi wrote this two months before I started the Nostr build. He knew.'
            },
            {
              memberColorIndex: 1,
              memberInitials: 'DK',
              memberName: 'DK',
              text: 'Three years on, NIP-84 did what this essay was asking for.'
            }
          ]}
          engaged={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 3, initials: 'BS' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 6, initials: 'MW' }
          ]}
        />
      </div>
    </Block>

    <Block id="shelf" title="The shelf." accent="shelf.">
      {#snippet filters()}
        <div class="filter-row">
          <button type="button" class="filter-pill on">Everything <span class="c">24</span></button>
          <button type="button" class="filter-pill">Books <span class="c">8</span></button>
          <button type="button" class="filter-pill">Podcasts <span class="c">5</span></button>
          <button type="button" class="filter-pill">Essays <span class="c">7</span></button>
          <button type="button" class="filter-pill">Papers <span class="c">2</span></button>
          <button type="button" class="filter-pill">Archive <span class="c">2</span></button>
          <button type="button" class="filter-pill sort">Recent ↓</button>
        </div>
      {/snippet}

      <div class="shelf-grid">
        <ShelfTile
          id="s-amusing"
          type="book"
          bookVariant="dark"
          typeChipLabel="Book · 1985"
          title={'Amusing\nOurselves\nto Death'}
          author="Neil Postman"
          status="reading"
          statusLabel="Reading"
          engaged={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 6, initials: 'MW' }
          ]}
          stats="31 hl · 5 thr"
        />
        <ShelfTile
          id="s-tftc642"
          type="podcast"
          typeChipLabel="Podcast"
          title={'Broken Money,\nTwo Years In'}
          author="TFTC · ep. 642 · Lyn Alden"
          status="this-week"
          statusLabel="This week"
          engaged={[
            { colorIndex: 3, initials: 'BS' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 6, initials: 'MW' }
          ]}
          stats="7 hl · 2 thr"
        />
        <ShelfTile
          id="s-dergigi"
          type="essay"
          typeChipLabel="Essay · 2023"
          title={'Purple Text,\nOrange Highlights'}
          author="Dergigi · dergigi.com"
          status="re-read"
          statusLabel="Re-read"
          engaged={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 3, initials: 'BS' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 6, initials: 'MW' }
          ]}
          stats="14 hl · 3 thr"
        />
        <ShelfTile
          id="s-whitepaper"
          type="paper"
          typeChipLabel="Paper · 2008"
          title={'Bitcoin: A Peer-to-Peer\nElectronic Cash System'}
          author="Satoshi Nakamoto"
          engaged={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' }
          ]}
          stats="22 hl · 3 thr · Dec"
        />
        <ShelfTile
          id="s-seven"
          type="essay"
          typeChipLabel="Essay · 2025"
          title={'Nostr and the\nSeven Churches'}
          author="fiatjaf"
          engaged={[
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 1, initials: 'DK' }
          ]}
          stats="11 hl · 2 thr · Nov"
        />
        <ShelfTile
          id="s-zuboff"
          type="book"
          bookVariant="red"
          typeChipLabel="Book · 2019"
          title={'The Age of\nSurveillance\nCapitalism'}
          author="Shoshana Zuboff"
          engaged={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 3, initials: 'BS' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 6, initials: 'MW' }
          ]}
          stats="41 hl · 4 thr · Oct"
        />
        <ShelfTile
          id="s-networkstate"
          type="book"
          bookVariant="blue"
          typeChipLabel="Book · 2022"
          title={'The\nNetwork\nState'}
          author="Balaji Srinivasan"
          engaged={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 6, initials: 'MW' }
          ]}
          stats="48 hl · 7 thr · May"
        />
        <ShelfTile
          id="s-halchived"
          type="archive"
          typeChipLabel="Archive · 1993"
          title={'Hal Finney on\nthe Extropians list'}
          author="archive.org · 1993–2000"
          engaged={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 4, initials: 'MB' }
          ]}
          stats="11 hl · 1 thr · Jan"
        />
      </div>

      <SeeAllLink label="See all 24 on the shelf" href="#" />
    </Block>

    <Block id="highlights" title="The room's highlights." accent="highlights.">
      {#snippet filters()}
        <div class="filter-row">
          <button type="button" class="filter-pill on">All <span class="c">412</span></button>
          <button type="button" class="filter-pill">DK <span class="c">84</span></button>
          <button type="button" class="filter-pill">Pablo F <span class="c">92</span></button>
          <button type="button" class="filter-pill">Miljan <span class="c">51</span></button>
          <button type="button" class="filter-pill">Bob S <span class="c">48</span></button>
          <button type="button" class="filter-pill">Steve L <span class="c">76</span></button>
          <button type="button" class="filter-pill">Max W <span class="c">61</span></button>
          <button type="button" class="filter-pill sort">Most-replied ↓</button>
        </div>
      {/snippet}

      <div class="hl-reel">
        <HighlightCard
          quote="They do not exchange ideas; they exchange images."
          sourceTitle="Postman · Amusing Ourselves to Death"
          sourceSub="ch. 6 · Currently reading"
          marks={[{ colorIndex: 2, initials: 'PF' }]}
          replies={8}
          hot
          date="Mon"
        />
        <HighlightCard
          href="/room/signal-over-noise/artifact/tftc-642"
          quote="The unit of account is the thing nobody audits. That's why it's the best place to hide a long-running monetary lie."
          sourceTitle="Lyn Alden · TFTC #642"
          sourceSub="00:43:12 → 00:44:08 · this week"
          marks={[
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' }
          ]}
          replies={3}
          date="Tue"
        />
        <HighlightCard
          href="/room/signal-over-noise/artifact/dergigi-purple"
          quote="Orange is the new purple."
          sourceTitle="Dergigi · Purple Text, Orange Highlights"
          sourceSub="this week · re-read"
          marks={[{ colorIndex: 6, initials: 'MW' }]}
          replies={2}
          date="Mon"
        />
        <HighlightCard
          href="/room/signal-over-noise/artifact/dergigi-purple"
          quote="Reading is a solitary act that yearns to be a social one."
          sourceTitle="Dergigi · Purple Text, Orange Highlights"
          sourceSub="marked by all 6"
          marks={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 3, initials: 'BS' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 6, initials: 'MW' }
          ]}
          replies={0}
          date="Sun"
        />
        <HighlightCard
          quote="A purely peer-to-peer version of electronic cash would allow online payments to be sent directly from one party to another without going through a financial institution."
          sourceTitle="Satoshi · Bitcoin Whitepaper"
          sourceSub="§1 · Dec 2025 re-read"
          marks={[{ colorIndex: 5, initials: 'SL' }]}
          replies={11}
          hot
          date="Dec"
        />
        <HighlightCard
          quote="Money is memory."
          sourceTitle="Nick Szabo · Shelling Out"
          sourceSub="three words, one thesis"
          marks={[{ colorIndex: 3, initials: 'BS' }]}
          replies={4}
          date="Jun"
        />
      </div>

      <SeeAllLink label="See all 412 highlights" href="#" />
    </Block>

    <Block id="discussions" title="Every discussion." accent="discussion.">
      {#snippet filters()}
        <div class="filter-row">
          <button type="button" class="filter-pill on">All <span class="c">38</span></button>
          <button type="button" class="filter-pill">Active <span class="c">4</span></button>
          <button type="button" class="filter-pill">Unread <span class="c">2</span></button>
          <button type="button" class="filter-pill">Books</button>
          <button type="button" class="filter-pill">Podcasts</button>
          <button type="button" class="filter-pill">Essays</button>
          <button type="button" class="filter-pill sort">Most recent ↓</button>
        </div>
      {/snippet}

      <div class="disc-list">
        <DiscussionRow
          status="active"
          title={'Thirty-nine years later and "television" is the only word you\'d need to swap.'}
          source={'on <b>Postman · Amusing Ourselves to Death</b> · ch. 6 · started by DK'}
          participants={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 4, initials: 'MB' }
          ]}
          replies={8}
          lastAt="Tue 9:03"
        />
        <DiscussionRow
          href="/room/signal-over-noise/artifact/tftc-642"
          status="active"
          title="The unit of account is the thing nobody audits."
          source={'on <b>Lyn Alden · TFTC #642</b> · 43:12 · started by Miljan'}
          participants={[
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 1, initials: 'DK' }
          ]}
          replies={3}
          lastAt="Tue 16:44"
        />
        <DiscussionRow
          href="/room/signal-over-noise/artifact/dergigi-purple"
          status="active"
          title="Gigi wrote this two months before I started the Nostr build."
          source={'on <b>Dergigi · Purple Text, Orange Highlights</b> · started by Pablo F'}
          participants={[
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 1, initials: 'DK' }
          ]}
          replies={4}
          lastAt="yesterday"
        />
        <DiscussionRow
          status="closed"
          statusLabel="Closed · Oct"
          title="Is surveillance capitalism inevitable, or can Nostr route around it?"
          source={'on <b>Shoshana Zuboff · Surveillance Capitalism</b> · started by DK · longest thread'}
          participants={[
            { colorIndex: 1, initials: 'DK' },
            { colorIndex: 2, initials: 'PF' },
            { colorIndex: 3, initials: 'BS' },
            { colorIndex: 4, initials: 'MB' },
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 6, initials: 'MW' }
          ]}
          replies={47}
          lastAt="Oct 2025"
        />
        <DiscussionRow
          status="closed"
          statusLabel="Closed · Dec"
          title="Is the Whitepaper holding up, at year 17?"
          source={'on <b>Satoshi · Bitcoin Whitepaper</b> · started by Steve L'}
          participants={[
            { colorIndex: 5, initials: 'SL' },
            { colorIndex: 6, initials: 'MW' }
          ]}
          replies={23}
          lastAt="Dec 2025"
        />
      </div>

      <SeeAllLink label="See all 38 threads" href="#" />
    </Block>

    <Block id="lately" title="Lately in the room." accent="room.">
      <ActivityFeed
        items={[
          {
            id: 'f1',
            memberColorIndex: 6,
            memberInitials: 'MW',
            memberName: 'Max W',
            action: 'marked',
            body: '&ldquo;<span class="f-ref">slow literature</span>&rdquo; in the Stewart Brand lecture.',
            time: '12m'
          },
          {
            id: 'f2',
            memberColorIndex: 4,
            memberInitials: 'MB',
            memberName: 'Miljan',
            action: 'replied',
            body: "to DK's thread on Postman — <span class=\"f-ref\">&ldquo;This is genuinely what we're building Primal against.&rdquo;</span>",
            time: '34m'
          },
          {
            id: 'f3',
            memberColorIndex: 2,
            memberInitials: 'PF',
            memberName: 'Pablo F',
            action: 'marked',
            body: '&ldquo;<span class="f-ref">they exchange images</span>&rdquo; in Postman, chapter 6.',
            time: '1h'
          },
          {
            id: 'f4',
            memberColorIndex: 3,
            memberInitials: 'BS',
            memberName: 'Bob S',
            action: 'shared',
            body: '<span class="f-ref">TFTC #642 — Lyn Alden on Broken Money</span> to the room.',
            time: '2h'
          },
          {
            id: 'f5',
            memberColorIndex: 5,
            memberInitials: 'SL',
            memberName: 'Steve L',
            action: 'voted',
            body: 'for <span class="f-ref">Broken Money</span> in the up-next.',
            time: '5h'
          },
          {
            id: 'f6',
            memberColorIndex: 1,
            memberInitials: 'DK',
            memberName: 'DK',
            action: 'started',
            body: 'a thread on the Postman opening passage.',
            time: 'Mon'
          }
        ]}
      />
    </Block>
  </div>

  <aside class="sidebar">
    <MembersSidebar members={sidebarMembers} />
    <UpNextVoting items={upNextItems} />
    <CaptureCta />
  </aside>
</div>

<style>
  .room-main {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--grid-sidebar);
    gap: var(--grid-gap);
    padding: 44px 0 80px;
  }

  @media (max-width: 1060px) {
    .room-main {
      grid-template-columns: 1fr;
      gap: 32px;
    }
  }

  .room-content {
    min-width: 0;
  }

  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  @media (min-width: 1060px) {
    .sidebar {
      position: sticky;
      top: 112px;
      align-self: start;
      max-height: calc(100vh - 140px);
      overflow-y: auto;
    }
  }

  /* ── This week ─────────────────────────────── */
  .also-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  @media (max-width: 760px) {
    .also-grid {
      grid-template-columns: 1fr;
    }
  }

  /* ── Shelf ─────────────────────────────────── */
  .shelf-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 14px;
  }

  /* ── Highlights reel ──────────────────────── */
  .hl-reel {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 14px;
  }

  /* ── Discussions list ─────────────────────── */
  .disc-list {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    overflow: hidden;
  }

  /* ── Filter row (shelf, etc.) ──────────────── */
  .filter-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 20px;
  }

  .filter-pill {
    padding: 6px 12px;
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: 999px;
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    color: var(--ink-soft);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    transition: all 150ms ease;
  }

  .filter-pill.on {
    background: var(--ink);
    color: var(--surface);
    border-color: var(--ink);
  }

  .filter-pill:hover:not(.on) {
    border-color: var(--ink);
    color: var(--ink);
  }

  .filter-pill.sort {
    margin-left: auto;
  }

  .filter-pill .c {
    font-family: var(--font-mono);
    font-size: 10px;
    opacity: 0.7;
    font-weight: 400;
  }
</style>
