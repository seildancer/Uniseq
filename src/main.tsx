import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import './styles.css';

type Span = { start: number; end: number };
type SourceAnchor = { file_path: string; span: Span; snippet: string };
type TaskState = 'Todo' | 'Done';
type PageRef = { raw: string; page_path: string; span?: Span };
type Entry = { runtime_id: string; text: string; level: { Heading?: number } | 'ListItem' | 'Paragraph'; task?: TaskState | null; links: PageRef[]; tags: PageRef[]; anchor: SourceAnchor };
type WorkspaceWarning = { path?: string | null; kind?: string; message: string };
type WorkspaceSummary = { root: string; journals: Array<{ date: string; path: string }>; pages: Array<{ page_path: string; path: string }>; warnings: WorkspaceWarning[]; config?: { schema_version: number; app_version: string } };
type IndexedPage = { page_path: string; has_file: boolean; aliases?: string[]; own_entry_ids?: string[]; incoming_entry_ids: string[]; outbound_pages?: string[] };
type WorkspaceIndex = { pages: Record<string, IndexedPage>; entries: Entry[] };
type Snapshot = { workspace: WorkspaceSummary; index: WorkspaceIndex };
type PageProjection = { page_path: string; own_entries: Entry[]; incoming_entries: Entry[]; aliases: string[] };
type SearchHit = { entry: Entry; score: number };
type TimelineEntry = { date: string; entry: Entry };
type ViewKind = 'journal' | 'page' | 'search' | 'tasks' | 'timeline' | 'graph' | 'assets' | 'pdf' | 'whiteboard' | 'flashcards' | 'plugins' | 'sync' | 'settings' | 'browser' | 'mobile';
type View = { kind: Exclude<ViewKind, 'page'> } | { kind: 'page'; page: string };
type PageNode = { name: string; path: string; children: Record<string, PageNode> };
type Filters = { text: string; state: 'all' | TaskState; page: string; tag: string; from: string; to: string; includeAssets: boolean };
type SearchOptions = { text?: string | null; page_filters: string[]; tag_filters: string[]; task_state?: 'Todo' | 'Done' | 'Any' | null; date_from?: string | null; date_to?: string | null; include_assets: boolean };
type SearchResult = { entries: Entry[]; total: number; matched_on: string[] };
type TaskQuery = { state?: 'Todo' | 'Done' | 'Any' | null; page?: string | null; tag?: string | null; date_from?: string | null; date_to?: string | null };
type GraphNode = { id: string; kind: 'Page' | 'Tag' | 'Journal' | 'Asset'; label: string };
type GraphEdge = { from: string; to: string; label?: string | null };
type GraphData = { nodes: GraphNode[]; edges: GraphEdge[] };
type ReferencedAnchor = { page_path: string; anchor: SourceAnchor };
type AssetRecord = { relative_path: string; size_bytes: number; modified_ms: number; referenced_by: ReferencedAnchor[] };
type AssetRegistry = { assets: AssetRecord[] };
type FeatureStatus = 'Available' | 'Disabled' | 'Deferred';
type FeatureSurfaceRecord = { id: string; name: string; status: FeatureStatus; storage_dir?: string | null; note?: string | null };
type FeatureRegistry = { surfaces: FeatureSurfaceRecord[] };
type PluginManifest = { id: string; name: string; version: string; description?: string | null; capabilities: string[]; entry?: string | null; disabled: boolean };
type PluginRegistry = { plugins: PluginManifest[] };
type SyncState = { status: string | { Error: string }; manifest_path: string; local_seq: number; conflicts: Array<{ path: string; local_hash: string; remote_hash?: string | null; detected_at_ms: number }>; errors: string[] };
type AppSettings = { editor: { auto_save_seconds: number; default_extension: string; spell_check: boolean; indent_size: number }; theme: { mode: string; accent: string; font_size: number }; calendar: { week_start: number; date_format: string; show_week_numbers: boolean }; sync: { enabled: boolean; manifest_dir: string; last_sync_ms?: number | null }; plugins: { enabled: boolean; plugin_dirs: string[]; disabled_plugins: string[] } };

const today = new Date().toISOString().slice(0, 10);
const monthFmt = new Intl.DateTimeFormat(undefined, { month: 'long', year: 'numeric' });
const dayFmt = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric' });
const defaultSettings: AppSettings = { editor: { auto_save_seconds: 30, default_extension: 'md', spell_check: true, indent_size: 2 }, theme: { mode: 'system', accent: '#e14b2d', font_size: 14 }, calendar: { week_start: 1, date_format: '%Y-%m-%d', show_week_numbers: false }, sync: { enabled: false, manifest_dir: '.uniseq/sync', last_sync_ms: null }, plugins: { enabled: false, plugin_dirs: ['app/plugins', 'plugins'], disabled_plugins: [] } };

