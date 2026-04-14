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
  eyebrow: string;
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
    href: '/community',
    label: 'Communities',
    description: 'Your NIP-29 groups, group creation flow, and community-specific reading surfaces.',
    status: 'Foundation'
  },
  {
    href: '/discover',
    label: 'Discover',
    description: 'Public community browsing for guests and future recommendation entry points.',
    status: 'Foundation'
  },
  {
    href: '/me',
    label: 'Me',
    description: 'Personal vault, community membership list, and the first pass at For Later storage.',
    status: 'Protected'
  },
  {
    href: '/share/community/demo-group',
    label: 'Public Community',
    description: 'Shareable SSR page for a group landing page before full relay-backed data arrives.',
    status: 'Stub'
  },
  {
    href: '/g/demo-group/e/demo-highlight',
    label: 'Public Highlight',
    description: 'New canonical share shape for highlights: group context plus canonical highlight event.',
    status: 'New route'
  }
];

export const guestActions: SurfaceAction[] = [
  { href: '/discover', label: 'Explore public communities' },
  { href: '/onboarding', label: 'Set up a profile', tone: 'secondary' }
];

export const memberActions: SurfaceAction[] = [
  { href: '/community/create', label: 'Create a community' },
  { href: '/me', label: 'Open my vault', tone: 'secondary' }
];

export const communityIndexSpec: SurfaceSpec = {
  eyebrow: 'Communities',
  title: 'Your group index starts here.',
  description:
    'This surface becomes the working home for memberships, group creation, and the reading communities you return to daily.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/community/create', label: 'Create community' },
    { href: '/discover', label: 'Browse public communities', tone: 'secondary' }
  ],
  sections: [
    {
      title: 'This deploy includes',
      items: [
        'A stable route for the community list with product framing instead of a raw TODO page.',
        'Navigation entry points for guests and signed-in members.',
        'A clean landing point for live NIP-29 subscriptions in the next milestone.'
      ]
    },
    {
      title: 'Next implementation',
      items: [
        'Load kind:39000 metadata for groups the viewer belongs to.',
        'Render membership-aware cards with member counts and recent activity.',
        'Hook create, join, leave, and invite flows to Croissant.'
      ]
    },
    {
      title: 'Protocol notes',
      items: [
        'Group metadata is sourced from kind:39000.',
        'Membership changes flow through kinds 9000, 9001, 9021, and 9022.',
        'All group-scoped content stays keyed by the h tag.'
      ]
    }
  ]
};

export const communitySpec: SurfaceSpec = {
  eyebrow: 'Community',
  title: 'A community front page will live here.',
  description:
    'This route will combine public-safe SSR metadata with client-side subscriptions for artifacts, highlights, and membership-aware actions.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/community', label: 'Back to communities', tone: 'secondary' },
    { href: '/community/create', label: 'Create another community' }
  ],
  sections: [
    {
      title: 'Planned layout',
      items: [
        'Hero header with name, picture, about copy, and membership controls.',
        'Featured artifact rail plus a denser community library below.',
        'Entry points for invites, joins, and role-aware admin actions.'
      ]
    },
    {
      title: 'Data model',
      items: [
        'SSR should only read public-safe kind:39000 metadata.',
        'Authenticated content hydrates after NIP-42-capable relay connections are ready.',
        'Member counts need a fallback when kind:39002 is partial or unavailable.'
      ]
    }
  ],
  note:
    'The next pass should split SSR metadata loading from live client subscriptions so this page never flashes empty.'
};

export const communityCreateSpec: SurfaceSpec = {
  eyebrow: 'Create Community',
  title: 'Community creation starts with structure, not noise.',
  description:
    'This route will become the multi-step flow for name, cover, access mode, visibility, and invite setup before publishing the NIP-29 create-group event.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/community', label: 'Back to communities', tone: 'secondary' },
    { href: '/discover', label: 'Review public communities' }
  ],
  sections: [
    {
      title: 'Planned flow',
      items: [
        'Collect name, description, and cover media.',
        'Choose Open or Closed access plus Public or Private visibility.',
        'Follow creation with invite links, codes, and direct member adds.'
      ]
    },
    {
      title: 'Protocol notes',
      items: [
        'Create with kind:9007, then immediately publish metadata via kind:9002.',
        'All Highlighter groups are restricted; open-write groups are out of scope.',
        'Closed groups eventually use invite codes via kind:9009.'
      ]
    }
  ]
};

export const artifactSpec: SurfaceSpec = {
  eyebrow: 'Artifact',
  title: 'Artifact detail is the reading room.',
  description:
    'The artifact route will become the place where metadata, highlights, discussion, and saving flows converge around one shared source.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/community', label: 'Back to communities', tone: 'secondary' },
    { href: '/community/create', label: 'Create a test group' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Artifact header with source, author, link, and hero media.',
        'Group-shared highlights keyed to the artifact address.',
        'Discussion and For Later entry points tied to the same source.'
      ]
    },
    {
      title: 'Protocol notes',
      items: [
        'Artifacts are addressable events keyed by a d tag URL hash.',
        'Highlights reference artifacts through a tags, not raw event ids.',
        'Duplicate shares should resolve to the same artifact address inside a group.'
      ]
    }
  ]
};

