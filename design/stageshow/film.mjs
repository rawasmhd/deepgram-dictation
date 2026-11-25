#!/usr/bin/env node
// Film the dictation demos. The demos live in examples/ here and are built
// inside the stageshow skill's own examples folder, where its preview and
// film scripts pick them up.
//
//   node film.mjs sync                      copy examples/* into the skill
//   node film.mjs still <name> [--scale 2] [--out file]
//                                           one PNG of the poster frame
//   node film.mjs gate <name> [--width 390] [--still]
//                                           the skill's film.mjs --gate
//   node film.mjs gif <name> [--fps 20] [--seconds 14] [--out file] [--dump dir]
//                                           record the loop on the virtual
//                                           clock, one frame per step
//
// Needs the skill's preview server: `npm run dev` in
// .agents/skills/stageshow (STAGESHOW_URL overrides the address).
import { cpSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import { PNG } from 'pngjs'
import gifenc from 'gifenc'

const { GIFEncoder, applyPalette, quantize } = gifenc

const here = dirname(fileURLToPath(import.meta.url))
const skill = resolve(here, '../../.agents/skills/stageshow')
const url = process.env.STAGESHOW_URL ?? 'http://127.0.0.1:5173'
const [cmd, name, ...flags] = process.argv.slice(2)
const opt = (key, fallback) => {
  const i = flags.indexOf(`--${key}`)
  return i >= 0 ? flags[i + 1] : fallback
}

function sync() {
  for (const dir of readdirSync(resolve(here, 'examples'))) {
    cpSync(resolve(here, 'examples', dir), resolve(skill, 'examples', dir), { recursive: true })
  }
  console.log('synced examples into the skill')
}

async function open(route, scale) {
  const { chromium } = await import('playwright-core')
  const browser = await chromium.launch({ channel: 'chrome' })
  const page = await browser.newPage({ viewport: { width: 1100, height: 700 }, deviceScaleFactor: scale })
  page.on('pageerror', (e) => console.error(`page error: ${e.message}`))
  await page.goto(`${url}/?${route}`)
  await page.waitForSelector('#example .ss-stage')
  await page.evaluate(() => document.fonts.ready)
  return { browser, page }
}

async function clipOf(page) {
  const box = await (await page.$('#example .ss-stage')).boundingBox()
  return { x: box.x, y: box.y, width: box.width, height: box.height }
}

async function still() {
  const scale = Number(opt('scale', 2))
  const out = resolve(opt('out', resolve(here, 'shots', `${name}.png`)))
  const { browser, page } = await open(`capture=${name}&still=1`, scale)
  await page.waitForTimeout(800)
  mkdirSync(dirname(out), { recursive: true })
  await page.screenshot({ path: out, clip: await clipOf(page) })
  await browser.close()
  console.log(`still -> ${out}`)
}

function gate() {
  const width = opt('width')
  const out = resolve(here, 'shots', `${name}-gate${width ? `-${width}` : ''}.png`)
  mkdirSync(dirname(out), { recursive: true })
  const args = [resolve(skill, 'scripts/film.mjs'), `${url}/?capture=${name}`, '--select', '#example', '--gate', '--out', out]
  if (width) args.push('--width', width)
  if (flags.includes('--still')) args.push('--still')
  const result = spawnSync(process.execPath, args, { stdio: 'inherit', cwd: skill })
  process.exitCode = result.status ?? 1
}

async function gif() {
  const fps = Number(opt('fps', 20))
  const seconds = Number(opt('seconds', 14))
  const out = resolve(opt('out', resolve(here, 'shots', `${name}.gif`)))
  const { browser, page } = await open(`embed=${name}`, 1)
  // The embed route owns a virtual clock: time moves only when stepped
  await page.evaluate(() => {
    window.postMessage({ type: 'stageshow:control', visible: true, playing: false }, location.origin)
  })
  await page.evaluate(() => window.__stageshow.clock.settle())
  const clip = await clipOf(page)
  const width = Math.round(clip.width)
  const height = Math.round(clip.height)
  const per = Math.max(1, Math.round(60 / fps))
  const count = Math.round(seconds * fps)
  // --dump <dir> also writes every frame as a PNG, numbered, for inspection
  const dump = opt('dump')
  if (dump) mkdirSync(resolve(dump), { recursive: true })
  const frames = []
  for (let i = 0; i < count; i++) {
    await page.evaluate(async (k) => {
      await window.__stageshow.clock.step(k)
      await window.__stageshow.clock.flushLayout?.()
    }, per)
    const png = await page.screenshot({ clip, type: 'png' })
    if (dump) writeFileSync(resolve(dump, `${String(i).padStart(3, '0')}.png`), png)
    frames.push(PNG.sync.read(png).data)
  }
  await browser.close()
  // One palette for the whole file, from a sample of frames
  const sample = Buffer.concat(frames.filter((_, i) => i % 8 === 0))
  const palette = quantize(new Uint8Array(sample.buffer, sample.byteOffset, sample.length), 256, { format: 'rgb444' })
  const encoder = GIFEncoder()
  frames.forEach((frame, i) => {
    const index = applyPalette(new Uint8Array(frame.buffer, frame.byteOffset, frame.length), palette, 'rgb444')
    encoder.writeFrame(index, width, height, { palette: i === 0 ? palette : undefined, delay: Math.round(1000 / fps), repeat: 0 })
  })
  encoder.finish()
  const bytes = encoder.bytes()
  mkdirSync(dirname(out), { recursive: true })
  writeFileSync(out, bytes)
  console.log(`gif -> ${out} (${count} frames at ${fps} fps, ${width}x${height}, ${(bytes.length / 1024).toFixed(0)} KB)`)
}

const commands = { sync, still, gate, gif }
if (!commands[cmd] || (cmd !== 'sync' && !name)) {
  console.error('usage: node film.mjs sync | still <name> | gate <name> | gif <name>')
  process.exit(2)
}
await commands[cmd]()
