<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import BookCoverLg from './BookCoverLg.svelte';
  import { memberTint } from '../utils/colors';

  interface Reader {
    pubkey: string;
    colorIndex: number;
  }

  interface Stat {
    value: string;
    label: string;
  }

  let {
    title,
    subtitle,
    coverTitle,
    coverAuthor,
    coverKicker,
    coverVariant = 'dark',
    image,
    stats,
    readers,
    readersNote,
    openHref = '#',
    continueHref = '#',
    continueLabel = 'Continue reading'
  }: {
    title: string;
    subtitle?: string;
    coverTitle: string;
    coverAuthor?: string;
    coverKicker?: string;
    coverVariant?: 'dark' | 'red' | 'blue' | 'green' | 'plum';
    image?: string;
    stats?: Stat[];
    readers?: Reader[];
    readersNote?: string;
    openHref?: string;
    continueHref?: string;
    continueLabel?: string;
  } = $props();
</script>

<div class="grid grid-cols-[140px_1fr_auto] items-start gap-7 border-b border-base-300 px-8 pb-6 pt-7 max-md:grid-cols-[100px_1fr] max-md:gap-5 max-md:p-5">
  <div class="w-[140px] max-md:w-[100px]">
    {#if image}
      <div class="aspect-[2/3] w-full overflow-hidden rounded shadow-[3px_3px_14px_rgba(0,0,0,0.18)]">
        <img class="size-full object-cover" src={image} alt={coverTitle} />
      </div>
    {:else}
      <BookCoverLg
        title={coverTitle}
        author={coverAuthor}
        kicker={coverKicker}
        variant={coverVariant}
      />
    {/if}
  </div>

  <div>
    <h3 class="m-0 mb-1 text-[26px] font-semibold leading-tight tracking-tight text-base-content">{title}</h3>
    {#if subtitle}
      <div class="mb-4 text-sm italic text-base-content/60">{subtitle}</div>
    {/if}

    {#if stats && stats.length}
      <div class="flex flex-wrap gap-5 text-[13px] text-base-content/60">
        {#each stats as stat (stat.label)}
          <span><b class="text-sm font-semibold text-base-content">{stat.value}</b> {stat.label}</span>
        {/each}
      </div>
    {/if}

    {#if readers && readers.length}
      <div class="mt-4 flex items-center gap-1.5 text-xs text-base-content/60 [&_.room-member-avatar]:shadow-[0_0_0_1px_white]">
        {#each readers as reader (reader.pubkey)}
          <User.Root {ndk} pubkey={reader.pubkey}>
            <span
              class="room-member-avatar"
              style:--mav-size="22px"
              style:--mav-ring={memberTint(reader.colorIndex)}
              style:--mav-ring-width="1.5px"
            >
              <User.Avatar />
            </span>
          </User.Root>
        {/each}
        {#if readersNote}
          <span class="ml-2 text-[12.5px] italic text-base-content/60">{readersNote}</span>
        {/if}
      </div>
    {/if}
  </div>

  <div class="flex items-start gap-2.5 max-md:col-span-full max-md:justify-self-start">
    <a class="btn btn-sm btn-outline rounded-none whitespace-nowrap text-xs font-medium" href={openHref}>Open artifact</a>
    <a class="btn btn-sm btn-neutral rounded-none whitespace-nowrap text-xs font-medium" href={continueHref}>{continueLabel}</a>
  </div>
</div>
