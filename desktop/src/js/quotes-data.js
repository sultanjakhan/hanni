// ── quotes-data.js — Random quote logic (no persistence) ──
import { QUOTES } from './quotes-collection.js';

let currentIndex = -1;

function pickRandom() {
  if (QUOTES.length <= 1) { currentIndex = 0; return; }
  let next;
  do { next = Math.floor(Math.random() * QUOTES.length); } while (next === currentIndex);
  currentIndex = next;
}

export function randomQuote() {
  pickRandom();
  return QUOTES[currentIndex];
}

export function getQuote() {
  if (currentIndex < 0) pickRandom();
  return QUOTES[currentIndex];
}

export function getTotal() { return QUOTES.length; }
