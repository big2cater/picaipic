export type AiPluginHostGpu = {
  name: string;
  vendor: string;
  backendCandidates: string[];
};

export type AiPluginHostEnvironment = {
  os: string;
  arch: string;
  platform: string;
  gpus: AiPluginHostGpu[];
  candidateBackends: string[];
  probeError?: string;
};

export type AiPluginNetworkPermissions = {
  runtime?: boolean;
  setupDownloads?: boolean;
  uploadSelectedFiles?: boolean;
  uploadOutputs?: boolean;
  allowedDomains?: string[];
};

export type AiPluginPermissions = {
  readSelectedFiles?: boolean;
  writeOutputDir?: boolean;
  writeSourceFiles?: boolean;
  launchChildProcesses?: boolean;
  network?: AiPluginNetworkPermissions;
};

export type AiPluginPermissionGrant = {
  pluginId?: string;
  runtimeNetwork?: boolean;
  setupDownloads?: boolean;
  uploadSelectedFiles?: boolean;
  uploadOutputs?: boolean;
  allowedDomains?: string[];
  updatedAt?: string;
};

export type AiPluginRuntimeBinding = {
  scope?: string;
  kind?: string;
  id?: string;
  label?: string;
  python?: string;
  root?: string;
  requirements?: string;
  notes?: string;
};

export type AiPluginProfileState = {
  pluginId?: string;
  profileId?: string;
  backend?: string;
  capability?: string;
  status?: string;
  verified?: boolean;
  updatedAt?: string;
  runtimeBinding?: AiPluginRuntimeBinding | null;
};

export type AiPluginInstallProfile = {
  id: string;
  backend: string;
  label?: string;
  supportLevel?: string;
  derivedFrom?: string;
  envDir?: string;
  requirements?: string;
  runtimeBinding?: AiPluginRuntimeBinding;
  runtimeBindings?: AiPluginRuntimeBinding[];
  notes?: string;
  resolvedRuntimeDir?: string | null;
  state?: AiPluginProfileState | null;
};

const BACKEND_LABELS: Record<string, string> = {
  auto: 'Auto',
  cuda: 'CUDA',
  rocm: 'ROCm',
  directml: 'DirectML',
  openvino: 'OpenVINO',
  mps: 'MPS',
  cpu: 'CPU',
};

const BACKEND_PRIORITY = ['cuda', 'rocm', 'directml', 'openvino', 'mps', 'cpu'];
const PROFILE_SUPPORT_RANK: Record<string, number> = {
  official: 0,
  preferred: 0,
  derived: 1,
  compatible: 2,
  fallback: 3,
};

export function backendLabel(backend: string) {
  return BACKEND_LABELS[String(backend || '').toLowerCase()] || String(backend || '');
}

export function recommendedBackend(
  hostEnvironment?: AiPluginHostEnvironment | null,
  allowedBackends: string[] = [],
) {
  const detected = new Set((hostEnvironment?.candidateBackends || []).map((item) => String(item).toLowerCase()));
  const allowed = new Set(
    allowedBackends
      .map((item) => String(item).toLowerCase())
      .filter((item) => item && item !== 'auto'),
  );
  const hasAllowed = allowed.size > 0;

  for (const backend of BACKEND_PRIORITY) {
    if (detected.has(backend) && (!hasAllowed || allowed.has(backend))) {
      return backend;
    }
  }

  if (!hasAllowed || allowed.has('cpu')) return 'cpu';
  return Array.from(allowed)[0] || 'cpu';
}

export function deviceOptionLabel(
  option: string,
  hostEnvironment?: AiPluginHostEnvironment | null,
  allowedBackends: string[] = [],
) {
  const value = String(option || '').toLowerCase();
  const recommended = recommendedBackend(hostEnvironment, allowedBackends);
  const detected = new Set((hostEnvironment?.candidateBackends || []).map((item) => String(item).toLowerCase()));

  if (value === 'auto') {
    return `Auto (recommended: ${backendLabel(recommended)})`;
  }

  if (value === recommended) {
    return `${backendLabel(value)} (recommended)`;
  }

  if (value !== 'cpu' && hostEnvironment && !detected.has(value)) {
    return `${backendLabel(value)} (not detected)`;
  }

  return backendLabel(value);
}

export function devicePreferenceHint(
  hostEnvironment?: AiPluginHostEnvironment | null,
  allowedBackends: string[] = [],
) {
  const recommended = recommendedBackend(hostEnvironment, allowedBackends);
  return `Only affects this run. It does not install dependencies. Recommended: ${backendLabel(recommended)}.`;
}

export function pluginPermissions(plugin: any): AiPluginPermissions {
  return plugin?.permissions || { network: {} };
}

export function pluginPermissionGrant(plugin: any): AiPluginPermissionGrant | null {
  return plugin?.permissionGrant || null;
}

export function pluginAllowedDomains(plugin: any): string[] {
  return Array.isArray(pluginPermissions(plugin)?.network?.allowedDomains)
    ? pluginPermissions(plugin).network!.allowedDomains!.filter(Boolean)
    : [];
}

export function buildPluginPermissionGrantRequest(
  plugin: any,
  requested: {
    runtimeNetwork?: boolean;
    setupDownloads?: boolean;
    uploadSelectedFiles?: boolean;
    uploadOutputs?: boolean;
  },
) {
  const grant = pluginPermissionGrant(plugin);
  return {
    runtimeNetwork: Boolean(requested.runtimeNetwork || grant?.runtimeNetwork),
    setupDownloads: Boolean(requested.setupDownloads || grant?.setupDownloads),
    uploadSelectedFiles: Boolean(requested.uploadSelectedFiles || grant?.uploadSelectedFiles),
    uploadOutputs: Boolean(requested.uploadOutputs || grant?.uploadOutputs),
    allowedDomains: pluginAllowedDomains(plugin),
  };
}

