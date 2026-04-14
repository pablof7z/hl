<script lang="ts">
  import { goto } from '$app/navigation';
  import { ndk } from '$lib/ndk/client';
  import { profileHasBasics } from '$lib/onboarding';

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));

  $effect(() => {
    if (isReadOnly) return;
    if (!currentUser) return;

    const profile = currentUser.profile;
    if (ndk.$sessions !== undefined && !profileHasBasics(profile)) {
      void goto('/me/setup');
    }
  });
</script>

<svelte:head>
  <title>My Vault — Highlighter</title>
</svelte:head>

<div class="me-page">
  <div class="me-header">
    <h1 class="me-title">My Vault</h1>
    <p class="me-subtitle">
      Your profile, saved artifacts, highlight history, and future synthesis layers all collect
      here.
    </p>
  </div>

  <nav class="me-tabs">
    <a href="/me/highlights" class="me-tab">Highlights</a>
    <a href="/me/communities" class="me-tab">Communities</a>
    <a href="/me/for-later" class="me-tab">For Later</a>
    <a href="/me/recommended" class="me-tab">Recommended</a>
    <a href="/me/synthesis" class="me-tab">Synthesis</a>
  </nav>

  <div class="me-intro-grid">
    <a class="me-intro-card" href="/me/highlights">
      <span>Highlights</span>
      <p>Highlights you authored, regardless of which communities you shared them into.</p>
    </a>
    <a class="me-intro-card" href="/me/communities">
      <span>Communities</span>
      <p>Membership-aware entry points back into the groups you read and discuss inside.</p>
    </a>
    <a class="me-intro-card" href="/me/for-later">
      <span>For Later</span>
      <p>Private saves that stay local-first in MVP so capture is fast and low-friction.</p>
    </a>
  </div>
</div>

<style>
  .me-page {
    display: grid;
    gap: 2rem;
    padding-top: 1rem;
  }

  .me-header {
    display: grid;
    gap: 0.35rem;
  }

  .me-title {
    margin: 0;
    font-size: 2rem;
    font-weight: 700;
    color: var(--text-strong);
    letter-spacing: -0.02em;
    font-family: var(--font-serif);
  }

  .me-subtitle {
    margin: 0;
    color: var(--muted);
    font-size: 0.95rem;
  }

  .me-tabs {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .me-tab {
    padding: 0.45rem 1rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    font-size: 0.88rem;
    font-weight: 500;
    text-decoration: none;
    transition: border-color 140ms, color 140ms;
  }

  .me-tab:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .me-intro-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: 0.9rem;
  }

  .me-intro-card {
    display: grid;
    gap: 0.45rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface);
    color: inherit;
    text-decoration: none;
    transition:
      border-color 140ms ease,
      transform 140ms ease;
  }

  .me-intro-card:hover {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
    transform: translateY(-1px);
  }

  .me-intro-card span {
    color: var(--text-strong);
    font-weight: 700;
  }

  .me-intro-card p {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
    line-height: 1.5;
  }
</style>
