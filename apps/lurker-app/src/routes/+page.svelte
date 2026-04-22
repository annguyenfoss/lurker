<script lang="ts">
  import { onMount } from 'svelte';
  import { createVolume, listActiveVolumes, mountVolume, probeSystem, unmountVolume } from '$lib/api';
  import type {
    ActiveVolume,
    CreateCipher,
    CreateCommand,
    MountCommand,
    OperationResponse,
    OutputEntry,
    SystemProbe,
    UnmountCommand,
    VolumeType
  } from '$lib/types';

  let systemProbe: SystemProbe | null = null;
  let activeVolumes: ActiveVolume[] = [];
  let lastResult: OperationResponse | null = null;
  let loading = true;
  let busyLabel = '';
  let pageError = '';

  let createTarget = '';
  let createSizeGb = '4';
  let createForce = false;
  let createSourceKind: 'file' | 'block' = 'file';
  let createVolumeType: 'luks' | 'veracrypt' = 'luks';
  let createCipher: CreateCipher = 'aes';
  let createPassphrase = '';

  let mountSource = '';
  let mountPoint = '';
  let mountTag = '';
  let mountVolumeType: VolumeType = 'auto';
  let mountPassphrase = '';

  let unmountTarget = '';
  let unmountTag = '';
  let unmountVolumeType: VolumeType = 'auto';

  onMount(async () => {
    await refresh();
  });

  async function refresh() {
    loading = true;
    pageError = '';
    try {
      const [probe, volumes] = await Promise.all([probeSystem(), listActiveVolumes()]);
      systemProbe = probe;
      activeVolumes = volumes;
    } catch (error) {
      pageError = formatError(error);
    } finally {
      loading = false;
    }
  }

  async function submitCreate() {
    const request: CreateCommand = {
      target: createTarget,
      size_gb: createSourceKind === 'file' ? createSizeGb : null,
      force: createForce,
      source_kind: createSourceKind,
      volume_type: createVolumeType,
      cipher: createCipher,
      passphrase: optionalValue(createPassphrase)
    };

    await runOperation('Creating volume', () => createVolume(request));
  }

  async function submitMount() {
    const request: MountCommand = {
      source: mountSource,
      mountpoint: mountPoint,
      tag: optionalValue(mountTag),
      volume_type: mountVolumeType,
      passphrase: optionalValue(mountPassphrase)
    };

    await runOperation('Mounting volume', () => mountVolume(request));
  }

  async function submitUnmount() {
    const request: UnmountCommand = {
      target: unmountTarget,
      tag: optionalValue(unmountTag),
      volume_type: unmountVolumeType
    };

    await runOperation('Unmounting volume', () => unmountVolume(request));
  }

  async function runOperation(label: string, action: () => Promise<OperationResponse>) {
    busyLabel = label;
    pageError = '';
    try {
      lastResult = await action();
      await refresh();
    } catch (error) {
      pageError = formatError(error);
    } finally {
      busyLabel = '';
    }
  }

  function optionalValue(value: string): string | null {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : null;
  }

  function formatError(error: unknown): string {
    if (error instanceof Error) {
      return error.message;
    }
    return String(error);
  }

  function toolMissing(name: string): boolean {
    return !systemProbe?.tools.find((tool) => tool.name === name)?.path;
  }

  function entryClass(level: OutputEntry['level']): string {
    switch (level) {
      case 'success':
        return 'log-success';
      case 'warning':
        return 'log-warning';
      case 'error':
        return 'log-error';
      case 'detail':
        return 'log-detail';
      case 'progress':
        return 'log-progress';
      default:
        return '';
    }
  }
</script>

<svelte:head>
  <title>Lurker</title>
</svelte:head>

