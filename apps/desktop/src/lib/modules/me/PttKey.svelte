<script lang="ts">
  // The push-to-talk key, as „Audio und Sprechen“ shows it in the hold mode and „Tasten“
  // next to the mute and deafen keys. The picker itself is shared (A3).
  import KeyPicker from './KeyPicker.svelte';
</script>

<KeyPicker
  action="ptt"
  warnWithout="Ohne Taste geht dein Mikro in diesem Modus nie an."
  note="Gut sind Tasten, die das Spiel nicht braucht – zum Beispiel eine Seitentaste der Maus."
/>
