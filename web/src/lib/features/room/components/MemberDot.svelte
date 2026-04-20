<script lang="ts">
  import { getContext } from 'svelte';
  import { USER_CONTEXT_KEY, type UserContext } from '$lib/ndk/ui/user/user.context';

  type SizePreset = 'sm' | 'md' | 'lg';

  let {
    colorIndex,
    size = 'md',
    pubkey,
    initials: explicitInitials,
    title: explicitTitle,
    online = false
  }: {
    colorIndex: number;
    size?: SizePreset | number;
    pubkey?: string;
    initials?: string;
    title?: string;
    online?: boolean;
  } = $props();

  const TINTS = [
    'var(--h-amber)',
    'var(--h-sage)',
    'var(--h-blue)',
    'var(--h-rose)',
    'var(--h-lilac)',
    'var(--h-amber-l)'
  ] as const;

  const PRESET_PX: Record<SizePreset, number> = { sm: 24, md: 36, lg: 48 };

  // If caller is wrapped in <User.Root>, derive initials + title from the profile there.
  const userCtx = getContext<UserContext | undefined>(USER_CONTEXT_KEY);

  const tint = $derived(TINTS[((colorIndex - 1) % 6 + 6) % 6]);
  const px = $derived(typeof size === 'number' ? size : PRESET_PX[size]);
  const fontSize = $derived(Math.max(9, Math.round(px * 0.32)));

  const derivedFromProfile = $derived.by(() => {
    const name =
      userCtx?.profile?.displayName ||
      userCtx?.profile?.name ||
      userCtx?.profile?.nip05?.split('@')[0] ||
      '';
    if (!name) return { initials: '', title: '' };
    const parts = name.trim().split(/\s+/).filter(Boolean);
    const initials =
      (parts.length === 1 ? parts[0].slice(0, 2) : (parts[0][0] ?? '') + (parts[1][0] ?? ''))
        .toUpperCase() || '??';
    return { initials, title: name };
  });

  const pubkeyFallback = $derived(pubkey ? pubkey.slice(0, 2).toUpperCase() : '');

  const initials = $derived(
    explicitInitials ?? (derivedFromProfile.initials || pubkeyFallback)
  );
  const title = $derived(explicitTitle ?? (derivedFromProfile.title || undefined));
</script>

<span
  class="member-dot"
  class:has-initials={!!initials}
  class:online
  style:width="{px}px"
  style:height="{px}px"
  style:line-height="{px}px"
  style:font-size="{fontSize}px"
  style:background-color={tint}
  {title}
  role={initials ? 'img' : undefined}
  aria-label={title}
>
  {#if initials}{initials}{/if}
</span>

<style>
  .member-dot {
    display: inline-block;
    border-radius: 50%;
    flex-shrink: 0;
    font-family: var(--font-sans);
    font-weight: 600;
    text-align: center;
    color: var(--ink);
    position: relative;
    vertical-align: middle;
  }

  .member-dot.online::after {
    content: '';
    position: absolute;
    bottom: 0;
    right: 0;
    width: 28%;
    height: 28%;
    min-width: 8px;
    min-height: 8px;
    border-radius: 50%;
    background: #7CAE7A;
    border: 2px solid var(--surface);
  }
</style>