function App() {
  const [workspacePath, setWorkspacePath] = useState('');
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [view, setView] = useState<View>({ kind: 'journal' });
  const [journalDate, setJournalDate] = useState(today);
  const [calendarMonth, setCalendarMonth] = useState(today.slice(0, 7));
  const [draft, setDraft] = useState('- ');
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [pageProjection, setPageProjection] = useState<PageProjection | null>(null);
  const [searchHits, setSearchHits] = useState<SearchHit[]>([]);
  const [searchResult, setSearchResult] = useState<SearchResult>({ entries: [], total: 0, matched_on: [] });
  const [taskEntries, setTaskEntries] = useState<Entry[]>([]);
  const [timelineEntries, setTimelineEntries] = useState<TimelineEntry[]>([]);
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] });
  const [assetRegistry, setAssetRegistry] = useState<AssetRegistry>({ assets: [] });
  const [featureRegistry, setFeatureRegistry] = useState<FeatureRegistry>({ surfaces: [] });
  const [pluginRegistry, setPluginRegistry] = useState<PluginRegistry>({ plugins: [] });
  const [syncState, setSyncState] = useState<SyncState | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteQuery, setPaletteQuery] = useState('');
  const [assetFrom, setAssetFrom] = useState('inbox/example.pdf');
  const [assetTo, setAssetTo] = useState('library/example.pdf');
  const [searchFilters, setSearchFilters] = useState<Filters>({ text: '', state: 'all', page: '', tag: '', from: '', to: '', includeAssets: true });
  const [taskFilters, setTaskFilters] = useState<Filters>({ text: '', state: 'all', page: '', tag: '', from: '', to: '', includeAssets: false });
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);

  const pages = useMemo(() => Object.keys(snapshot?.index.pages ?? {}).sort((a, b) => a.localeCompare(b)), [snapshot]);
  const journalDates = useMemo(() => new Set(snapshot?.workspace.journals.map((j) => j.date) ?? []), [snapshot]);
  const warnings = snapshot?.workspace.warnings ?? [];
  const visibleEntries = useMemo(() => {
    if (!snapshot) return [];
    if (view.kind === 'journal') return snapshot.index.entries.filter((e) => e.anchor.file_path.includes(`${journalDate}.md`));
    if (view.kind === 'tasks') return taskEntries;
    if (view.kind === 'timeline') return timelineEntries.map((item) => item.entry);
    if (view.kind === 'search') return searchResult.entries;
    if (view.kind === 'page') return pageProjection ? [...pageProjection.own_entries, ...pageProjection.incoming_entries] : [];
    return [];
  }, [snapshot, view, journalDate, searchResult, pageProjection, taskEntries, timelineEntries]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => { if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); setPaletteOpen((open) => !open); } if (event.key === 'Escape') setPaletteOpen(false); };
    window.addEventListener('keydown', onKey); return () => window.removeEventListener('keydown', onKey);
  }, []);

  useEffect(() => {
    if (!workspacePath.trim() || !snapshot) return;
    let cancelled = false;
    async function loadProjection() {
      try {
        if (view.kind === 'page') { const projection = await invoke<PageProjection>('query_page_cmd', { path: workspacePath, page: view.page }); if (!cancelled) setPageProjection(projection); }
        if (view.kind === 'search') { const result = await invoke<SearchResult>('structured_search_cmd', { path: workspacePath, options: toSearchOptions(searchFilters) }); if (!cancelled) setSearchResult(result); }
        if (view.kind === 'tasks') { const tasks = await invoke<Entry[]>('task_query_cmd', { path: workspacePath, query: toTaskQuery(taskFilters) }); if (!cancelled) setTaskEntries(tasks); }
        if (view.kind === 'timeline') { const timeline = await invoke<TimelineEntry[]>('query_timeline_cmd', { path: workspacePath }); if (!cancelled) setTimelineEntries(timeline); }
        if (view.kind === 'graph') { const graph = await invoke<GraphData>('graph_data_cmd', { path: workspacePath }); if (!cancelled) setGraphData(graph); }
        if (view.kind === 'assets') { const registry = await invoke<AssetRegistry>('asset_registry_cmd', { path: workspacePath }); if (!cancelled) setAssetRegistry(registry); }
        if (featureSurfaces.some((surface) => surface.kind === view.kind) || view.kind === 'settings') { const registry = await invoke<FeatureRegistry>('feature_surfaces_cmd', { path: workspacePath }); if (!cancelled) setFeatureRegistry(registry); }
        if (view.kind === 'plugins') { const registry = await invoke<PluginRegistry>('plugins_cmd', { path: workspacePath }); if (!cancelled) setPluginRegistry(registry); }
        if (view.kind === 'sync') { const state = await invoke<SyncState>('sync_plan_cmd', { path: workspacePath }); if (!cancelled) setSyncState(state); }
        if (view.kind === 'settings') { const loaded = await invoke<AppSettings>('load_settings_cmd', { path: workspacePath }); if (!cancelled) setSettings(loaded); }
        if (view.kind === 'journal') { const entries = await invoke<Entry[]>('query_journal_cmd', { path: workspacePath, date: journalDate }); if (!cancelled) setSnapshot((current) => current ? { ...current, index: { ...current.index, entries: mergeEntries(current.index.entries, entries) } } : current); }
      } catch (err) { if (!cancelled) setError(String(err)); }
    }
    loadProjection(); return () => { cancelled = true; };
  }, [view, searchFilters, taskFilters, workspacePath, journalDate, Boolean(snapshot)]);

  async function refresh(path = workspacePath) { if (!path.trim()) return; setError(null); setNotice(null); try { setSnapshot(await invoke<Snapshot>('open_workspace_cmd', { path })); } catch (err) { setError(String(err)); } }
  async function createThenOpen() { setError(null); try { await invoke('create_workspace_cmd', { path: workspacePath }); await refresh(); } catch (err) { setError(String(err)); } }
  async function appendEntry() { if (!draft.trim()) return; await invoke('append_journal_entry_cmd', { path: workspacePath, date: journalDate, markdown: draft }); setDraft('- '); await refresh(); }
  async function toggle(entry: Entry) { await invoke('toggle_task_cmd', { path: workspacePath, anchor: entry.anchor, desired: null }); await refresh(); if (view.kind === 'tasks') setTaskEntries(await invoke<Entry[]>('task_query_cmd', { path: workspacePath, query: toTaskQuery(taskFilters) })); }
  async function moveAsset() { if (!assetFrom.trim() || !assetTo.trim()) return; setError(null); try { await invoke('move_asset_cmd', { path: workspacePath, fromRelative: assetFrom, toRelative: assetTo }); setNotice('Asset move/import handed to Rust successfully.'); await refresh(); } catch (err) { setError(String(err)); } }
  async function cacheSnapshot() { try { await invoke('save_cache_snapshot_cmd', { path: workspacePath }); setNotice('Index snapshot saved by Rust.'); } catch (err) { setError(String(err)); } }
  async function clearCache() { try { await invoke('clear_cache_snapshot_cmd', { path: workspacePath }); setNotice('Rust cache cleared; next open rebuilds.'); } catch (err) { setError(String(err)); } }
  async function syncNow() { try { const state = await invoke<SyncState>('sync_now_cmd', { path: workspacePath }); setSyncState(state); setNotice('Local sync manifest refreshed by Rust.'); } catch (err) { setError(String(err)); } }
  async function saveAppSettings() { try { await invoke('save_settings_cmd', { path: workspacePath, settings }); setNotice('Settings saved through Rust.'); } catch (err) { setError(String(err)); } }

  const paletteItems = useMemo(() => [
    { label: 'Open daily journal', detail: journalDate, run: () => setView({ kind: 'journal' }) }, { label: 'Search workspace', detail: 'structured_search_cmd', run: () => setView({ kind: 'search' }) }, { label: 'Task rollup', detail: 'task_query_cmd', run: () => setView({ kind: 'tasks' }) }, { label: 'Timeline', detail: 'query_timeline_cmd', run: () => setView({ kind: 'timeline' }) },
    { label: 'Graph map', detail: 'graph_data_cmd', run: () => setView({ kind: 'graph' }) }, { label: 'Asset registry', detail: 'asset_registry_cmd', run: () => setView({ kind: 'assets' }) }, { label: 'Sync status', detail: 'sync_plan_cmd', run: () => setView({ kind: 'sync' }) }, { label: 'Plugins', detail: 'plugins_cmd', run: () => setView({ kind: 'plugins' }) }, { label: 'Settings', detail: 'load/save settings', run: () => setView({ kind: 'settings' }) },
    ...featureSurfaces.map((s) => ({ label: s.title, detail: s.status, run: () => setView({ kind: s.kind }) })), ...pages.map((page) => ({ label: page, detail: 'page', run: () => setView({ kind: 'page', page }) })),
  ].filter((item) => `${item.label} ${item.detail}`.toLowerCase().includes(paletteQuery.toLowerCase())), [pages, paletteQuery, journalDate]);

  return <main className="app-shell">
    <aside className="rail">
      <div className="brand"><span>◒</span><div><strong>Uniseq</strong><small>journal-first graph</small></div></div>
      <label className="workspace-picker"><span>Workspace path</span><input value={workspacePath} onChange={(e) => setWorkspacePath(e.target.value)} placeholder="C:\\notes\\uniseq" /></label>
      <div className="button-row"><button onClick={() => refresh()}>Open</button><button className="ghost" onClick={createThenOpen}>Create</button></div>
      <nav className="primary-nav"><NavButton view={view} kind="journal" label="Daily journal" setView={setView} /><button className={view.kind === 'timeline' ? 'active' : ''} onClick={() => setView({ kind: 'timeline' })}>Calendar / timeline</button><NavButton view={view} kind="search" label="Search" setView={setView} /><NavButton view={view} kind="tasks" label="Tasks" setView={setView} /></nav>
      <PageTree pages={pages} current={view.kind === 'page' ? view.page : ''} onOpen={(page) => setView({ kind: 'page', page })} />
      <nav className="secondary-nav"><NavButton view={view} kind="graph" label="Graph" setView={setView} /><NavButton view={view} kind="assets" label="Assets" setView={setView} /><NavButton view={view} kind="sync" label="Sync" setView={setView} /><NavButton view={view} kind="plugins" label="Plugins" setView={setView} /><NavButton view={view} kind="settings" label="Settings" setView={setView} /><button onClick={() => setPaletteOpen(true)}>Command palette <kbd>Ctrl K</kbd></button></nav>
    </aside>
    <section className="surface">
      <header className="topbar"><div><p className="eyebrow">Rust-owned markdown · architecture surfaces</p><h1>{title(view)}</h1>{view.kind === 'page' && pageProjection?.aliases.length ? <p className="aliases">Aliases: {pageProjection.aliases.join(', ')}</p> : null}</div><div className="top-actions">{view.kind === 'journal' && <CalendarPicker month={calendarMonth} value={journalDate} marked={journalDates} onMonth={setCalendarMonth} onPick={setJournalDate} />}</div></header>
      {error && <p className="error">{error}</p>}{notice && <p className="notice">{notice}</p>}{warnings.length > 0 && <WarningStrip warnings={warnings} />}{!snapshot && <EmptyState />}
      {snapshot && <>
        {view.kind === 'journal' && <section className="composer"><textarea value={draft} onChange={(e) => setDraft(e.target.value)} placeholder="Capture a thought. Use #tags, [[pages]], and - [ ] tasks." /><button onClick={appendEntry}>Append through Rust</button></section>}
        {view.kind === 'search' && <FilterPanel label="Search filters" filters={searchFilters} setFilters={setSearchFilters} pages={pages} includeAssetToggle />}
        {view.kind === 'tasks' && <FilterPanel label="Task filters" filters={taskFilters} setFilters={setTaskFilters} pages={pages} />}
        {view.kind === 'search' && <p className="result-count">{searchResult.total} Rust result{searchResult.total === 1 ? '' : 's'} {searchResult.matched_on.length ? `· matched ${searchResult.matched_on.join(', ')}` : ''}</p>}
        {view.kind === 'tasks' && <p className="result-count">{taskEntries.length} task{taskEntries.length === 1 ? '' : 's'} returned by task_query_cmd</p>}
        {view.kind === 'page' && <PageProjectionView projection={pageProjection} onToggle={toggle} />}
        {['journal', 'search', 'tasks', 'timeline'].includes(view.kind) && <section className={view.kind === 'timeline' ? 'timeline' : 'entry-grid'}>{visibleEntries.map((entry) => <EntryCard entry={entry} onToggle={() => toggle(entry)} key={entry.runtime_id} hit={searchHits.find((h) => h.entry.runtime_id === entry.runtime_id)} />)}</section>}
        {view.kind === 'graph' && <GraphView graph={graphData} onOpen={(page) => setView({ kind: 'page', page })} />}
        {view.kind === 'assets' && <AssetsView registry={assetRegistry} assetFrom={assetFrom} assetTo={assetTo} setAssetFrom={setAssetFrom} setAssetTo={setAssetTo} onMove={moveAsset} />}
        {view.kind === 'sync' && <SyncView state={syncState} onSync={syncNow} />}
        {view.kind === 'plugins' && <PluginsView registry={pluginRegistry} />}
        {featureSurfaces.some((surface) => surface.kind === view.kind) && <FeatureSurface kind={view.kind} registry={featureRegistry} workspaceRoot={snapshot.workspace.root} />}
        {view.kind === 'settings' && <SettingsView snapshot={snapshot} settings={settings} setSettings={setSettings} onSave={saveAppSettings} onSaveCache={cacheSnapshot} onClearCache={clearCache} featureRegistry={featureRegistry} />}
        {visibleEntries.length === 0 && ['journal', 'search', 'tasks', 'timeline'].includes(view.kind) && <p className="quiet">No entries in this projection yet.</p>}
      </>}
    </section>
    {paletteOpen && <CommandPalette query={paletteQuery} setQuery={setPaletteQuery} items={paletteItems} close={() => setPaletteOpen(false)} />}
  </main>;
}

