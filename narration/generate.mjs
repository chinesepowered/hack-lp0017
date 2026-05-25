#!/usr/bin/env node
// ElevenLabs narration generator for Whistleblower (LP-0017).
// Zero dependencies — uses Node's built-in fetch. Run with:
//   node --env-file=.env narration/generate.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const API_KEY = process.env.ELEVENLABS_API_KEY;
if (!API_KEY) {
  console.error('ERROR: ELEVENLABS_API_KEY is not set.');
  console.error('Create a .env file with your key, then run:');
  console.error('  node --env-file=.env narration/generate.mjs');
  process.exit(1);
}

const VOICE_ID = process.env.ELEVENLABS_VOICE_ID || 'EXAVITQu4vr4xnSDxMaL';
const MODEL_ID = process.env.ELEVENLABS_MODEL_ID || 'eleven_multilingual_v2';
const SCRIPT_PATH = process.env.SCRIPT_PATH
  ? path.resolve(process.env.SCRIPT_PATH)
  : path.join(__dirname, 'script.md');
const OUT_PATH = process.env.OUT_PATH
  ? path.resolve(process.env.OUT_PATH)
  : path.join(__dirname, 'narration.mp3');

if (!fs.existsSync(SCRIPT_PATH)) {
  console.error(`ERROR: script file not found: ${SCRIPT_PATH}`);
  process.exit(1);
}

const raw = fs.readFileSync(SCRIPT_PATH, 'utf8');
const text = raw
  .replace(/^#.*$/gm, '')
  .replace(/^_.*_\s*$/gm, '')
  .replace(/\*\*(.+?)\*\*/g, '$1')
  .replace(/\*(.+?)\*/g, '$1')
  .replace(/`([^`]+)`/g, '$1')
  .replace(/\[(.+?)\]\(.+?\)/g, '$1')
  .replace(/\n{2,}/g, '\n\n')
  .trim();

const wordCount = text.split(/\s+/).filter(Boolean).length;
const estSeconds = Math.round((wordCount / 150) * 60);
const mins = Math.floor(estSeconds / 60);
const secs = estSeconds % 60;

console.log(`Script:   ${wordCount} words (~${mins}:${String(secs).padStart(2, '0')} at 150 wpm)`);
console.log(`Voice:    ${VOICE_ID}`);
console.log(`Model:    ${MODEL_ID}`);
console.log(`Output:   ${OUT_PATH}`);
console.log('\nCalling ElevenLabs...');

const res = await fetch(
  `https://api.elevenlabs.io/v1/text-to-speech/${VOICE_ID}`,
  {
    method: 'POST',
    headers: {
      'xi-api-key': API_KEY,
      'Content-Type': 'application/json',
      Accept: 'audio/mpeg',
    },
    body: JSON.stringify({
      text,
      model_id: MODEL_ID,
      voice_settings: {
        stability: 0.5,
        similarity_boost: 0.75,
        style: 0.35,
        use_speaker_boost: true,
      },
    }),
  },
);

if (!res.ok) {
  const err = await res.text();
  console.error(`\nElevenLabs API error (${res.status} ${res.statusText}):`);
  console.error(err);
  process.exit(1);
}

const audio = Buffer.from(await res.arrayBuffer());
fs.writeFileSync(OUT_PATH, audio);

const kb = (audio.length / 1024).toFixed(1);
console.log(`\n[OK] Wrote ${kb} KB to ${OUT_PATH}`);
console.log('Drop it into your video editor as the voiceover track.');
