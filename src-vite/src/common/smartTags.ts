export interface SmartTagDef {
  id: string;
  // CLIP text prompt should stay in English for stable semantic search.
  prompt: string;
}

// Smart tags use the same settings.imageSearch.thresholdIndex as free-text search
// (Content.vue → getImageSearchFileList without thresholdOverride). Do not hard-code thr here.
// Host text path: abs floor max(0.16, thr*0.85); rel top1*0.85 only on empty fallback;
// thr_cap Top-K: VH 30 / H 40 / M 50 / L 200. Similar-from-file uses a separate image ladder.

export interface SmartTagCategoryDef {
  id: string;
  items: SmartTagDef[];
}

/**
 * Short CLIP-style English prompts (closer to "a photo of a {label}" training).
 * Long multi-clause descriptions dilute B/32 text→image scores.
 * Product set: 6 coarse subject buckets (people / pets / landscape / architecture / plants / birds).
 *
 * People/pets (owner logs 2026-07-24, ~103 embeds):
 * - bare "human" flooded personal libraries (99/103 ≥0.204 @ High).
 * - "pet dog or cat" had sharp top scores but only top~2 of VH-5 were true pets.
 * Prefer face/portrait/group for people; list common pet species for pets.
 */
export const SMART_TAG_CATEGORIES: SmartTagCategoryDef[] = [
  {
    id: 'people',
    items: [
      {
        id: 'people',
        // Short plural "people" matches groups/queues/backs better than "portrait"
        // (owner: mall queue full of people missed by portrait prompt).
        // Avoid multi-"or" clauses (dilute max). Face naming still uses face index.
        prompt: 'a photo of people',
      },
    ],
  },
  {
    id: 'pets',
    items: [
      {
        id: 'pets',
        // Common household pets (concrete species beat abstract "pet").
        prompt: 'a photo of a dog or cat or rabbit or hamster or bird pet',
      },
    ],
  },
  {
    id: 'landscape',
    items: [{ id: 'landscape', prompt: 'a photo of a natural landscape' }],
  },
  {
    id: 'architecture',
    items: [{ id: 'architecture', prompt: 'a photo of a building' }],
  },
  {
    id: 'plants',
    items: [{ id: 'plants', prompt: 'a photo of a plant' }],
  },
  {
    id: 'birds',
    items: [{ id: 'birds', prompt: 'a photo of a bird' }],
  },
];

export function getSmartTagById(id: string | null | undefined): SmartTagDef | null {
  if (!id) return null;
  for (const category of SMART_TAG_CATEGORIES) {
    const found = category.items.find((item) => item.id === id);
    if (found) return found;
  }
  return null;
}
