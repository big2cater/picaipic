/** Library panel quick-entry ids (sidebar album tab). */
export const LIB_ITEM = {
  ALL: 'all-files',
  FAV: 'favorites',
  TODAY: 'on-this-day',
} as const;

export type LibItem = (typeof LIB_ITEM)[keyof typeof LIB_ITEM];

/** Grid date-grouping modes (settings + view overrides). */
export const DATE_GROUP = {
  NONE: 0,
  DAY: 1,
  MONTH: 2,
} as const;

export type DateGroup = (typeof DATE_GROUP)[keyof typeof DATE_GROUP];
