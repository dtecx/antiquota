<!-- Owns the small shared workbench icon set. -->
<script lang="ts">
  let {
    name,
    size = 16,
  }: {
    name:
      'plus' | 'folder' | 'chat' | 'menu' | 'edit' | 'send' | 'stop' | 'close' | 'search' | 'pin';
    size?: number;
  } = $props();
  const paths = {
    plus: 'M8 3v10M3 8h10',
    folder: 'M2 4h4l1.5 2H14v7H2z',
    chat: 'M2 3h12v8H6l-4 3z',
    menu: 'M2 4h12M2 8h12M2 12h12',
    edit: 'M10.5 2.5l3 3-8 8H2.5v-3z',
    send: 'M8 13V3M3 8l5-5 5 5',
    stop: 'M4 4h8v8H4z',
    close: 'M4 4l8 8M12 4l-8 8',
    search: 'M10.5 10.5l3 3M11 6.8a4.2 4.2 0 1 1-8.4 0 4.2 4.2 0 0 1 8.4 0',
    pin: 'M5 2h6l-1 5 3 3H3l3-3zM8 10v4',
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.25"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"><path d={paths[name]} /></svg
>
