export interface SmartTagDef {
  id: string;
  // CLIP text prompt should stay in English for stable semantic search.
  prompt: string;
}

// Smart-tag cosine-similarity floor (higher = stricter). Backend must honor
// `ImageSearchParams.threshold` for text queries — do not force 0.25 in Rust.
// Slightly above the settings Medium (0.28) keeps concept search usable while
// cutting off-topic noise vs the old forced 0.25.
export const SMART_TAG_SEARCH_THRESHOLD = 0.28;

export interface SmartTagCategoryDef {
  id: string;
  items: SmartTagDef[];
}

/**
 * Concept-style English prompts for local CLIP image search.
 * Prefer concrete visual cues over abstract category names.
 */
export const SMART_TAG_CATEGORIES: SmartTagCategoryDef[] = [
  {
    id: 'family',
    items: [
      {
        id: 'family',
        prompt:
          'a multi-generation family group photo with adults and children together, people posing for a family portrait or holiday gathering',
      },
    ],
  },
  {
    id: 'kids',
    items: [
      {
        id: 'kids',
        prompt:
          'a photo of a young child or toddler or baby, kids playing outdoors or at home, childhood portrait',
      },
    ],
  },
  {
    id: 'pets',
    items: [
      {
        id: 'pets',
        prompt:
          'a photo of a pet dog or cat as the main subject, domestic animal portrait, furry companion',
      },
    ],
  },
  {
    id: 'portraits',
    items: [
      {
        id: 'portraits',
        prompt:
          'a close portrait of one person looking at the camera, face filling much of the frame, head-and-shoulders photo',
      },
    ],
  },
  {
    id: 'food',
    items: [
      {
        id: 'food',
        prompt:
          'food photography of a plated meal or dish on a table, restaurant plate or home cooking, edible food as main subject',
      },
    ],
  },
  {
    id: 'sports',
    items: [
      {
        id: 'sports',
        prompt:
          'people playing sports or outdoor athletic activity, running cycling soccer basketball hiking with motion and action',
      },
    ],
  },
  {
    id: 'landscape',
    items: [
      {
        id: 'landscape',
        prompt:
          'a wide scenic landscape of mountains ocean forest lake or countryside under open sky, nature vista without people as main subject',
      },
    ],
  },
  {
    id: 'night',
    items: [
      {
        id: 'night',
        prompt:
          'a night photo with dark sky city lights neon street lamps or stars, low-light outdoor evening scene',
      },
    ],
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