export const discussionSpec: SurfaceSpec = {
  eyebrow: 'Discussion',
  title: 'Threaded conversation lands here.',
  description:
    'Artifact and highlight discussion both resolve to NIP-22 comment threads, with root scope and parent scope kept separate.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/community', label: 'Back to communities', tone: 'secondary' },
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
  eyebrow: 'Discover',
  title: 'Public community discovery starts here.',
  description:
    'This route is the guest-friendly entrance to Highlighter and the future seed for recommendations, search, and public community growth loops.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/community', label: 'View community home', tone: 'secondary' },
    { href: '/onboarding', label: 'Create a profile' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Public cards for open and visible communities.',
        'Context about why a group is worth joining before a user signs in.',
        'Entry points into public share pages and onboarding.'
      ]
    },
    {
      title: 'Out of scope for this slice',
      items: [
        'Ranking and recommendation logic.',
        'Full-text search across public communities.',
        'Any authenticated group content.'
      ]
    }
  ]
};

export const shareCommunitySpec: SurfaceSpec = {
  eyebrow: 'Public Community',
  title: 'This route becomes the shareable group landing page.',
  description:
    'It should render without JavaScript, surface only public-safe metadata, and make community joins legible to someone seeing Highlighter for the first time.',
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
        'Hide or reject private and closed community data when appropriate.',
        'Emit clean metadata for search engines and social previews.'
      ]
    },
    {
      title: 'Growth role',
      items: [
        'This is the public invitation layer for communities.',
        'The page should clearly explain the group, not just mirror the app UI.',
        'Join and request-invite CTAs should reflect group access rules.'
      ]
    }
  ]
};

export const shareHighlightSpec: SurfaceSpec = {
  eyebrow: 'Public Highlight',
  title: 'Highlight sharing now uses group context in the URL.',
  description:
    'The share route is `/g/[group-id]/e/[highlight-id]`, which keeps a canonical highlight event tied to the community context it was shared into.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/share/community/demo-group', label: 'View public community', tone: 'secondary' },
    { href: '/discover', label: 'Explore more public surfaces' }
  ],
  sections: [
    {
      title: 'Route semantics',
      items: [
        'The same kind:9802 highlight can be shared into multiple communities.',
        'The group id disambiguates which community context this public card represents.',
        'The loader should resolve the kind:16 repost first, then fetch the referenced highlight.'
      ]
    },
    {
      title: 'Protocol notes',
      items: [
        'Canonical highlights stay group-neutral and carry no h tag.',
        'Community sharing uses a kind:16 repost with an h tag.',
        'Public cards should resolve artifact context from the highlight a tag.'
      ]
    }
  ]
};

export const meHighlightsSpec: SurfaceSpec = {
  eyebrow: 'My Highlights',
  title: 'Your personal highlight vault will live here.',
  description:
    'This view becomes the canonical list of highlights you authored, independent of which communities they were shared into.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/me', label: 'Back to my vault', tone: 'secondary' },
    { href: '/discover', label: 'Find a community' }
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

export const meCommunitiesSpec: SurfaceSpec = {
  eyebrow: 'My Communities',
  title: 'Your memberships will be summarized here.',
  description:
    'This page will give signed-in members a clean list of the communities they belong to and quick jumps back into active reading spaces.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/me', label: 'Back to my vault', tone: 'secondary' },
    { href: '/community/create', label: 'Create a community' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Membership-aware community cards.',
        'Fast entry into recent groups and invitation workflows.',
        'Counts and status pulled from group metadata plus membership events.'
      ]
    }
  ]
};

export const meForLaterSpec: SurfaceSpec = {
  eyebrow: 'For Later',
  title: 'Private saves will start here.',
  description:
    'For MVP this queue is intentionally local-first so saving an artifact does not depend on relay writes or cross-device sync.',
  status: 'Milestone 1 scaffold',
  actions: [
    { href: '/me', label: 'Back to my vault', tone: 'secondary' },
    { href: '/discover', label: 'Browse public communities' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Locally stored saved artifacts with status pills.',
        'Quick actions for moving an item into a community.',
        'A later upgrade path to encrypted NIP-51 bookmark storage.'
      ]
    }
  ]
};

export const meRecommendedSpec: SurfaceSpec = {
  eyebrow: 'Recommended',
  title: 'Recommendations are reserved for a later milestone.',
  description:
    'This route exists now so navigation is stable before recommendation logic and community discovery ranking arrive.',
  status: 'Placeholder',
  actions: [
    { href: '/discover', label: 'Use discover instead' },
    { href: '/me', label: 'Back to my vault', tone: 'secondary' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Discovery suggestions based on communities and artifacts you interact with.',
        'A bridge from public discovery into your personal vault.',
        'Low-noise recommendations that feel useful rather than feed-like.'
      ]
    }
  ]
};

export const meSynthesisSpec: SurfaceSpec = {
  eyebrow: 'Synthesis',
  title: 'Reading synthesis is intentionally a placeholder.',
  description:
    'The route is live now so the information architecture is stable before any AI-assisted synthesis work starts.',
  status: 'Placeholder',
  actions: [
    { href: '/me', label: 'Back to my vault', tone: 'secondary' },
    { href: '/me/highlights', label: 'Review my highlights' }
  ],
  sections: [
    {
      title: 'Planned content',
      items: [
        'Cross-artifact patterns discovered from your highlights.',
        'A lightweight summary layer instead of noisy chat output.',
        'Privacy decisions made explicitly before any model integration.'
      ]
    }
  ]
};
