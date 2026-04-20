<script lang="ts">
  import Passage from './Passage.svelte';
  import Thread from './Thread.svelte';

  interface Span {
    text: string;
    colorIndex?: number;
    markedBy?: string;
  }

  interface Message {
    id: string;
    pubkey: string;
    colorIndex: number;
    time: string;
    body: string;
    isReply?: boolean;
  }

  let {
    passageLabel,
    passageSpans,
    threadTitle,
    threadStarterPubkey,
    threadStartedAt,
    messages
  }: {
    passageLabel?: string;
    passageSpans: Span[];
    threadTitle?: string;
    threadStarterPubkey?: string;
    threadStartedAt?: string;
    messages: Message[];
  } = $props();
</script>

<div class="discussions-panel">
  <Passage label={passageLabel} spans={passageSpans} />
  <div class="thread-slot">
    <Thread
      title={threadTitle}
      starterPubkey={threadStarterPubkey}
      startedAt={threadStartedAt}
      {messages}
    />
  </div>
</div>

<style>
  .discussions-panel {
    display: flex;
    flex-direction: column;
  }

  .thread-slot {
    padding: 0 32px 32px;
  }

  @media (max-width: 760px) {
    .thread-slot {
      padding: 0 20px 24px;
    }
  }
</style>