function NavButton({ view, kind, label, setView }: { view: View; kind: Exclude<ViewKind, 'page'>; label: string; setView: (view: View) => void }) { return <button className={view.kind === kind ? 'active' : ''} onClick={() => setView({ kind })}>{label}</button>; }
function mergeEntries(existing: Entry[], fresh: Entry[]) { const merged = new Map(existing.map((entry) => [entry.runtime_id, entry])); fresh.forEach((entry) => merged.set(entry.runtime_id, entry)); return [...merged.values()]; }
function title(view: View) { if (view.kind === 'page') return view.page; if (view.kind === 'journal') return 'Daily ledger'; if (view.kind === 'search') return 'Search observatory'; if (view.kind === 'tasks') return 'Task rollup'; if (view.kind === 'timeline') return 'Timeline atlas'; if (view.kind === 'graph') return 'Graph cartography'; if (view.kind === 'assets') return 'Asset registry'; if (view.kind === 'sync') return 'Sync weather'; if (view.kind === 'plugins') return 'Plugin airlock'; if (view.kind === 'settings') return 'Workspace cockpit'; return featureSurfaces.find((s) => s.kind === view.kind)?.title ?? 'Surface'; }

function taskState(value: Filters['state']): 'Todo' | 'Done' | 'Any' | null { return value === 'all' ? null : value; }
function toSearchOptions(filters: Filters): SearchOptions { return { text: filters.text.trim() || null, page_filters: filters.page.trim() ? [filters.page.trim()] : [], tag_filters: filters.tag.trim() ? [filters.tag.trim()] : [], task_state: taskState(filters.state), date_from: filters.from || null, date_to: filters.to || null, include_assets: filters.includeAssets }; }
function toTaskQuery(filters: Filters): TaskQuery { return { state: taskState(filters.state), page: filters.page.trim() || null, tag: filters.tag.trim() || null, date_from: filters.from || null, date_to: filters.to || null }; }
function pageFromGraphId(id: string) { return id.replace(/^(page|tag):/, '').replace(/^journal:/, 'journal/'); }
function surfaceStatusClass(status?: FeatureStatus | string) { return status === 'Available' ? 'status-pill' : 'status-pill deferred'; }

