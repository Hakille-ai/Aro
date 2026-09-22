<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Mic from "@lucide/svelte/icons/mic";
  import MicOff from "@lucide/svelte/icons/mic-off";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import {
    voiceState,
    audioLevel,
    startVoiceSession,
    stopVoiceSession,
    stopSpeaking,
  } from "./voice-service";

  export let size: number = 80;
  export let interactive: boolean = true;
  export let showStateLabel: boolean = false;
  export let language: "fr" | "en" = "fr";

  let canvas: HTMLCanvasElement;
  let animationId: number;
  let rotation = 0;
  let pulsePhase = 0;

  function handleClick() {
    if (!interactive) return;
    if ($voiceState === "listening") {
      stopVoiceSession();
    } else if ($voiceState === "speaking") {
      stopSpeaking();
    } else {
      startVoiceSession();
    }
  }

  onMount(() => {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    function render() {
      if (!ctx || !canvas) return;
      const width = canvas.width;
      const height = canvas.height;
      const centerX = width / 2;
      const centerY = height / 2;
      const baseRadius = (size / 2) * 0.65;

      ctx.clearRect(0, 0, width, height);

      // State parameters
      const isListening = $voiceState === "listening";
      const isThinking = $voiceState === "thinking";
      const isSpeaking = $voiceState === "speaking";
      const currentLevel = $audioLevel;

      pulsePhase += 0.04;
      rotation += isThinking ? 0.06 : 0.015;

      // Dynamic radius modulation
      const levelBoost = currentLevel * 18;
      const breathing = Math.sin(pulsePhase) * 3;
      const effectiveRadius = Math.max(10, baseRadius + levelBoost + (isListening || isSpeaking ? breathing : breathing * 0.5));

      // 1. Atmospheric Outer Aura Glow
      const auraGradient = ctx.createRadialGradient(
        centerX,
        centerY,
        effectiveRadius * 0.4,
        centerX,
        centerY,
        effectiveRadius * 1.8
      );

      if (isThinking) {
        auraGradient.addColorStop(0, "rgba(175, 82, 222, 0.5)"); // Violet
        auraGradient.addColorStop(0.5, "rgba(0, 113, 227, 0.3)"); // Electric blue
        auraGradient.addColorStop(1, "rgba(255, 45, 85, 0)"); // Pink fade
      } else if (isListening) {
        auraGradient.addColorStop(0, "rgba(0, 122, 255, 0.6)"); // Siri cyan/blue
        auraGradient.addColorStop(0.6, "rgba(88, 86, 214, 0.35)"); // Indigo
        auraGradient.addColorStop(1, "rgba(0, 245, 255, 0)");
      } else if (isSpeaking) {
        auraGradient.addColorStop(0, "rgba(50, 215, 75, 0.55)"); // Emerald
        auraGradient.addColorStop(0.5, "rgba(0, 199, 190, 0.35)"); // Mint
        auraGradient.addColorStop(1, "rgba(48, 209, 88, 0)");
      } else {
        // Idle ambient Apple Intelligence glow
        auraGradient.addColorStop(0, "rgba(175, 82, 222, 0.4)");
        auraGradient.addColorStop(0.5, "rgba(0, 113, 227, 0.2)");
        auraGradient.addColorStop(1, "rgba(0, 0, 0, 0)");
      }

      ctx.fillStyle = auraGradient;
      ctx.beginPath();
      ctx.arc(centerX, centerY, effectiveRadius * 1.8, 0, Math.PI * 2);
      ctx.fill();

      // 2. Soundwave Ripples (Listening or Speaking)
      if ((isListening || isSpeaking) && currentLevel > 0.1) {
        ctx.strokeStyle = isListening ? "rgba(0, 122, 255, 0.4)" : "rgba(48, 209, 88, 0.4)";
        ctx.lineWidth = 1.5;
        const rippleR = effectiveRadius + currentLevel * 24;
        ctx.beginPath();
        ctx.arc(centerX, centerY, rippleR, 0, Math.PI * 2);
        ctx.stroke();
      }

      // 3. Fluid Organic Luminous Core
      ctx.save();
      ctx.translate(centerX, centerY);
      ctx.rotate(rotation);

      const coreGradient = ctx.createLinearGradient(
        -effectiveRadius,
        -effectiveRadius,
        effectiveRadius,
        effectiveRadius
      );

      if (isThinking) {
        coreGradient.addColorStop(0, "#bf5af2");
        coreGradient.addColorStop(0.5, "#5e5ce6");
        coreGradient.addColorStop(1, "#ff375f");
      } else if (isListening) {
        coreGradient.addColorStop(0, "#0a84ff");
        coreGradient.addColorStop(0.5, "#5ac8fa");
        coreGradient.addColorStop(1, "#bf5af2");
      } else if (isSpeaking) {
        coreGradient.addColorStop(0, "#30d158");
        coreGradient.addColorStop(0.5, "#66d4cf");
        coreGradient.addColorStop(1, "#0a84ff");
      } else {
        coreGradient.addColorStop(0, "#8b5cf6");
        coreGradient.addColorStop(0.5, "#3b82f6");
        coreGradient.addColorStop(1, "#ec4899");
      }

      ctx.fillStyle = coreGradient;
      ctx.beginPath();

      // Multi-point fluid perimeter
      const points = 8;
      for (let i = 0; i < points; i++) {
        const angle = (i / points) * Math.PI * 2;
        const wobble = Math.sin(pulsePhase * 2 + i) * (currentLevel * 8 + 2);
        const r = effectiveRadius + wobble;
        const px = Math.cos(angle) * r;
        const py = Math.sin(angle) * r;
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      ctx.closePath();
      ctx.fill();

      // 4. Bright Specular Center Flare
      const centerFlare = ctx.createRadialGradient(
        -effectiveRadius * 0.2,
        -effectiveRadius * 0.2,
        2,
        0,
        0,
        effectiveRadius * 0.9
      );
      centerFlare.addColorStop(0, "rgba(255, 255, 255, 0.85)");
      centerFlare.addColorStop(0.4, "rgba(255, 255, 255, 0.2)");
      centerFlare.addColorStop(1, "rgba(255, 255, 255, 0)");
      ctx.fillStyle = centerFlare;
      ctx.beginPath();
      ctx.arc(0, 0, effectiveRadius, 0, Math.PI * 2);
      ctx.fill();

      ctx.restore();

      animationId = requestAnimationFrame(render);
    }

    render();
  });

  onDestroy(() => {
    if (animationId) {
      cancelAnimationFrame(animationId);
    }
  });

  $: stateLabel = {
    idle: language === "fr" ? "Prêt à écouter" : "Ready to listen",
    listening: language === "fr" ? "Écoute en direct..." : "Listening...",
    thinking: language === "fr" ? "L'IA réfléchit..." : "Reasoning...",
    speaking: language === "fr" ? "L'IA vous répond" : "Speaking...",
    error: language === "fr" ? "Microphone indisponible" : "Mic unavailable",
  }[$voiceState];
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="voice-orb-container"
  class:interactive
  class:active={$voiceState === "listening" || $voiceState === "speaking"}
  on:click={handleClick}
  title={interactive ? (language === "fr" ? "Cliquez pour converser par la voix avec ARO" : "Click to speak with ARO") : undefined}
>
  <div class="canvas-wrapper" style="width: {size * 1.6}px; height: {size * 1.6}px;">
    <canvas
      bind:this={canvas}
      width={Math.round(size * 1.6 * 2)}
      height={Math.round(size * 1.6 * 2)}
      style="width: {size * 1.6}px; height: {size * 1.6}px;"
    ></canvas>
    <!-- Center icon overlay for high clarity -->
    <div class="orb-center-icon">
      {#if $voiceState === "listening"}
        <Mic size={Math.max(14, Math.round(size * 0.26))} class="state-icon" />
      {:else if $voiceState === "thinking"}
        <Sparkles size={Math.max(14, Math.round(size * 0.26))} class="state-icon" />
      {:else if $voiceState === "speaking"}
        <Volume2 size={Math.max(14, Math.round(size * 0.26))} class="state-icon" />
      {:else if $voiceState === "error"}
        <AlertCircle size={Math.max(14, Math.round(size * 0.26))} class="state-icon error" />
      {:else}
        <Mic size={Math.max(14, Math.round(size * 0.24))} class="state-icon idle" />
      {/if}
    </div>
  </div>

  {#if showStateLabel}
    <div class="orb-state-label animate-fade-in" class:live={$voiceState !== "idle"}>
      {#if $voiceState !== "idle"}
        <span class="live-dot"></span>
      {/if}
      <span>{stateLabel}</span>
    </div>
  {/if}
</div>

<style>
  .voice-orb-container {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    position: relative;
    user-select: none;
  }

  .voice-orb-container.interactive {
    cursor: pointer;
  }

  .canvas-wrapper {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: transform 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .voice-orb-container.interactive:hover .canvas-wrapper {
    transform: scale(1.06);
  }

  .voice-orb-container.interactive:active .canvas-wrapper {
    transform: scale(0.96);
  }

  canvas {
    display: block;
    filter: drop-shadow(0 0 16px rgba(175, 82, 222, 0.35));
  }

  .orb-center-icon {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    pointer-events: none;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ffffff;
    filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.4));
  }

  .state-icon.idle {
    opacity: 0.85;
  }

  .state-icon.error {
    color: #ff453a;
  }

  .orb-state-label {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    font-size: 12px;
    font-weight: 500;
    color: #a1a1aa;
    letter-spacing: 0.3px;
  }

  .orb-state-label.live {
    color: #ffffff;
    font-weight: 600;
  }

  .live-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #bf5af2;
    animation: livePulse 1.2s infinite;
  }

  @keyframes livePulse {
    0%, 100% { transform: scale(0.8); opacity: 0.5; }
    50% { transform: scale(1.4); opacity: 1; }
  }
</style>
