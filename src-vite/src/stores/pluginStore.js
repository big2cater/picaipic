import { defineStore } from 'pinia';
import { getAiPluginStatus, listAiPlugins } from '@/common/api';

export const usePluginStore = defineStore('pluginStore', {
  state: () => ({
    plugins: [],
    statuses: {},
    loaded: false,
    loading: false,
    error: '',
  }),

  getters: {
    validPlugins: (state) => state.plugins.filter((plugin) =>
      plugin?.id && plugin?.validation?.valid && plugin?.platformSupported !== false
    ),
    runningPluginIds: (state) => new Set(
      Object.entries(state.statuses || {})
        .filter(([, status]) => Boolean(status?.reachable && status?.managed))
        .map(([pluginId]) => pluginId)
    ),
  },

  actions: {
    async loadPlugins(force = false) {
      if (this.loading) return this.plugins;
      if (this.loaded && !force) return this.plugins;

      this.loading = true;
      this.error = '';
      try {
        const plugins = await listAiPlugins();
        this.plugins = Array.isArray(plugins) ? plugins : [];

        const statuses = {};
        await Promise.all(
          this.plugins
            .filter((plugin) => plugin?.id && plugin?.validation?.valid)
            .map(async (plugin) => {
              const status = await getAiPluginStatus(plugin.id);
              if (status) {
                statuses[plugin.id] = status;
              }
            })
        );
        this.statuses = statuses;
        this.loaded = true;
      } catch (error) {
        this.error = error?.message || String(error);
        this.plugins = [];
        this.statuses = {};
      } finally {
        this.loading = false;
      }
      return this.plugins;
    },

    setStatus(pluginId, status) {
      if (!pluginId) return;
      this.statuses = {
        ...this.statuses,
        [pluginId]: status,
      };
    },

    removePlugin(pluginId) {
      if (!pluginId) return;
      this.plugins = this.plugins.filter((plugin) => plugin?.id !== pluginId);
      const nextStatuses = { ...this.statuses };
      delete nextStatuses[pluginId];
      this.statuses = nextStatuses;
    },

    getMenuItems(context, placement) {
      return this.validPlugins
        .filter((plugin) => this.runningPluginIds.has(plugin.id))
        .flatMap((plugin) => {
          const menus = plugin?.contributes?.menus || [];
          return menus
            .filter((menu) =>
              Array.isArray(menu.contexts) &&
              Array.isArray(menu.placements) &&
              menu.contexts.includes(context) &&
              menu.placements.includes(placement)
            )
            .map((menu) => ({
              ...menu,
              pluginId: plugin.id,
              pluginName: plugin.name,
            }));
        })
        .sort((a, b) => Number(a.order ?? 1000) - Number(b.order ?? 1000));
    },
  },
});
