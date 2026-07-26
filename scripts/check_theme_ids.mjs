import assert from 'node:assert/strict';

// Keep in sync with src-vite/src/common/utils.ts THEME_ID / clamp / predicates
const THEME_ID = {
  DEFAULT: 0,
  RETRO: 1,
  CMYK: 2,
  BLACK_HOLE: 3,
  CYBERPUNK: 4,
};

function clampThemeId(themeId) {
  const n = Number(themeId);
  if (!Number.isFinite(n) || n < 0 || n > THEME_ID.CYBERPUNK) return THEME_ID.DEFAULT;
  return Math.floor(n);
}

function isBlackHoleTheme(appearance, lightTheme, darkTheme) {
  const id = appearance === 0 ? lightTheme : darkTheme;
  return clampThemeId(id) === THEME_ID.BLACK_HOLE;
}

function isCyberpunkTheme(appearance, lightTheme, darkTheme) {
  const id = appearance === 0 ? lightTheme : darkTheme;
  return clampThemeId(id) === THEME_ID.CYBERPUNK;
}

function forcesDarkDataTheme(themeId) {
  const id = clampThemeId(themeId);
  return id === THEME_ID.BLACK_HOLE || id === THEME_ID.CYBERPUNK;
}

assert.equal(clampThemeId(4), 4);
assert.equal(clampThemeId(5), 0);
assert.equal(clampThemeId(-1), 0);
assert.equal(isCyberpunkTheme(1, 0, 4), true);
assert.equal(isCyberpunkTheme(1, 0, 3), false);
assert.equal(isBlackHoleTheme(1, 0, 3), true);
assert.equal(forcesDarkDataTheme(3), true);
assert.equal(forcesDarkDataTheme(4), true);
assert.equal(forcesDarkDataTheme(1), false);
console.log('check_theme_ids: ok');
