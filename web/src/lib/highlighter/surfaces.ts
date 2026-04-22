export type SurfaceAction = {
  href: string;
  label: string;
  tone?: 'primary' | 'secondary';
};

export type SurfaceSection = {
  title: string;
  items: string[];
};

export type SurfaceSpec = {
  title: string;
  description: string;
  status: string;
  actions?: SurfaceAction[];
  sections: SurfaceSection[];
  note?: string;
};

export type LaunchCard = {
  href: string;
  label: string;
  description: string;
  status: string;
};

export const launchCards: LaunchCard[] = [
  {
    href: '/rooms',
    label: 'Rooms',
    description: 'Your NIP-29 rooms, creation flow, and room-specific reading surfaces.',
    status: 'Foundation'
  },
  {
    href: '/discover',
    label: 'Discover',
    description: 'Public room browsing for guests and future recommendation entry points.',
    status: 'Foundation'
  },
  {
    href: '/me',
    label: 'Me',
    description: 'Personal vault, room membership list, and a standard NIP-51 For Later queue.',
    status: 'Protected'
  },
  {
    href: '/share/community/demo-group',
    label: 'Public Room',
    description: 'Shareable SSR page for a room landing page before full relay-backed data arrives.',
    status: 'Stub'
  },
  {
    href: '/r/demo-group/e/demo-highlight',
    label: 'Public Highlight',
    description: 'New canonical share shape for highlights: room context plus canonical highlight event.',
    status: 'New route'
  }
];

export const guestActions: SurfaceAction[] = [
  { href: '/discover', label: 'Explore public rooms' },
  { href: '/onboarding', label: 'Start a room', tone: 'secondary' }
];

export const memberActions: SurfaceAction[] = [
  { href: '/r/create', label: 'Create a room' },
  { href: '/me', label: 'Open my vault', tone: 'secondary' }
];

export const artifactSpec: SurfaceSpec = {
  title: 'Source detail is the reading room.',
  description:
    'This route will become the place where metadata, highlights, discussion, and saving flows converge around one shared source.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/rooms', label: 'Back to rooms', tone: 'secondary' },
    { href: '/r/create', label: 'Create a test group' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Reader header with source, author, link, and hero media.',
        'Group-shared highlights keyed to the source address.',
        'Discussion and For Later entry points tied to the same source.'
      ]
    },
    {
      title: 'Protocol notes',
      items: [
        'Shared sources are addressable events keyed by a d tag URL hash.',
        'Highlights reference sources through a tags, not raw event ids.',
        'Duplicate shares should resolve to the same source address inside a group.'
      ]
    }
  ]
};

export const discussionSpec: SurfaceSpec = {
  title: 'Threaded conversation lands here.',
  description:
    'Artifact and highlight discussion both resolve to NIP-22 comment threads, with root scope and parent scope kept separate.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/rooms', label: 'Back to rooms', tone: 'secondary' },
    { href: '/discover', label: 'See public surfaces' }
  ],
  sections: [
    {
      title: 'Planned interaction',
      items: [
        'Root comments plus visually nested replies.',
        'Reply composer with correct uppercase and lowercase NIP-22 tags.',
        'Reaction counts and optimistic comment posting.'
      ]
    },
    {
      title: 'Protocol notes',
      items: [
        'Uppercase tags define the root of the thread.',
        'Lowercase tags define the direct parent for this comment.',
        'Group routing continues to rely on the h tag.'
      ]
    }
  ]
};

export const discoverSpec: SurfaceSpec = {
  title: 'Public room discovery starts here.',
  description:
    'This route is the guest-friendly entrance to Highlighter and the future seed for recommendations, search, and public room growth loops.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/rooms', label: 'Back to rooms', tone: 'secondary' },
    { href: '/onboarding', label: 'Create a profile' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Public cards for open and visible rooms.',
        'Context about why a group is worth joining before a user signs in.',
        'Entry points into public share pages and onboarding.'
      ]
    },
    {
      title: 'Out of scope for this slice',
      items: [
        'Ranking and recommendation logic.',
        'Full-text search across public rooms.',
        'Any authenticated group content.'
      ]
    }
  ]
};

export const shareCommunitySpec: SurfaceSpec = {
  title: 'This route becomes the shareable group landing page.',
  description:
    'It should render without JavaScript, surface only public-safe metadata, and make room joins legible to someone seeing Highlighter for the first time.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/discover', label: 'Back to discover', tone: 'secondary' },
    { href: '/onboarding', label: 'Join Highlighter' }
  ],
  sections: [
    {
      title: 'SSR expectations',
      items: [
        'Load public-safe kind:39000 metadata on the server.',
        'Hide or reject private and closed room data when appropriate.',
        'Emit clean metadata for search engines and social previews.'
      ]
    },
    {
      title: 'Growth role',
      items: [
        'This is the public invitation layer for rooms.',
        'The page should clearly explain the group, not just mirror the app UI.',
        'Join and request-invite CTAs should reflect group access rules.'
      ]
    }
  ]
};

export const shareHighlightSpec: SurfaceSpec = {
  title: 'Highlight sharing now uses group context in the URL.',
  description:
    'The share route is `/r/[slug]/e/[id]`, which keeps a canonical highlight event tied to the room context it was shared into.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/share/community/demo-group', label: 'View public room', tone: 'secondary' },
    { href: '/discover', label: 'Explore more public surfaces' }
  ],
  sections: [
    {
      title: 'Route semantics',
      items: [
        'The same kind:9802 highlight can be shared into multiple rooms.',
        'The group id disambiguates which room context this public card represents.',
        'The loader should resolve the kind:16 repost first, then fetch the referenced highlight.'
      ]
    },
    {
      title: 'Protocol notes',
      items: [
        'Canonical highlights stay group-neutral and carry no h tag.',
        'Room sharing uses a kind:16 repost with an h tag.',
        'Public cards should resolve source context from the highlight a tag.'
      ]
    }
  ]
};

export const meHighlightsSpec: SurfaceSpec = {
  title: 'Your personal highlight vault will live here.',
  description:
    'This view becomes the canonical list of highlights you authored, independent of which rooms they were shared into.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/me', label: 'Back to my vault', tone: 'secondary' },
    { href: '/discover', label: 'Find a room' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Newest-first list of your kind:9802 events.',
        'Artifact source context alongside the quote.',
        'Share-again actions for sending the same highlight into additional groups.'
      ]
    }
  ]
};

export const meForLaterSpec: SurfaceSpec = {
  title: 'For Later bookmarks live here.',
  description:
    'This queue uses standard NIP-51 bookmark tags so saved sources stay tied to your Nostr identity.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/me', label: 'Back to my vault', tone: 'secondary' },
    { href: '/discover', label: 'Browse public rooms' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Plain NIP-51 a, e, and r bookmarks.',
        'Quick actions for moving an item into a room.',
        'Portable saved sources across clients that understand bookmark lists.'
      ]
    }
  ]
};

export const meRecommendedSpec: SurfaceSpec = {
  title: 'Recommendations are reserved for a later milestone.',
  description:
    'This route exists now so navigation is stable before recommendation logic and room discovery ranking arrive.',
  status: 'Placeholder',
  actions: [
    { href: '/discover', label: 'Use discover instead' },
    { href: '/me', label: 'Back to my vault', tone: 'secondary' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Discovery suggestions based on rooms and sources you interact with.',
        'A bridge from public discovery into your personal vault.',
        'Low-noise recommendations that feel useful rather than feed-like.'
      ]
    }
  ]
};
