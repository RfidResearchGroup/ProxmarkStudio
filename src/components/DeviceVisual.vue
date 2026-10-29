<script setup lang="ts">
import BrandMark from './BrandMark.vue'

defineProps<{
  connected: boolean
  connecting?: boolean
}>()
</script>

<template>
  <div class="visual" :class="{ 'visual--online': connected, 'visual--connecting': connecting }">
    <div class="visual__aura" />
    <div class="radar">
      <span class="radar__ring radar__ring--1" />
      <span class="radar__ring radar__ring--2" />
      <span class="radar__ring radar__ring--3" />
      <span class="radar__ring radar__ring--4" />
      <span class="radar__axis radar__axis--x" />
      <span class="radar__axis radar__axis--y" />
      <span class="radar__sweep" />
      <span class="radar__blip radar__blip--1" />
      <span class="radar__blip radar__blip--2" />
      <span class="radar__blip radar__blip--3" />
    </div>

    <div class="device">
      <span class="device__shadow" />
      <div class="device__body">
        <div class="device__top">
          <BrandMark :size="21" />
          <div class="device__title">
            <strong>PROXMARK</strong>
            <span>RFID</span>
          </div>
          <span class="device__power" />
        </div>
        <div class="device__screen">
          <span class="screen-wave">
            <i v-for="index in 11" :key="index" :style="{ height: `${5 + ((index * 7) % 13)}px` }" />
          </span>
          <span class="screen-label">125 / 13.56</span>
        </div>
        <div class="device__bottom">
          <span class="device__port" />
          <span class="device__vent" v-for="index in 4" :key="index" />
          <span class="device__led" />
        </div>
      </div>
    </div>

    <span class="tag tag--usb">USB</span>
    <span class="tag tag--ble">BLE</span>
    <span class="tag tag--rf">RF</span>
    <span class="visual__status">{{ connected ? 'CONNECTED' : connecting ? 'NEGOTIATING' : 'STANDBY' }}</span>
  </div>
</template>

<style scoped>
.visual {
  position: relative;
  display: grid;
  place-items: center;
  width: 100%;
  min-height: 340px;
  overflow: hidden;
  isolation: isolate;
}

.visual__aura {
  position: absolute;
  width: 300px;
  height: 300px;
  border-radius: 50%;
  background: radial-gradient(circle, var(--accent-faint), transparent 66%);
  filter: blur(2px);
  opacity: 0.7;
  transition: opacity 600ms ease;
}

.visual--online .visual__aura {
  opacity: 1;
  background: radial-gradient(circle, var(--accent-dim), transparent 68%);
}

.radar {
  position: absolute;
  width: 320px;
  height: 320px;
  border-radius: 50%;
  opacity: 0.75;
}

.radar__ring {
  position: absolute;
  inset: 0;
  margin: auto;
  border-radius: 50%;
  border: 1px solid var(--border);
  transition: border-color 600ms ease;
}

.radar__ring--1 {
  width: 320px;
  height: 320px;
  border-style: dashed;
  opacity: 0.6;
}
.radar__ring--2 {
  width: 250px;
  height: 250px;
}
.radar__ring--3 {
  width: 178px;
  height: 178px;
  border-color: var(--border-strong);
}
.radar__ring--4 {
  width: 108px;
  height: 108px;
  border-style: dashed;
}

.visual--online .radar__ring {
  border-color: var(--border-accent);
}

.radar__axis {
  position: absolute;
  background: var(--border);
}
.radar__axis--x {
  left: 0;
  right: 0;
  top: 50%;
  height: 1px;
}
.radar__axis--y {
  top: 0;
  bottom: 0;
  left: 50%;
  width: 1px;
}

