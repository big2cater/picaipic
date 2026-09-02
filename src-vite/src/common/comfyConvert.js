/**
 * Converts a ComfyUI **UI-format** workflow (what the graph editor saves: `nodes` +
 * `links` + positional `widgets_values`) into the **API format** that `/prompt` accepts.
 *
 * The UI format stores widget values in a positional array, so mapping them onto named
 * inputs is only possible with the node definitions from `/object_info`. Callers therefore
 * need a reachable ComfyUI server; without it the conversion would have to guess.
 *
 * Cases that cannot be mapped are reported as thrown errors carrying an i18n key rather
 * than silently producing a workflow that ComfyUI would reject:
 *   - bypassed (`mode: 4`) nodes, whose inputs would have to be rewired to their consumers
 *   - `PrimitiveNode`, whose value is produced by the frontend, not the node definition
 *   - node types absent from `/object_info` (custom node pack not installed on the server)
 */

/// Purely decorative canvas elements; they never take part in execution.
const DECORATION_TYPES = new Set(['Note', 'MarkdownNote']);

/// Types the frontend synthesises, so their value cannot be recovered from `/object_info`.
const UNSUPPORTED_TYPES = new Set(['PrimitiveNode']);

/// True when `obj` is the UI format rather than the API format.
///
/// Deliberately strict. The UI format is a canvas description whose `nodes` entries carry
/// positional `widgets_values`; the API format has no such field. Testing merely for "has a
/// nodes array" misclassifies an API-format export that still keeps a `nodes` key (several
/// ComfyUI versions do), and running the conversion over already-API JSON silently shifts
/// every widget value — which shows up as nonsense like `cfg: 959948902156062.0`.
export function isUiWorkflow(obj) {
  if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return false;
  const nodes = obj.nodes;
  if (!Array.isArray(nodes) || nodes.length === 0) return false;
  return nodes.some((node) => node && typeof node === 'object' && 'widgets_values' in node);
}

/**
 * Normalizes a validated API-format document into the clean keyed form `/prompt` needs.
 *
 * The plain keyed format (`{ nodeId: { class_type, inputs } }`) is passed through as-is.
 * Some ComfyUI exports keep a `nodes` array on top of the API format — the same array the UI
 * format uses, but without `widgets_values` — whose entries carry `class_type`/`inputs` and
 * are the actual workflow; those are rebuilt into the keyed form. Stray canvas keys
 * (`nodes`/`links`) never reach the server, where an unexpected top-level key would get
 * mistaken for a node id and rejected.
 *
 * Returns null when neither shape contains any valid node.
 */
export function normalizeApiWorkflow(obj) {
  if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return null;

  const api = {};
  const isApiNode = (node) =>
    node &&
    typeof node === 'object' &&
    !Array.isArray(node) &&
    typeof node.class_type === 'string' &&
    node.inputs &&
    typeof node.inputs === 'object' &&
    !Array.isArray(node.inputs);

  // `nodes` entries are UI-shaped only when they carry widgets_values; without it they are
  // the API export's own node list (class_type is a UI node never has), so take them.
  if (Array.isArray(obj.nodes)) {
    for (const node of obj.nodes) {
      if (!isApiNode(node) || node.id === undefined) continue;
      api[String(node.id)] = { class_type: node.class_type, inputs: node.inputs };
    }
  }

  for (const [key, node] of Object.entries(obj)) {
    if (!isApiNode(node)) continue;
    api[key] = { class_type: node.class_type, inputs: node.inputs };
  }

  return Object.keys(api).length > 0 ? api : null;
}

/// link_id -> [origin_node_id, origin_slot]
function buildLinkIndex(ui) {
  const map = new Map();
  for (const link of Array.isArray(ui?.links) ? ui.links : []) {
    // Layout: [link_id, origin_node, origin_slot, target_node, target_slot, type]
    if (!Array.isArray(link) || link.length < 3) continue;
    map.set(link[0], [String(link[1]), link[2]]);
  }
  return map;
}

/**
 * @param {object} ui          parsed UI-format workflow
 * @param {object} objectInfo  distilled `/object_info` ({ NodeType: { widgets: [...] } })
 * @returns {{ api: object, skipped: Array<{ type: string, reason: string }> }}
 * @throws {Error} message is an i18n key, optionally suffixed with `:` + detail
 */
export function convertUiToApi(ui, objectInfo) {
  if (!objectInfo || typeof objectInfo !== 'object') {
    throw new Error('ui_no_object_info');
  }

  const nodes = Array.isArray(ui?.nodes) ? ui.nodes : [];
  if (nodes.length === 0) {
    throw new Error('ui_no_nodes');
  }

  const links = buildLinkIndex(ui);
  const byId = new Map();
  for (const node of nodes) {
    if (node && node.id !== undefined) byId.set(String(node.id), node);
  }

  // A Reroute is pass-through, so walk upstream until a real producer is found.
  function followReroute(nodeId, slot) {
    const seen = new Set();
    let currentId = String(nodeId);
    let currentSlot = slot;
    for (;;) {
      const node = byId.get(currentId);
      if (!node || node.type !== 'Reroute' || seen.has(currentId)) break;
      seen.add(currentId);
      const upstream = Array.isArray(node.inputs) ? node.inputs[0] : null;
      const target = links.get(upstream?.link);
      if (!target) return null;
      [currentId, currentSlot] = target;
      currentId = String(currentId);
    }
    return [currentId, currentSlot];
  }

  const api = {};
  const skipped = [];
  const missing = new Set();

  for (const node of nodes) {
    if (!node || !node.type) continue;

    if (DECORATION_TYPES.has(node.type) || node.type === 'Reroute') continue;

    if (node.mode === 2) {
      skipped.push({ type: node.type, reason: 'muted' });
      continue;
    }
    if (node.mode === 4) {
      throw new Error('ui_bypass_unsupported');
    }
    if (UNSUPPORTED_TYPES.has(node.type)) {
      throw new Error('ui_primitive_unsupported');
    }

    const layout = objectInfo[node.type];
    if (!layout || !Array.isArray(layout.widgets)) {
      missing.add(node.type);
      continue;
    }

    const values = Array.isArray(node.widgets_values) ? node.widgets_values : [];
    const inputs = {};
    let cursor = 0;
    for (const widget of layout.widgets) {
      if (cursor >= values.length) break;
      inputs[widget.name] = values[cursor++];
      // A randomizable widget is trailed by a "control_after_generate" value that has
      // no counterpart in the API format, so it must be stepped over.
      if (widget.control) cursor += 1;
    }

    for (const slot of Array.isArray(node.inputs) ? node.inputs : []) {
      const target = links.get(slot.link);
      if (!target) continue;
      const resolved = followReroute(target[0], target[1]);
      if (resolved) inputs[slot.name] = resolved;
    }

    api[String(node.id)] = { class_type: node.type, inputs };
  }

  if (missing.size > 0) {
    throw new Error(`ui_missing_nodes:${[...missing].join(', ')}`);
  }
  if (Object.keys(api).length === 0) {
    throw new Error('ui_no_nodes');
  }

  return { api, skipped };
}
