<script lang="ts">
  // The chrome: sidebar, outline, settings, search. Never the document itself.
  //
  // Phases P3 (outline, settings), P5 (vault sidebar, search) and P7 (edit
  // controls) fill this in. Keeping it empty until then is deliberate: an empty
  // chrome costs nothing, and the app shell has to prove it boots fast before
  // anything is added to it.
</script>

{#if false}
  <aside class="chrome"></aside>
{/if}

<style>
  .chrome {
    position: fixed;
    inset-block: 0;
    inset-inline-start: 0;
    inline-size: var(--sidebar-width, 15rem);
  }
</style>
