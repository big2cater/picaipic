/** Library panel quick-entry ids (sidebar album tab). */
export const LIB_ITEM = {
  ALL: 'all-files',
  FAV: 'favorites',
  TODAY: 'on-this-day',
} as const;

export type LibItem = (typeof LIB_ITEM)[keyof typeof LIB_ITEM];

/**
 * Absolute left-sidebar indices from Home.vue `buttons` array order.
 * Keep in sync with that array — do not use raw numbers for routing.
 * (Smart Albums was inserted at index 1 and shifted later entries.)
 */
export const SIDEBAR = {
  LIBRARY: 0,
  SMART: 1,
  FAVORITE: 2,
  SEARCH: 3,
  CALENDAR: 4,
  TAG: 5,
  PERSON: 6,
  LOCATION: 7,
  CAMERA: 8,
  MAP: 9,
} as const;

export type SidebarIndex = (typeof SIDEBAR)[keyof typeof SIDEBAR];

/** Grid date-grouping modes (settings + view overrides). */
export const DATE_GROUP = {
  NONE: 0,
  DAY: 1,
  MONTH: 2,
} as const;

export type DateGroup = (typeof DATE_GROUP)[keyof typeof DATE_GROUP];
