<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  export let active: boolean = false;
  export let color: string = "#3b82f6";
  export let height: number = 40;

  let canvasElement: HTMLCanvasElement;
  let animationId: number;

  onMount(() => {
    startAnimation();
  });

  onDestroy(() => {
    if (animationId) cancelAnimationFrame(animationId);
  });

  $: if (active) {
    startAnimation();
  }

  function startAnimation() {
    if (!canvasElement) return;
    const ctx = canvasElement.getContext("2d");
    if (!ctx) return;

    let step = 0;
    function render() {
      if (!ctx || !canvasElement) return;
      ctx.clearRect(0, 0, canvasElement.width, canvasElement.height);

      const numBars = 32;
      const barWidth = canvasElement.width / numBars - 2;

      for (let i = 0; i < numBars; i++) {
        const value = active
          ? Math.sin(step * 0.1 + i * 0.3) * 0.4 + Math.cos(step * 0.15 + i * 0.2) * 0.4 + 0.2
          : 0.1;
        const barHeight = Math.max(4, value * canvasElement.height * 0.8);
        const x = i * (barWidth + 2);
        const y = (canvasElement.height - barHeight) / 2;

        ctx.fillStyle = color;
        ctx.shadowColor = color;
        ctx.shadowBlur = active ? 8 : 0;
        ctx.beginPath();
        ctx.roundRect(x, y, barWidth, barHeight, 2);
        ctx.fill();
      }

      step++;
      animationId = requestAnimationFrame(render);
    }
    render();
  }
</script>

<div class="waveform-container" style="height: {height}px;">
  <canvas bind:this={canvasElement} width="280" height={height}></canvas>
</div>

<style>
  .waveform-container {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    overflow: hidden;
  }

  canvas {
    width: 100%;
    height: 100%;
  }
</style>
