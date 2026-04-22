<script lang="ts">
  import type { PageProps } from './$types';

  let { data }: PageProps = $props();
</script>

<section class="max-w-3xl">
  <h1 class="font-serif text-4xl font-bold text-base-content mb-1">Changelog</h1>
  <p class="text-base-content/50 text-sm mb-8">Recent commits deployed to beta</p>

  <ul class="list-none p-0 flex flex-col">
    {#each data.commits as commit}
      <li class="flex items-baseline gap-3 py-2.5 border-b border-base-300">
        <code class="text-xs text-primary shrink-0">{commit.shortHash}</code>
        <span class="flex-1 text-sm text-base-content">{commit.message}</span>
        <time class="text-xs text-base-content/50 shrink-0" datetime={commit.date}>
          {new Date(commit.date).toLocaleDateString('en-US', {
            month: 'short',
            day: 'numeric',
            year: 'numeric'
          })}
        </time>
      </li>
    {/each}
  </ul>
</section>
