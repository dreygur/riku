// @ts-check
// Inline SVG icons as markup strings. Every value is a constant, so they are
// safe to splice into innerHTML.

/**
 * @param {string} p path markup
 * @param {number} [w] size in px, 15 when omitted
 * @param {number} [sw] stroke width, 1.9 when omitted
 */
const sv = (p, w, sw) => `<svg width="${w || 15}" height="${w || 15}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${sw || 1.9}" stroke-linecap="square" stroke-linejoin="miter" aria-hidden="true">${p}</svg>`;

export const I = {
  term: sv('<path d="M4 17l6-5-6-5"/><path d="M12 19h8"/>'),
  restart: sv('<path d="M9 4h11v16H4v-9"/><path d="M1 8l3 3 3-3"/>'),
  restartSm: sv('<path d="M9 4h11v16H4v-9"/><path d="M1 8l3 3 3-3"/>', 11, 2.4),
  deploy: sv('<path d="M12 15V4"/><path d="M7 9l5-5 5 5"/><path d="M4 15v6h16v-6"/>', 15, 2),
  rollback: sv('<path d="M9 4h11v16H4v-9"/><path d="M1 8l3 3 3-3"/><path d="M12 8v5h3"/>'),
  stop: sv('<rect x="6" y="6" width="12" height="12"/>', 14, 2),
  minus: sv('<path d="M5 12h14"/>', 14, 2),
  plus: sv('<path d="M5 12h14M12 5v14"/>', 14, 2),
  chev: sv('<path d="M6 9l6 6 6-6"/>', 14, 2),
  lock: sv('<rect x="4" y="11" width="16" height="10"/><path d="M8 11V4h8v7"/>'),
  lockLg: sv('<rect x="4" y="11" width="16" height="10"/><path d="M8 11V4h8v7"/>', 18),
  unlock: sv('<rect x="4" y="11" width="16" height="10"/><path d="M8 11V4h8v2"/>'),
  https: sv('<rect x="5" y="11" width="14" height="10"/><path d="M8 11V5h8v6"/>', 13, 2),
  ext: sv('<path d="M7 17L17 7"/><path d="M8 7h9v9"/>', 12, 2),
  server: sv('<rect x="3" y="4" width="18" height="7"/><rect x="3" y="13" width="18" height="7"/>', 13),
  commit: sv('<rect x="8.5" y="8.5" width="7" height="7"/><path d="M3 12h5.5M15.5 12H21"/>', 13),
  alert: sv('<path d="M12 3.5l9.5 16.5h-19z"/><path d="M12 10v4.5"/><path d="M12 17.5v.01"/>', 16, 2),
  cross: sv('<path d="M6 6l12 12M18 6L6 18"/>', 12, 3),
  x: sv('<path d="M6 6l12 12M18 6L6 18"/>', 12, 2.4),
  arrow: sv('<path d="M5 12h14"/><path d="M13 6l6 6-6 6"/>', 14, 2),
  check: sv('<path d="M5 12l5 5 9-10"/>', 10, 4),
  bang: sv('<path d="M12 5v8"/><path d="M12 18v1"/>', 10, 4),
  info: sv('<path d="M12 11v8"/><path d="M12 5v1"/>', 10, 4),
  spin: '<svg class="spin" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="square" aria-hidden="true"><path d="M4 9V4h16v16H4"/></svg>',
};
