// ── quotes-data.js — Quote navigation with history ──
import { QUOTES } from './quotes-collection.js';

const history = [];
let historyPos = -1;

function pickRandom() {
  if (QUOTES.length <= 1) return 0;
  const cur = history.length ? history[historyPos] : -1;
  let next;
  do { next = Math.floor(Math.random() * QUOTES.length); } while (next === cur);
  return next;
}

export function nextQuote() {
  if (historyPos < history.length - 1) {
    historyPos++;
  } else {
    history.push(pickRandom());
    historyPos = history.length - 1;
  }
  return QUOTES[history[historyPos]];
}

export function prevQuote() {
  if (historyPos > 0) historyPos--;
  return QUOTES[history[historyPos]];
}

export function getQuote() {
  if (!history.length) return nextQuote();
  return QUOTES[history[historyPos]];
}

export function canGoBack() { return historyPos > 0; }