function matchesFilters(entry: Entry, filters: Filters) {
  if (filters.state !== 'all' && entry.task !== filters.state) return false;
  if (filters.page && !entry.links.some((link) => link.page_path.toLowerCase().includes(filters.page.toLowerCase())) && !entry.anchor.file_path.toLowerCase().includes(filters.page.toLowerCase())) return false;
  if (filters.tag && !entry.tags.some((tag) => tag.page_path.toLowerCase().includes(filters.tag.toLowerCase()))) return false;
  const date = entry.anchor.file_path.match(/(\d{4}-\d{2}-\d{2})\.md/)?.[1] ?? '';
  if (filters.from && date && date < filters.from) return false; if (filters.to && date && date > filters.to) return false;
  if (!filters.includeAssets && /assets?[/\\]|\.(pdf|png|jpe?g|gif|svg|webp|mp4|mov|mp3|wav)/i.test(entry.text)) return false;
  return true;
}

function FilterPanel({ label, filters, setFilters, pages, includeAssetToggle = false }: { label: string; filters: Filters; setFilters: (filters: Filters) => void; pages: string[]; includeAssetToggle?: boolean }) {
  const patch = (next: Partial<Filters>) => setFilters({ ...filters, ...next });
  return <section className="filter-panel"><strong>{label}</strong><input value={filters.text} onChange={(e) => patch({ text: e.target.value })} placeholder="Text query sent to Rust search when supported" /><Segmented value={filters.state} onChange={(state) => patch({ state })} options={['all', 'Todo', 'Done']} /><input list="pages" value={filters.page} onChange={(e) => patch({ page: e.target.value })} placeholder="Page / namespace" /><datalist id="pages">{pages.map((page) => <option value={page} key={page} />)}</datalist><input value={filters.tag} onChange={(e) => patch({ tag: e.target.value })} placeholder="Tag filter" /><input type="date" value={filters.from} onChange={(e) => patch({ from: e.target.value })} /><input type="date" value={filters.to} onChange={(e) => patch({ to: e.target.value })} />{includeAssetToggle && <label className="checkline"><input type="checkbox" checked={filters.includeAssets} onChange={(e) => patch({ includeAssets: e.target.checked })} /> include asset references</label>}</section>;
}

