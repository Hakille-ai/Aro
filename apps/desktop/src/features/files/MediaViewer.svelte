<script lang="ts">
  import Play from "@lucide/svelte/icons/play";
  import Pause from "@lucide/svelte/icons/pause";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import Music from "@lucide/svelte/icons/music";
  import Video from "@lucide/svelte/icons/video";

  export let url: string;
  export let fileName: string;
  export let mimeType: string;
  export let theme: "light" | "dark" = "dark";

  $: isAudio = mimeType.startsWith("audio/") || /\.(mp3|wav|ogg|m4a|aac|flac)$/i.test(fileName);
  $: isVideo = mimeType.startsWith("video/") || /\.(mp4|webm|mov|mkv)$/i.test(fileName);

  let mediaEl: HTMLMediaElement;
  let isPlaying = false;
  let currentTime = 0;
  let duration = 0;
  let volume = 1.0;
  let isMuted = false;

  function togglePlay() {
    if (!mediaEl) return;
    if (isPlaying) {
      mediaEl.pause();
      isPlaying = false;
    } else {
      mediaEl.play();
      isPlaying = true;
    }
  }

  function handleTimeUpdate() {
    if (mediaEl) {
      currentTime = mediaEl.currentTime;
      duration = mediaEl.duration || 0;
    }
  }

  function handleSeek(e: Event) {
    const target = e.target as HTMLInputElement;
    const newTime = parseFloat(target.value);
    if (mediaEl) {
      mediaEl.currentTime = newTime;
      currentTime = newTime;
    }
  }

  function toggleMute() {
    if (!mediaEl) return;
    mediaEl.muted = !isMuted;
    isMuted = !isMuted;
  }

  function handleVolumeChange(e: Event) {
    const target = e.target as HTMLInputElement;
    volume = parseFloat(target.value);
    if (mediaEl) {
      mediaEl.volume = volume;
      isMuted = volume === 0;
    }
  }

  function formatDuration(sec: number): string {
    if (isNaN(sec) || sec === 0) return "0:00";
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s < 10 ? "0" : ""}${s}`;
  }
</script>

<div class="media-viewer-container" class:dark={theme === "dark"}>
  {#if isVideo}
    <div class="video-wrapper">
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={mediaEl}
        src={url}
        controls
        autoplay
        class="video-element"
        on:play={() => (isPlaying = true)}
        on:pause={() => (isPlaying = false)}
        on:timeupdate={handleTimeUpdate}
        on:loadedmetadata={handleTimeUpdate}
      ></video>
    </div>
  {:else if isAudio}
    <div class="audio-card">
      <div class="audio-visualizer-orb">
        <div class="orb-ring" class:pulse={isPlaying}></div>
        <Music size={36} class="music-icon" />
      </div>

      <div class="audio-info">
        <span class="audio-filename">{fileName}</span>
        <span class="audio-type">{mimeType || "Audio"}</span>
      </div>

      <!-- svelte-ignore a11y_media_has_caption -->
      <audio
        bind:this={mediaEl}
        src={url}
        on:play={() => (isPlaying = true)}
        on:pause={() => (isPlaying = false)}
        on:timeupdate={handleTimeUpdate}
        on:loadedmetadata={handleTimeUpdate}
        on:ended={() => (isPlaying = false)}
      ></audio>

      <!-- Custom Player Controls -->
      <div class="audio-controls">
        <button class="play-btn" type="button" on:click={togglePlay}>
          {#if isPlaying}
            <Pause size={18} />
          {:else}
            <Play size={18} style="margin-left: 2px;" />
          {/if}
        </button>

        <span class="time-label">{formatDuration(currentTime)}</span>

        <input
          type="range"
          min="0"
          max={duration || 100}
          value={currentTime}
          step="0.1"
          class="timeline-slider"
          on:input={handleSeek}
        />

        <span class="time-label total">{formatDuration(duration)}</span>

        <button class="mute-btn" type="button" on:click={toggleMute}>
          {#if isMuted || volume === 0}
            <VolumeX size={16} />
          {:else}
            <Volume2 size={16} />
          {/if}
        </button>

        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          value={isMuted ? 0 : volume}
          class="volume-slider"
          on:input={handleVolumeChange}
        />
      </div>
    </div>
  {:else}
    <div class="unsupported-media">
      <Video size={48} />
      <p>{fileName}</p>
      <a href={url} download={fileName} class="download-link">Télécharger le fichier média</a>
    </div>
  {/if}
</div>

<style>
  .media-viewer-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    padding: 24px;
    box-sizing: border-box;
  }

  .video-wrapper {
    width: 100%;
    max-width: 960px;
    height: 100%;
    max-height: 80vh;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 12px;
    overflow: hidden;
    background: #000000;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
  }

  .video-element {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .audio-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    background: #ffffff;
    padding: 36px 40px;
    border-radius: 20px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.1);
    max-width: 520px;
    width: 100%;
    border: 1px solid rgba(0, 0, 0, 0.08);
  }

  .dark .audio-card {
    background: #1c1c1e;
    border-color: rgba(255, 255, 255, 0.08);
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.4);
  }

  .audio-visualizer-orb {
    position: relative;
    width: 80px;
    height: 80px;
    border-radius: 50%;
    background: linear-gradient(135deg, #0071e3, #af52de);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ffffff;
    margin-bottom: 20px;
    box-shadow: 0 8px 24px rgba(0, 113, 227, 0.35);
  }

  .orb-ring {
    position: absolute;
    inset: -6px;
    border-radius: 50%;
    border: 2px solid rgba(175, 82, 222, 0.4);
    opacity: 0;
    transition: opacity 0.3s;
  }

  .orb-ring.pulse {
    opacity: 1;
    animation: pulse-ring 2s infinite ease-out;
  }

  @keyframes pulse-ring {
    0% { transform: scale(0.95); opacity: 0.8; }
    50% { transform: scale(1.15); opacity: 0.2; }
    100% { transform: scale(0.95); opacity: 0.8; }
  }

  .audio-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    margin-bottom: 24px;
    text-align: center;
  }

  .audio-filename {
    font-size: 16px;
    font-weight: 600;
    max-width: 400px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .audio-type {
    font-size: 12px;
    color: #86868b;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .audio-controls {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
  }

  .play-btn {
    width: 42px;
    height: 42px;
    border-radius: 50%;
    background: #0071e3;
    color: #ffffff;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: transform 0.15s, background 0.15s;
  }

  .play-btn:hover {
    background: #0077ed;
    transform: scale(1.05);
  }

  .time-label {
    font-size: 12px;
    color: #86868b;
    min-width: 34px;
    font-variant-numeric: tabular-nums;
  }

  .timeline-slider {
    flex: 1;
    accent-color: #0071e3;
    cursor: pointer;
  }

  .mute-btn {
    background: transparent;
    border: none;
    color: #86868b;
    cursor: pointer;
    display: flex;
    align-items: center;
    padding: 4px;
  }

  .mute-btn:hover {
    color: inherit;
  }

  .volume-slider {
    width: 60px;
    accent-color: #0071e3;
    cursor: pointer;
  }

  .unsupported-media {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: #86868b;
  }

  .download-link {
    color: #0071e3;
    text-decoration: underline;
    font-size: 14px;
  }
</style>