.radar__sweep {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: conic-gradient(from 0deg, transparent 0deg, transparent 300deg, var(--accent-dim) 350deg, transparent 360deg);
  mask-image: radial-gradient(circle, transparent 26%, #000 27%, #000 100%);
  -webkit-mask-image: radial-gradient(circle, transparent 26%, #000 27%, #000 100%);
  animation: sweep 9s linear infinite;
  opacity: 0;
  transition: opacity 500ms ease;
}

.visual--online .radar__sweep {
  opacity: 1;
}

.radar__blip {
  position: absolute;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 9px var(--accent);
  opacity: 0;
}

.visual--online .radar__blip {
  animation: blip 3.4s ease-in-out infinite;
}

.radar__blip--1 {
  top: 22%;
  left: 68%;
}
.radar__blip--2 {
  top: 72%;
  left: 26%;
  animation-delay: 1.2s !important;
}
.radar__blip--3 {
  top: 40%;
  left: 16%;
  animation-delay: 2.1s !important;
}

@keyframes sweep {
  to {
    transform: rotate(360deg);
  }
}

@keyframes blip {
  0%,
  100% {
    opacity: 0;
    transform: scale(0.4);
  }
  20%,
  45% {
    opacity: 1;
    transform: scale(1);
  }
}

.device {
  position: relative;
  z-index: 2;
  transform: rotate(-7deg);
  transition: transform 600ms var(--ease-out);
}

.visual--online .device {
  transform: rotate(-3deg) translateY(-3px);
}

.device__shadow {
  position: absolute;
  left: 50%;
  bottom: -34px;
  width: 170px;
  height: 34px;
  transform: translateX(-50%);
  background: radial-gradient(ellipse, rgba(0, 0, 0, 0.62), transparent 68%);
  filter: blur(8px);
}

.device__body {
  width: 196px;
  padding: 12px;
  border-radius: 14px;
  border: 1px solid rgba(160, 190, 225, 0.28);
  background:
    linear-gradient(160deg, rgba(46, 61, 82, 0.96), rgba(13, 20, 31, 0.98) 58%),
    var(--panel-solid);
  box-shadow:
    0 26px 50px rgba(0, 0, 0, 0.5),
    inset 0 1px 0 rgba(255, 255, 255, 0.14),
    inset 0 -2px 8px rgba(0, 0, 0, 0.6);
  position: relative;
}

.device__body::before {
  content: '';
  position: absolute;
  inset: 2px;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.04);
  pointer-events: none;
}

.device__top {
  display: flex;
  align-items: center;
  gap: 8px;
}

.device__title {
  display: flex;
  flex-direction: column;
  line-height: 1.05;
}

.device__title strong {
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.13em;
  color: rgba(235, 245, 255, 0.9);
}

.device__title span {
  margin-top: 2px;
  font-family: var(--font-mono);
  font-size: 9.5px;
  letter-spacing: 0.16em;
  color: rgba(160, 185, 210, 0.7);
}

.device__power {
  margin-left: auto;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--faint);
}

.visual--online .device__power {
  background: var(--accent);
  box-shadow: 0 0 8px var(--accent);
}

.device__screen {
  position: relative;
  display: flex;
  align-items: flex-end;
  gap: 8px;
  height: 58px;
  margin-top: 11px;
  padding: 8px 9px;
  border-radius: 7px;
  border: 1px solid rgba(76, 240, 200, 0.16);
  background:
    linear-gradient(180deg, rgba(6, 20, 19, 0.94), rgba(3, 11, 12, 0.98));
  box-shadow: inset 0 0 22px rgba(76, 240, 200, 0.07);
  overflow: hidden;
}

.device__screen::after {
  content: '';
  position: absolute;
  inset: 0;
  background: repeating-linear-gradient(180deg, transparent 0 2px, rgba(0, 0, 0, 0.17) 2px 3px);
  pointer-events: none;
}

.screen-wave {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 26px;
}

.screen-wave i {
  width: 2px;
  border-radius: 2px;
  background: var(--accent);
  opacity: 0.28;
  transition: opacity 400ms ease;
}

.visual--online .screen-wave i {
  opacity: 0.85;
  animation: wave 2.4s ease-in-out infinite;
}

.visual--online .screen-wave i:nth-child(2n) {
  animation-delay: 0.2s;
}
.visual--online .screen-wave i:nth-child(3n) {
  animation-delay: 0.45s;
}

@keyframes wave {
  0%,
  100% {
    transform: scaleY(0.62);
  }
  50% {
    transform: scaleY(1);
  }
}

.screen-label {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: 6.5px;
  letter-spacing: 0.12em;
  color: rgba(76, 240, 200, 0.56);
}

.device__bottom {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 10px;
}

.device__port {
  width: 15px;
  height: 4px;
  border-radius: 2px;
  background: rgba(0, 0, 0, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.device__vent {
  width: 9px;
  height: 2px;
  border-radius: 2px;
  background: rgba(0, 0, 0, 0.38);
}

.device__led {
  margin-left: auto;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--faint);
}

.visual--online .device__led {
  background: var(--warning);
  box-shadow: 0 0 7px var(--warning);
}

.tag {
  position: absolute;
  z-index: 3;
  display: inline-flex;
  align-items: center;
  height: 21px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--border);
  background: rgba(8, 14, 23, 0.76);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.1em;
  color: var(--muted);
  backdrop-filter: blur(8px);
  transition:
    color 500ms ease,
    border-color 500ms ease;
}

.visual--online .tag {
  color: var(--accent);
  border-color: var(--border-accent);
}

.tag--usb {
  left: 8%;
  bottom: 30%;
}
.tag--ble {
  right: 7%;
  top: 32%;
}
.tag--rf {
  right: 13%;
  bottom: 22%;
}

.visual__status {
  position: absolute;
  bottom: 6px;
  left: 50%;
  transform: translateX(-50%);
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.24em;
  color: var(--faint);
  transition: color 500ms ease;
}

.visual--online .visual__status {
  color: var(--accent);
}

@media (max-width: 1100px) {
  .visual {
    min-height: 290px;
    transform: scale(0.9);
  }
}

@media (max-width: 860px) {
  .visual {
    min-height: 250px;
    transform: scale(0.8);
    margin: -20px 0;
  }
}
</style>