export function missingPluginPermissionFlags(
  plugin: any,
  requested: {
    runtimeNetwork?: boolean;
    setupDownloads?: boolean;
    uploadSelectedFiles?: boolean;
    uploadOutputs?: boolean;
  },
) {
  const grant = pluginPermissionGrant(plugin);
  const missing: string[] = [];
  if (requested.runtimeNetwork && !grant?.runtimeNetwork) missing.push('runtime network');
  if (requested.setupDownloads && !grant?.setupDownloads) missing.push('setup downloads');
  if (requested.uploadSelectedFiles && !grant?.uploadSelectedFiles) missing.push('upload selected files');
  if (requested.uploadOutputs && !grant?.uploadOutputs) missing.push('upload outputs');
  return missing;
}

function profileRuntimeBindingOptions(
  profile: AiPluginInstallProfile,
  hostEnvironment?: AiPluginHostEnvironment | null,
) {
  const bindings: AiPluginRuntimeBinding[] = [];
  const seen = new Set<string>();
  const pushBinding = (binding?: AiPluginRuntimeBinding) => {
    if (!binding?.scope) return;
    const key = binding.id || `${binding.scope}:${binding.python || binding.root || bindings.length}`;
    if (seen.has(key)) return;
    seen.add(key);
    bindings.push(binding);
  };

  pushBinding(profile.runtimeBinding);
  for (const binding of profile.runtimeBindings || []) {
    pushBinding(binding);
  }

  const declaredRequirements = profile.runtimeBinding?.requirements || profile.requirements;
  for (const runtime of hostEnvironment?.pythonRuntimes || []) {
    if (!runtime?.available) continue;
    pushBinding({
      scope: runtime.scope || 'external',
      kind: 'python',
      id: `discovered:${runtime.id}`,
      label: runtime.label,
      python: runtime.python,
      root: runtime.root,
      requirements: declaredRequirements,
      notes: `${runtime.source}${runtime.version ? ` - ${runtime.version}` : ''}`,
    });
  }

  return bindings;
}

function profileStatusLevel(profile?: AiPluginInstallProfile | null) {
  const state = profile?.state;
  if (!state) return 0;
  if (state.verified || String(state.status || '').toLowerCase() === 'verified') return 3;
  if (String(state.status || '').toLowerCase() === 'needsverify') return 2;
  if (String(state.status || '').toLowerCase() === 'installing') return 1;
  return 0;
}

export function pluginRecommendedProfile(
  plugin: any,
  hostEnvironment?: AiPluginHostEnvironment | null,
): AiPluginInstallProfile | null {
  const profiles = Array.isArray(plugin?.installProfiles) ? plugin.installProfiles : [];
  if (profiles.length === 0) return null;

  const detected = new Set((hostEnvironment?.candidateBackends || []).map((backend) => String(backend).toLowerCase()));
  const matches = profiles.filter((profile) => detected.has(String(profile.backend || '').toLowerCase()));
  const candidates = matches.length > 0 ? matches : profiles.filter((profile) => String(profile.backend || '').toLowerCase() === 'cpu');
  const sorted = [...(candidates.length > 0 ? candidates : profiles)].sort((a, b) => {
    const rankA = PROFILE_SUPPORT_RANK[String(a.supportLevel || '').toLowerCase()] ?? 10;
    const rankB = PROFILE_SUPPORT_RANK[String(b.supportLevel || '').toLowerCase()] ?? 10;
    return rankA - rankB;
  });
  return sorted[0] || null;
}

export function pluginStartProfile(
  plugin: any,
  hostEnvironment?: AiPluginHostEnvironment | null,
): AiPluginInstallProfile | null {
  const profiles = Array.isArray(plugin?.installProfiles) ? plugin.installProfiles : [];
  if (profiles.length === 0) return null;

  const verified = profiles.find((profile) => profileStatusLevel(profile) >= 3);
  const needsVerification = profiles.find((profile) => profileStatusLevel(profile) > 0);
  return verified || pluginRecommendedProfile(plugin, hostEnvironment) || needsVerification || profiles[0] || null;
}

export function pluginStartRuntimeBinding(
  plugin: any,
  hostEnvironment?: AiPluginHostEnvironment | null,
): AiPluginRuntimeBinding | undefined {
  const profile = pluginStartProfile(plugin, hostEnvironment);
  if (!profile) return undefined;
  const persisted = profile.state?.runtimeBinding || undefined;
  const options = profileRuntimeBindingOptions(profile, hostEnvironment);
  if (persisted?.id) {
    return options.find((binding) => binding.id === persisted.id) || persisted;
  }
  return persisted || options[0];
}

export function pluginStartRequest(
  plugin: any,
  hostEnvironment?: AiPluginHostEnvironment | null,
) {
  const profile = pluginStartProfile(plugin, hostEnvironment);
  if (!plugin?.id || !profile) return undefined;
  const runtimeBinding = pluginStartRuntimeBinding(plugin, hostEnvironment);
  return {
    profileId: profile.id,
    backend: profile.backend,
    capability: plugin?.smokeTest?.capability || plugin?.capabilities?.[0]?.id || '',
    runtimeBindingId: runtimeBinding?.id,
    runtimeBinding,
  };
}