function layoutGraph(graph: GraphData) { return graph.nodes.map((node, i) => ({ ...node, x: 54 + (i % 7) * 122, y: 64 + Math.floor(i / 7) * 94, weight: graph.edges.filter((edge) => edge.from === node.id || edge.to === node.id).length })); }
function GraphView({ graph, onOpen }: { graph: GraphData; onOpen: (page: string) => void }) { const nodes = layoutGraph(graph); return <section className="graph-wrap"><svg viewBox="0 0 900 520" role="img"><defs><linearGradient id="wire" x1="0" x2="1"><stop stopColor="#e14b2d" /><stop offset="1" stopColor="#174f78" /></linearGradient></defs>{graph.edges.map((edge) => { const a = nodes.find((n) => n.id === edge.from); const b = nodes.find((n) => n.id === edge.to); return a && b ? <line key={`${edge.from}-${edge.to}-${edge.label ?? ''}`} x1={a.x} y1={a.y} x2={b.x} y2={b.y} /> : null; })}{nodes.map((node) => <g key={node.id} onClick={() => node.kind !== 'Journal' && onOpen(pageFromGraphId(node.id))}><circle className={`node-${node.kind.toLowerCase()}`} cx={node.x} cy={node.y} r={15 + Math.min(node.weight, 8) * 2} /><text x={node.x + 22} y={node.y + 4}>{node.label}</text></g>)}</svg><div className="graph-list"><p className="quiet">{graph.nodes.length} nodes · {graph.edges.length} edges from graph_data_cmd</p>{nodes.slice(0, 18).map((node) => <button className="ghost" key={node.id} onClick={() => node.kind !== 'Journal' && onOpen(pageFromGraphId(node.id))}>{node.label}<span>{node.kind} · {node.weight}</span></button>)}</div></section>; }

