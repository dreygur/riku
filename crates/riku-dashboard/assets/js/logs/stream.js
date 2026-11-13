// @ts-check
// The live log stream for the app open in the drawer: the EventSource, the
// line buffer and how each line is turned into DOM.
//
// Log lines are arbitrary app output, so they are built with textContent and
// never pass through innerHTML.
import { logsUrl } from "../api.js";
import { $ } from "../dom.js";
import { fmtClock, parseLogEvent } from "../format.js";
import { session } from "../state.js";
import { renderSeg, syncLogUi } from "./drawer.js";

/** How many lines the console and the pause buffer keep. */
export const MAX_LINES = 2000;

/**
 * @typedef {object} LogLine
 * @property {string} tag process tag before the first tab, e.g. "web.1"
 * @property {string} text
 * @property {Date | null} ts the line's own time from the server, null when it has none
 * @property {"err" | "warn" | ""} lvl
 * @property {string} [base] tag up to the first ":" or space
 * @property {string} [hay] lowercased tag and text for the filter
 * @property {HTMLElement} [el]
 */

/** @typedef {"idle" | "connecting" | "live" | "reconnecting" | "closed"} StreamState */

export const L = {
  /** @type {string | null} app whose logs are open */
  app: null,
  /** @type {EventSource | null} */
  es: null,
  /** @type {LogLine[]} */
  lines: [],
  /** @type {Set<string>} */
  tags: new Set(),
  worker: "",
  q: "",
  follow: true,
  paused: false,
  /** @type {LogLine[]} lines received while paused */
  pending: [],
  unseen: 0,
  /** @type {StreamState} */
  stream: "idle",
};

export const con = $("#console");

const TAG_COLORS = ["#72B4F6", "#B79CF2", "#5FD0C4", "#E7A3C6", "#C8D36A", "#F0A877"];

/** @param {string} t */
const tagColor = (t) => {
  let h = 0;
  for (const ch of t) { h = (h * 31 + ch.charCodeAt(0)) >>> 0; }
  return TAG_COLORS[h % TAG_COLORS.length];
};

/** @param {string} t */
const baseTag = (t) => (t || "").split(/[:\s]/)[0];

/**
 * @param {string} tag
 * @param {string} text
 * @returns {LogLine["lvl"]}
 */
const lvlOf = (tag, text) => {
  if (/err/i.test(tag) || /\b(ERROR|ERR|FATAL|CRITICAL|PANIC|Traceback|Exception)\b/.test(text) || /^\s+at\s/.test(text)) {
    return "err";
  }
  if (/\b(WARN|WARNING)\b/.test(text)) { return "warn"; }
  return "";
};

/** @param {LogLine} ln */
export const visible = (ln) => (!L.worker || ln.base === L.worker) && (!L.q || ln.hay.includes(L.q));

/** @param {StreamState} s */
function setStream(s) { L.stream = s; syncLogUi(); }

/** @param {string} cls @param {string} text */
function textSpan(cls, text) {
  const s = document.createElement("span");
  s.className = cls;
  s.textContent = text;
  return s;
}

/** @param {LogLine} ln */
function lineEl(ln) {
  const div = document.createElement("div");
  div.className = "ln" + (ln.lvl ? " " + ln.lvl : "");
  const tag = textSpan("ln-tag", ln.tag);
  tag.style.color = tagColor(ln.base || "-");
  div.append(textSpan("ln-t", ln.ts ? fmtClock(ln.ts) : ""), tag, textSpan("ln-x", ln.text));
  return div;
}

/** @param {LogLine} ln */
export function addLine(ln) {
  ln.base = baseTag(ln.tag);
  if (ln.base && !L.tags.has(ln.base)) {
    L.tags.add(ln.base);
    renderSeg();
  }
  ln.hay = (ln.tag + " " + ln.text).toLowerCase();
  ln.el = lineEl(ln);
  ln.el.hidden = !visible(ln);
  con.appendChild(ln.el);
  L.lines.push(ln);
  while (L.lines.length > MAX_LINES) { L.lines.shift().el.remove(); }
  if (L.follow) {
    con.scrollTop = con.scrollHeight;
  } else if (!ln.el.hidden) {
    L.unseen++;
  }
  syncLogUi();
}

export function clearLines() {
  L.lines.forEach((ln) => ln.el.remove());
  L.lines = []; L.unseen = 0;
  syncLogUi();
}

/**
 * Each connection starts with the server's backlog of every log file, so a
 * reconnect would repeat lines already shown. Drop them first.
 */
export function startStream() {
  setStream("connecting");
  const es = new EventSource(logsUrl(L.app, session.token));
  L.es = es;
  let opened = false;
  es.addEventListener("open", () => {
    if (opened) {
      L.pending = [];
      clearLines();
    }
    opened = true;
    setStream("live");
  });
  es.addEventListener("log", (/** @type {MessageEvent<string>} */ ev) => {
    if (L.stream !== "live") { setStream("live"); }
    const { tag, ts, text } = parseLogEvent(ev.data);
    /** @type {LogLine} */
    const ln = { tag, text, ts, lvl: lvlOf(tag, text) };
    if (L.paused) {
      L.pending.push(ln);
      if (L.pending.length > MAX_LINES) { L.pending.shift(); }
      syncLogUi();
      return;
    }
    addLine(ln);
  });
  es.onerror = () => setStream(es.readyState === EventSource.CLOSED ? "closed" : "reconnecting");
}

/** Closes the stream, if one is open. */
export function stopStream() {
  if (L.es) { L.es.close(); L.es = null; }
}