<main class="shell">
  <section class="hero">
    <div>
      <p class="eyebrow">Lurker</p>
      <h1>Linux volume creation, mount, and unmount in one desktop shell.</h1>
      <p class="lede">
        A minimal Tauri frontend over the shared Rust core. The app stays intentionally small while
        the repository is split for long-term growth.
      </p>
    </div>
    <div class="hero-actions">
      <button class="secondary" on:click={refresh} disabled={loading || busyLabel !== ''}>
        Refresh state
      </button>
      {#if busyLabel}
        <p class="status">{busyLabel}…</p>
      {:else if loading}
        <p class="status">Loading system state…</p>
      {/if}
    </div>
  </section>

  {#if pageError}
    <section class="banner error">{pageError}</section>
  {/if}

  <section class="grid">
    <div class="column">
      <section class="panel">
        <header>
          <h2>Create</h2>
          <p>File containers or destructive block-device creation.</p>
        </header>

        <form class="form" on:submit|preventDefault={submitCreate}>
          <label>
            <span>Target path</span>
            <input bind:value={createTarget} placeholder="/path/to/vault.img or /dev/sdb1" required />
          </label>

          <div class="row">
            <label>
              <span>Target kind</span>
              <select bind:value={createSourceKind}>
                <option value="file">File</option>
                <option value="block">Block device</option>
              </select>
            </label>

            <label>
              <span>Volume type</span>
              <select bind:value={createVolumeType}>
                <option value="luks">LUKS</option>
                <option value="veracrypt">VeraCrypt</option>
              </select>
            </label>
          </div>

          <label>
            <span>Cipher</span>
            <select bind:value={createCipher}>
              <option value="aes">AES</option>
              <option value="serpent">Serpent</option>
              <option value="twofish">Twofish</option>
            </select>
          </label>

          <label class:disabled={createSourceKind === 'block'}>
            <span>Size in GB</span>
            <input bind:value={createSizeGb} disabled={createSourceKind === 'block'} placeholder="4" />
          </label>

          <label>
            <span>Passphrase</span>
            <input bind:value={createPassphrase} type="password" placeholder="Required" required />
          </label>

          <label class="checkbox">
            <input bind:checked={createForce} type="checkbox" />
            <span>I understand destructive create requires force.</span>
          </label>

          <button type="submit" disabled={busyLabel !== ''}>Create volume</button>
        </form>
      </section>

      <section class="panel">
        <header>
          <h2>Mount</h2>
          <p>Auto-detects LUKS vs VeraCrypt unless you pin the type.</p>
        </header>

        <form class="form" on:submit|preventDefault={submitMount}>
          <label>
            <span>Source</span>
            <input bind:value={mountSource} placeholder="/path/to/vault.img or /dev/sdb1" required />
          </label>

          <label>
            <span>Mountpoint</span>
            <input bind:value={mountPoint} placeholder="/mnt/lurker" required />
          </label>

          <div class="row">
            <label>
              <span>Type</span>
              <select bind:value={mountVolumeType}>
                <option value="auto">Auto</option>
                <option value="luks">LUKS</option>
                <option value="veracrypt">VeraCrypt</option>
              </select>
            </label>

            <label>
              <span>Tag</span>
              <input bind:value={mountTag} placeholder="Optional LUKS tag" />
            </label>
          </div>

          <label>
            <span>Passphrase</span>
            <input bind:value={mountPassphrase} type="password" placeholder="Required" required />
          </label>

          <button type="submit" disabled={busyLabel !== ''}>Mount volume</button>
        </form>
      </section>

      <section class="panel">
        <header>
          <h2>Unmount</h2>
          <p>Accepts a mountpoint, mapper, source file, or source block device.</p>
        </header>

        <form class="form" on:submit|preventDefault={submitUnmount}>
          <label>
            <span>Target</span>
            <input bind:value={unmountTarget} placeholder="/mnt/lurker or /dev/mapper/lurker_x" required />
          </label>

          <div class="row">
            <label>
              <span>Type</span>
              <select bind:value={unmountVolumeType}>
                <option value="auto">Auto</option>
                <option value="luks">LUKS</option>
                <option value="veracrypt">VeraCrypt</option>
              </select>
            </label>

            <label>
              <span>Tag</span>
              <input bind:value={unmountTag} placeholder="Optional LUKS tag" />
            </label>
          </div>

          <button type="submit" disabled={busyLabel !== ''}>Unmount volume</button>
        </form>
      </section>
    </div>

    <div class="column">
      <section class="panel">
        <header>
          <h2>System Status</h2>
          <p>What the shared Rust core can see in this Linux session.</p>
        </header>

        <div class="status-grid">
          <div class="status-card">
            <span>Privilege</span>
            <strong>{systemProbe?.is_root ? 'Running as root' : 'User session'}</strong>
          </div>
          <div class="status-card">
            <span>VeraCrypt</span>
            <strong>{toolMissing('veracrypt') ? 'Not found' : 'Available'}</strong>
          </div>
          <div class="status-card">
            <span>pkexec</span>
            <strong>{toolMissing('pkexec') ? 'Not found' : 'Available'}</strong>
          </div>
        </div>

        <ul class="tool-list">
          {#if systemProbe}
            {#each systemProbe.tools as tool}
              <li class:missing={!tool.path && tool.required}>
                <span>{tool.name}</span>
                <code>{tool.path ?? 'missing'}</code>
              </li>
            {/each}
          {/if}
        </ul>
      </section>

      <section class="panel">
        <header>
          <h2>Active Volumes</h2>
          <p>Current `lurker_*` mappers resolved from `/dev/mapper` and mountinfo.</p>
        </header>

        {#if activeVolumes.length === 0}
          <p class="empty">No active lurker volumes found.</p>
        {:else}
          <ul class="volume-list">
            {#each activeVolumes as volume}
              <li>
                <strong>{volume.mapper_name}</strong>
                <span>{volume.mapper_path}</span>
                <span>{volume.mountpoint ?? 'not mounted'}</span>
                <span>{volume.filesystem_type ?? 'unknown filesystem'}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <section class="panel">
        <header>
          <h2>Last Operation</h2>
          <p>Buffered logs returned by the Rust core and helper path.</p>
        </header>

        {#if lastResult}
          <div class:failure={!lastResult.ok} class="result-state">
            {lastResult.ok ? 'Completed successfully' : lastResult.error ?? 'Operation failed'}
          </div>
          <div class="log-list">
            {#each lastResult.logs as entry}
              <pre class={entryClass(entry.level)}>{entry.message}</pre>
            {/each}
          </div>
        {:else}
          <p class="empty">No operations run yet.</p>
        {/if}
      </section>
    </div>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family:
      "IBM Plex Sans",
      "Segoe UI",
      sans-serif;
    background:
      radial-gradient(circle at top right, rgba(187, 227, 198, 0.45), transparent 24rem),
      linear-gradient(180deg, #f4f7f2 0%, #eef2ec 100%);
    color: #102116;
  }

  .shell {
    max-width: 1360px;
    margin: 0 auto;
    padding: 2rem;
  }

  .hero,
  .panel,
  .banner {
    background: rgba(255, 255, 255, 0.9);
    border: 1px solid rgba(16, 33, 22, 0.1);
    border-radius: 1.1rem;
    box-shadow: 0 18px 50px rgba(16, 33, 22, 0.08);
  }

  .hero {
    display: flex;
    justify-content: space-between;
    gap: 2rem;
    padding: 1.5rem 1.75rem;
    margin-bottom: 1rem;
  }

  .eyebrow {
    margin: 0 0 0.5rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    font-size: 0.78rem;
    color: #5b765b;
  }

  h1,
  h2,
  p {
    margin: 0;
  }

  h1 {
    font-size: clamp(1.75rem, 2vw + 1rem, 2.8rem);
    line-height: 1.08;
    max-width: 18ch;
  }

  .lede {
    margin-top: 0.8rem;
    max-width: 54ch;
    color: #405145;
  }

  .hero-actions {
    min-width: 14rem;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.8rem;
  }

  .status {
    color: #405145;
  }

  .banner {
    padding: 0.9rem 1rem;
    margin-bottom: 1rem;
  }

  .banner.error {
    border-color: rgba(159, 42, 42, 0.2);
    background: rgba(255, 242, 242, 0.92);
    color: #7b2020;
  }

  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 0.85fr);
    gap: 1rem;
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .panel {
    padding: 1.2rem;
  }

  .panel header {
    margin-bottom: 1rem;
  }

  .panel header p {
    margin-top: 0.35rem;
    color: #4b5f52;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  .row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.75rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.92rem;
    color: #314238;
  }

  label.disabled {
    opacity: 0.6;
  }

  input,
  select,
  button,
  code,
  pre {
    font: inherit;
  }

  input,
  select {
    padding: 0.75rem 0.85rem;
    border: 1px solid rgba(16, 33, 22, 0.14);
    border-radius: 0.75rem;
    background: rgba(255, 255, 255, 0.92);
    color: #102116;
  }

  .checkbox {
    flex-direction: row;
    align-items: center;
    gap: 0.65rem;
  }

  .checkbox input {
    width: 1rem;
    height: 1rem;
    padding: 0;
  }

  button {
    padding: 0.82rem 1rem;
    border-radius: 999px;
    border: none;
    background: #173d23;
    color: white;
    cursor: pointer;
    font-weight: 600;
  }

  button.secondary {
    background: #e7efe7;
    color: #173d23;
    border: 1px solid rgba(23, 61, 35, 0.12);
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .status-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.7rem;
    margin-bottom: 1rem;
  }

  .status-card {
    padding: 0.9rem;
    border-radius: 0.85rem;
    background: #f4f7f2;
    border: 1px solid rgba(16, 33, 22, 0.08);
  }

  .status-card span {
    display: block;
    font-size: 0.78rem;
    color: #617166;
    margin-bottom: 0.35rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .tool-list,
  .volume-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .tool-list li,
  .volume-list li {
    display: grid;
    gap: 0.2rem;
    padding: 0.8rem 0.9rem;
    background: #f8faf7;
    border: 1px solid rgba(16, 33, 22, 0.08);
    border-radius: 0.85rem;
  }

  .tool-list li.missing {
    background: #fff4f4;
    border-color: rgba(159, 42, 42, 0.14);
  }

  code,
  .volume-list span,
  .empty,
  .result-state,
  pre {
    color: #48584d;
  }

  .empty {
    padding: 0.85rem 0;
  }

  .result-state {
    padding: 0.8rem 0.9rem;
    border-radius: 0.75rem;
    background: #f4f7f2;
    margin-bottom: 0.85rem;
  }

  .result-state.failure {
    background: #fff3f3;
    color: #7b2020;
  }

  .log-list {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  pre {
    margin: 0;
    padding: 0.8rem 0.9rem;
    border-radius: 0.75rem;
    background: #f8faf7;
    border: 1px solid rgba(16, 33, 22, 0.08);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .log-detail {
    background: #f1f7fb;
  }

  .log-success {
    background: #eef8ef;
    color: #1d5f25;
  }

  .log-warning {
    background: #fff8e8;
    color: #8a650e;
  }

  .log-error {
    background: #fff1f1;
    color: #8a2121;
  }

  .log-progress {
    background: #f5f5f5;
    color: #4f4f4f;
  }

  @media (max-width: 980px) {
    .shell {
      padding: 1rem;
    }

    .hero,
    .grid,
    .row,
    .status-grid {
      grid-template-columns: 1fr;
      display: grid;
    }

    .hero {
      display: grid;
    }

    .hero-actions {
      align-items: stretch;
      min-width: 0;
    }
  }
</style>