function AssetsView({ registry, assetFrom, assetTo, setAssetFrom, setAssetTo, onMove }: { registry: AssetRegistry; assetFrom: string; assetTo: string; setAssetFrom: (value: string) => void; setAssetTo: (value: string) => void; onMove: () => void }) { return <section className="asset-registry"><div className="asset-box"><h3>Move / import via Rust</h3><p>Registry and referenced-by counts come from asset_registry_cmd. The frontend never writes workspace files.</p><input value={assetFrom} onChange={(e) => setAssetFrom(e.target.value)} /><input value={assetTo} onChange={(e) => setAssetTo(e.target.value)} /><button onClick={onMove}>Move through Rust</button></div><div className="registry-grid">{registry.assets.length ? registry.assets.map((asset) => <article className="registry-card" key={asset.relative_path}><strong>{asset.relative_path}</strong><span>{asset.referenced_by.length} referenced-by count</span><small>{formatBytes(asset.size_bytes)} · {asset.referenced_by[0]?.page_path ?? 'unreferenced'}</small></article>) : <p className="quiet">No assets discovered by Rust yet.</p>}</div></section>; }
function formatBytes(size: number) { if (size > 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`; if (size > 1024) return `${(size / 1024).toFixed(1)} KB`; return `${size} B`; }
function SyncView({ state, onSync }: { state: SyncState | null; onSync: () => void }) { const status = typeof state?.status === 'string' ? state.status : state?.status ? `Error: ${state.status.Error}` : 'Loading'; return <section className="surface-cards"><article><span className="status-pill">{status}</span><h2>Local sync plan</h2><p>sync_plan_cmd reports sequence {state?.local_seq ?? 0} at {state?.manifest_path ?? '.uniseq/sync/manifest.toml'}.</p><button onClick={onSync}>Run sync_now_cmd</button></article><article><span className={state?.conflicts.length ? 'status-pill deferred' : 'status-pill'}>{state?.conflicts.length ?? 0} conflicts</span><h2>Conflict ledger</h2><p>Conflicts stay visible for review; no silent remote overwrite is implied.</p>{state?.conflicts.slice(0, 4).map((conflict) => <small key={`${conflict.path}-${conflict.detected_at_ms}`}>{conflict.path}</small>)}</article><article><span className="status-pill">offline-ready</span><h2>Errors</h2><p>{state?.errors.length ? state.errors.join(' · ') : 'No sync errors reported by Rust.'}</p></article></section>; }
function PluginsView({ registry }: { registry: PluginRegistry }) { return <section className="surface-cards">{registry.plugins.length ? registry.plugins.map((plugin) => <article key={plugin.id}><span className={plugin.disabled ? 'status-pill deferred' : 'status-pill'}>{plugin.disabled ? 'disabled' : 'scanned'}</span><h2>{plugin.name}</h2><p>{plugin.description ?? `Manifest ${plugin.id} v${plugin.version}`}</p><small>Capabilities: {plugin.capabilities.length ? plugin.capabilities.join(', ') : 'none'} · deny-by-default</small></article>) : <article><span className="status-pill deferred">empty</span><h2>No manifests</h2><p>plugins_cmd found no accepted manifests in app/plugins or plugins.</p><small>Unknown capabilities are rejected by Rust before reaching the UI.</small></article>}</section>; }

const featureSurfaces = [
  { kind: 'pdf' as const, title: 'PDF capture desk', status: 'placeholder', dir: 'assets/pdf' }, { kind: 'whiteboard' as const, title: 'Whiteboard studio', status: 'placeholder', dir: 'assets/whiteboards' }, { kind: 'flashcards' as const, title: 'Flashcard kiln', status: 'placeholder', dir: 'app/flashcards' }, { kind: 'browser' as const, title: 'Browser clipper', status: 'deferred', dir: 'app/clips' }, { kind: 'mobile' as const, title: 'Mobile inbox', status: 'deferred', dir: 'app/mobile' },
];
function FeatureSurface({ kind, registry, workspaceRoot }: { kind: ViewKind; registry: FeatureRegistry; workspaceRoot: string }) { const fallback = featureSurfaces.find((item) => item.kind === kind); const rustSurface = registry.surfaces.find((surface) => surface.id === kind || (kind === 'plugins' && surface.id === 'plugin')); if (!fallback && !rustSurface) return null; const titleText = rustSurface?.name ?? fallback?.title ?? String(kind); const status = rustSurface?.status ?? 'Deferred'; const storage = rustSurface?.storage_dir ?? fallback?.dir; return <section className="feature-hero"><span className={surfaceStatusClass(status)}>{status}</span><h2>{titleText}</h2><p>{rustSurface?.note ?? 'No fake full editor is mounted here. This surface reserves the architecture-compliant workflow and capture affordance until Rust exposes dedicated content commands.'}</p>{storage && <div><strong>Storage</strong><code>{workspaceRoot}/{storage}</code></div>}<button disabled>{status === 'Available' ? 'Open through dedicated command when exposed' : 'Deferred by feature_surfaces_cmd'}</button></section>; }
function SettingsView({ snapshot, settings, setSettings, onSave, onSaveCache, onClearCache, featureRegistry }: { snapshot: Snapshot; settings: AppSettings; setSettings: (settings: AppSettings) => void; onSave: () => void; onSaveCache: () => void; onClearCache: () => void; featureRegistry: FeatureRegistry }) { const patch = (next: Partial<AppSettings>) => setSettings({ ...settings, ...next }); const patchEditor = (editor: Partial<AppSettings['editor']>) => patch({ editor: { ...settings.editor, ...editor } }); const patchTheme = (theme: Partial<AppSettings['theme']>) => patch({ theme: { ...settings.theme, ...theme } }); const patchSync = (sync: Partial<AppSettings['sync']>) => patch({ sync: { ...settings.sync, ...sync } }); const patchPlugins = (plugins: Partial<AppSettings['plugins']>) => patch({ plugins: { ...settings.plugins, ...plugins } }); return <section className="settings-grid"><div className="stats"><span><strong>{snapshot.workspace.journals.length}</strong> journals</span><span><strong>{snapshot.workspace.pages.length}</strong> pages</span><span><strong>{snapshot.workspace.warnings.length}</strong> warnings</span></div><label><span>Theme mode</span><input value={settings.theme.mode} onChange={(e) => patchTheme({ mode: e.target.value })} /></label><label><span>Accent color</span><input value={settings.theme.accent} onChange={(e) => patchTheme({ accent: e.target.value })} /></label><label><span>Font size</span><input type="number" value={settings.theme.font_size} onChange={(e) => patchTheme({ font_size: Number(e.target.value) })} /></label><label><span>Auto-save seconds</span><input type="number" value={settings.editor.auto_save_seconds} onChange={(e) => patchEditor({ auto_save_seconds: Number(e.target.value) })} /></label><label className="checkline"><input type="checkbox" checked={settings.editor.spell_check} onChange={(e) => patchEditor({ spell_check: e.target.checked })} /> spell check</label><label className="checkline"><input type="checkbox" checked={settings.sync.enabled} onChange={(e) => patchSync({ enabled: e.target.checked })} /> sync manifest enabled</label><label><span>Sync manifest dir</span><input value={settings.sync.manifest_dir} onChange={(e) => patchSync({ manifest_dir: e.target.value })} /></label><label className="checkline"><input type="checkbox" checked={settings.plugins.enabled} onChange={(e) => patchPlugins({ enabled: e.target.checked })} /> plugins enabled</label><div className="button-row"><button onClick={onSave}>Save settings</button><button className="ghost" onClick={onSaveCache}>Save cache</button></div><button className="ghost" onClick={onClearCache}>Clear cache snapshot</button><p className="quiet">Settings are loaded and saved through load_settings_cmd/save_settings_cmd. {featureRegistry.surfaces.length} feature surfaces reported by Rust.</p></section>; }

function CalendarPicker({ month, value, marked, onMonth, onPick }: { month: string; value: string; marked: Set<string>; onMonth: (month: string) => void; onPick: (date: string) => void }) { const monthDate = new Date(`${month}-01T12:00:00`); const days = calendarDays(month); const shift = (delta: number) => { const next = new Date(monthDate); next.setMonth(next.getMonth() + delta); onMonth(next.toISOString().slice(0, 7)); }; return <div className="calendar-card"><div className="calendar-head"><button className="mini" onClick={() => shift(-1)}>‹</button><strong>{monthFmt.format(monthDate)}</strong><button className="mini" onClick={() => shift(1)}>›</button></div><div className="weekdays">{['M', 'T', 'W', 'T', 'F', 'S', 'S'].map((d, i) => <span key={`${d}${i}`}>{d}</span>)}</div><div className="calendar-grid">{days.map((date, i) => <button key={date || `blank-${i}`} disabled={!date} className={`${date === value ? 'picked' : ''} ${date && marked.has(date) ? 'marked' : ''}`} onClick={() => date && onPick(date)}>{date ? new Date(`${date}T12:00:00`).getDate() : ''}</button>)}</div></div>; }
function calendarDays(month: string) { const first = new Date(`${month}-01T12:00:00`); const startPad = (first.getDay() + 6) % 7; const count = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate(); return [...Array(startPad).fill(''), ...Array.from({ length: count }, (_, i) => `${month}-${String(i + 1).padStart(2, '0')}`)]; }
function PageTree({ pages, current, onOpen }: { pages: string[]; current: string; onOpen: (page: string) => void }) { const root = useMemo(() => buildTree(pages), [pages]); return <section className="page-list"><h2>Pages</h2>{Object.values(root.children).map((node) => <TreeNode key={node.path} node={node} current={current} onOpen={onOpen} />)}</section>; }
function buildTree(pages: string[]) { const root: PageNode = { name: '', path: '', children: {} }; pages.forEach((page) => { let cursor = root; page.split('/').forEach((part, index, all) => { const path = all.slice(0, index + 1).join('/'); cursor.children[part] ??= { name: part, path, children: {} }; cursor = cursor.children[part]; }); }); return root; }
function TreeNode({ node, current, onOpen }: { node: PageNode; current: string; onOpen: (page: string) => void }) { const children = Object.values(node.children); return <div className="tree-node"><button className={current === node.path ? 'active' : ''} onClick={() => onOpen(node.path)}><span>{children.length ? '▸' : '·'}</span>{node.name}</button>{children.length > 0 && <div className="tree-children">{children.map((child) => <TreeNode key={child.path} node={child} current={current} onOpen={onOpen} />)}</div>}</div>; }
function PageProjectionView({ projection, onToggle }: { projection: PageProjection | null; onToggle: (entry: Entry) => void }) { if (!projection) return <p className="quiet">Loading page projection through Rust…</p>; return <div className="projection"><section><h2>Own content</h2>{projection.own_entries.length ? projection.own_entries.map((entry) => <EntryCard key={entry.runtime_id} entry={entry} onToggle={() => onToggle(entry)} />) : <p className="quiet">This page has no body entries yet.</p>}</section><section><h2>Incoming references</h2>{projection.incoming_entries.length ? projection.incoming_entries.map((entry) => <EntryCard key={entry.runtime_id} entry={entry} onToggle={() => onToggle(entry)} />) : <p className="quiet">No backlinks have landed here.</p>}</section></div>; }
function EntryCard({ entry, onToggle, hit }: { entry: Entry; onToggle: () => void; hit?: SearchHit }) { const degraded = /{{|}}|collapsed::|id::|SCHEDULED:|DEADLINE:/i.test(entry.text); const date = entry.anchor.file_path.match(/(\d{4}-\d{2}-\d{2})\.md/)?.[1]; return <article className={entry.task ? `entry task ${entry.task.toLowerCase()}` : 'entry'}>{entry.task && <button className="check" onClick={onToggle}>{entry.task === 'Done' ? '✓' : '□'}</button>}<div className="entry-meta">{date && <span>{dayFmt.format(new Date(`${date}T12:00:00`))}</span>}{hit && <span>score {hit.score}</span>}{degraded && <span className="degraded">degraded construct</span>}</div><p>{entry.text}</p><footer>{[...entry.tags, ...entry.links].map((ref) => <span key={`${entry.runtime_id}-${ref.raw}-${ref.page_path}`}>{ref.page_path}</span>)}<small title={`${entry.anchor.file_path} · ${entry.anchor.span.start}-${entry.anchor.span.end}`}>{entry.anchor.file_path} · {entry.anchor.span.start}-{entry.anchor.span.end}</small></footer></article>; }
function WarningStrip({ warnings }: { warnings: WorkspaceWarning[] }) { return <aside className="warning-strip"><strong>{warnings.length} workspace warning{warnings.length === 1 ? '' : 's'}</strong>{warnings.slice(0, 3).map((warning, index) => <span key={`${warning.message}${index}`}>{warning.kind ?? 'Warning'}: {warning.message}</span>)}</aside>; }
function Segmented<T extends string>({ value, options, onChange }: { value: T; options: T[]; onChange: (value: T) => void }) { return <div className="segmented">{options.map((option) => <button key={option} className={value === option ? 'active' : ''} onClick={() => onChange(option)}>{option}</button>)}</div>; }
function CommandPalette({ query, setQuery, items, close }: { query: string; setQuery: (query: string) => void; items: Array<{ label: string; detail: string; run: () => void }>; close: () => void }) { return <div className="overlay" onMouseDown={close}><section className="palette" onMouseDown={(event) => event.stopPropagation()}><input autoFocus value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Jump to a page, command, or surface…" />{items.slice(0, 14).map((item) => <button key={`${item.label}${item.detail}`} onClick={() => { item.run(); close(); }}><strong>{item.label}</strong><span>{item.detail}</span></button>)}</section></div>; }
function EmptyState() { return <div className="empty"><h2>Open a local folder to begin.</h2><p>Uniseq will create canonical journals, pages, assets, app, and disposable cache folders without rewriting your notes.</p></div>; }

createRoot(document.getElementById('root')!).render(<App />);
