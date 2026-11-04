// @ts-check

/**
 * querySelector on `r`, or on the document when `r` is omitted.
 * @param {string} s
 * @param {ParentNode} [r]
 * @returns {HTMLElement}
 */
export const $ = (s, r) => (r || document).querySelector(s);
