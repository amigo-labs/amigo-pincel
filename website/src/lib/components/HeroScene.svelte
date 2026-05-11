<script lang="ts">
  // Pixel-art scene drawn inline as SVG. 64×48 grid scaled up, recognizably
  // "made-in-Pincel". Static by default, gains a subtle torch flicker animation
  // unless prefers-reduced-motion is set.

  // Palette references the PICO-8 / design-tokens scheme.
  const sky = '#1d2b53';
  const wallDark = '#5f574f';
  const wallLight = '#c2c3c7';
  const floor = '#83769c';
  const torchHandle = '#5f574f';
  const torchFlameA = '#ffa300';
  const torchFlameB = '#ffec27';
  const torchFlameC = '#ff004d';
  const sprite = '#ff77a8';
  const spriteShadow = '#7e2553';
  const spriteEye = '#fff1e8';
</script>

<div class="scene-wrap">
  <svg
    viewBox="0 0 64 48"
    width="100%"
    height="auto"
    role="img"
    aria-label="A small pink character standing in a stone room lit by a flickering torch."
    shape-rendering="crispEdges"
    class="pixelated scene"
  >
    <!-- Sky / background -->
    <rect x="0" y="0" width="64" height="48" fill={sky} />

    <!-- Stone wall (tilemap-like) -->
    <g fill={wallDark}>
      <rect x="0" y="0" width="64" height="24" />
    </g>
    <g fill={wallLight} opacity="0.35">
      <!-- horizontal mortar lines -->
      <rect x="0" y="6" width="64" height="1" />
      <rect x="0" y="14" width="64" height="1" />
      <rect x="0" y="22" width="64" height="1" />
      <!-- vertical mortar lines (offset rows) -->
      <rect x="8" y="0" width="1" height="6" />
      <rect x="24" y="0" width="1" height="6" />
      <rect x="40" y="0" width="1" height="6" />
      <rect x="56" y="0" width="1" height="6" />
      <rect x="0" y="7" width="1" height="7" />
      <rect x="16" y="7" width="1" height="7" />
      <rect x="32" y="7" width="1" height="7" />
      <rect x="48" y="7" width="1" height="7" />
      <rect x="8" y="15" width="1" height="7" />
      <rect x="24" y="15" width="1" height="7" />
      <rect x="40" y="15" width="1" height="7" />
      <rect x="56" y="15" width="1" height="7" />
    </g>

    <!-- Floor -->
    <rect x="0" y="38" width="64" height="10" fill={floor} />
    <rect x="0" y="38" width="64" height="1" fill={wallDark} />
    <!-- floor planks -->
    <g fill={wallDark} opacity="0.4">
      <rect x="0" y="42" width="64" height="1" />
      <rect x="0" y="46" width="64" height="1" />
    </g>

    <!-- Torch on the right -->
    <g>
      <!-- handle -->
      <rect x="52" y="14" width="2" height="10" fill={torchHandle} />
      <rect x="51" y="13" width="4" height="1" fill={torchHandle} />
      <!-- flame, animated -->
      <g class="flame">
        <rect x="52" y="9" width="2" height="4" fill={torchFlameC} />
        <rect x="51" y="11" width="4" height="2" fill={torchFlameA} />
        <rect x="52" y="7" width="2" height="2" fill={torchFlameB} />
        <rect x="53" y="6" width="1" height="1" fill={torchFlameB} />
      </g>
    </g>

    <!-- Character -->
    <g transform="translate(20 24)">
      <!-- body -->
      <rect x="3" y="6" width="6" height="6" fill={sprite} />
      <rect x="3" y="12" width="6" height="2" fill={spriteShadow} />
      <!-- head -->
      <rect x="2" y="0" width="8" height="6" fill={sprite} />
      <!-- eyes -->
      <rect x="4" y="2" width="1" height="1" fill={spriteEye} />
      <rect x="7" y="2" width="1" height="1" fill={spriteEye} />
      <!-- legs -->
      <rect x="3" y="14" width="2" height="2" fill={spriteShadow} />
      <rect x="7" y="14" width="2" height="2" fill={spriteShadow} />
    </g>

    <!-- Floor torch glow -->
    <g opacity="0.18" class="glow">
      <rect x="44" y="36" width="20" height="2" fill={torchFlameA} />
      <rect x="40" y="38" width="24" height="2" fill={torchFlameA} />
    </g>
  </svg>
</div>

<style>
  .scene-wrap {
    width: 100%;
    max-width: 480px;
    aspect-ratio: 4 / 3;
    background-color: var(--color-bg-card);
    border: 2px solid var(--color-bg-base);
    box-shadow: 6px 6px 0 var(--color-bg-base), 0 0 0 1px var(--color-border-strong);
    position: relative;
    overflow: hidden;
  }
  .scene {
    width: 100%;
    height: 100%;
    display: block;
  }
  .flame {
    transform-origin: 53px 11px;
    animation: flicker 480ms steps(2, end) infinite;
  }
  .glow {
    animation: glow 480ms steps(2, end) infinite;
  }
  @keyframes flicker {
    0% {
      transform: scale(1, 1);
    }
    50% {
      transform: scale(1, 0.9) translateY(0.5px);
    }
    100% {
      transform: scale(1, 1);
    }
  }
  @keyframes glow {
    0% {
      opacity: 0.18;
    }
    50% {
      opacity: 0.12;
    }
    100% {
      opacity: 0.18;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .flame,
    .glow {
      animation: none;
    }
  }
</style>
