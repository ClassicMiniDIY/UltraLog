//! Map tile providers + in-memory and on-disk caching.
//!
//! Two providers ship: Esri World Imagery (satellite) and OpenStreetMap
//! (raster street map). Tiles are fetched by a bounded background worker
//! pool; the UI thread requests `(provider, z, x, y)` and workers return PNG
//! bytes. Decoded PNGs are cached in memory as
//! `egui::TextureHandle` so subsequent frames don't re-decode.
//!
//! Disk cache lives at `{data_dir}/UltraLog/tiles/{provider}/{z}/{x}/{y}.png`,
//! mirroring the OECUA spec cache layout.
//!
//! Privacy / bandwidth note: the widget exposes a per-tab toggle and tiles
//! are off by default. Users opt in explicitly. Both providers' attribution
//! requirements are surfaced in the Track Map UI when the corresponding
//! provider is active.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;

use crate::state::TileProviderId;

const USER_AGENT: &str = concat!(
    "UltraLog/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/ClassicMiniDIY/UltraLog)"
);
const TILES_CACHE_DIR: &str = "tiles";
const DEFAULT_TILE_CACHE_MAX_MB: u32 = 256;
const MAX_TILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TILE_DIMENSION: u32 = 4096;
const MAX_TILE_PIXELS: u64 = 1024 * 1024;
const TILE_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const TILE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TILE_RETRY_DELAY: Duration = Duration::from_secs(10);
const TILE_QUEUE_RETRY_DELAY: Duration = Duration::from_millis(16);
const TILE_REQUEST_QUEUE_CAPACITY: usize = 64;
const TILE_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Source of map tiles. New providers add a `TileProviderId` variant in
/// `state.rs` and a match arm in `provider_for`.
pub trait TileProvider: Send + Sync {
    fn id(&self) -> TileProviderId;
    fn url(&self, z: u8, x: u32, y: u32) -> String;
    fn attribution(&self) -> &'static str;
    fn max_zoom(&self) -> u8;
    fn cache_subdir(&self) -> &'static str;
    /// Maximum simultaneous network fetches allowed against this provider.
    /// Disk-cache reads are not counted. Defaults to the full worker pool.
    fn max_concurrent_fetches(&self) -> usize {
        WORKER_COUNT
    }
}

pub struct EsriWorldImagery;
impl TileProvider for EsriWorldImagery {
    fn id(&self) -> TileProviderId {
        TileProviderId::EsriWorldImagery
    }
    fn url(&self, z: u8, x: u32, y: u32) -> String {
        format!(
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
        )
    }
    fn attribution(&self) -> &'static str {
        "Source: Esri, Maxar, Earthstar Geographics, and the GIS User Community"
    }
    fn max_zoom(&self) -> u8 {
        19
    }
    fn cache_subdir(&self) -> &'static str {
        "esri_world_imagery"
    }
}

pub struct OpenStreetMap;
impl TileProvider for OpenStreetMap {
    fn id(&self) -> TileProviderId {
        TileProviderId::OpenStreetMap
    }
    fn url(&self, z: u8, x: u32, y: u32) -> String {
        format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
    }
    fn attribution(&self) -> &'static str {
        "© OpenStreetMap contributors"
    }
    fn max_zoom(&self) -> u8 {
        19
    }
    fn cache_subdir(&self) -> &'static str {
        "openstreetmap"
    }
    /// The OSM tile usage policy caps applications at 2 simultaneous
    /// download connections (<https://operations.osmfoundation.org/policies/tiles/>).
    /// Exceeding it risks a per-IP block that would hit every UltraLog user.
    fn max_concurrent_fetches(&self) -> usize {
        2
    }
}

/// Resolve a [`TileProviderId`] to the corresponding static provider.
pub fn provider_for(id: TileProviderId) -> &'static dyn TileProvider {
    match id {
        TileProviderId::EsriWorldImagery => &EsriWorldImagery,
        TileProviderId::OpenStreetMap => &OpenStreetMap,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub provider: TileProviderId,
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug)]
struct FetchedTile {
    key: TileKey,
    /// `Some` on success, `None` if the fetch failed - the UI thread still
    /// needs to clear the in-flight marker so the request can be retried
    /// later (e.g. after the user pans away and back).
    bytes: Option<Vec<u8>>,
    /// A cancelled request was no longer visible when a worker dequeued it.
    /// It must not enter the retry backoff used for network failures.
    cancelled: bool,
}

#[derive(Debug)]
enum WorkerMsg {
    Fetch(TileKey),
}

/// Number of parallel HTTP workers. Tile servers have per-IP concurrency
/// limits; 4 is a polite default that still hides per-tile latency on
/// high-zoom panning. Providers with stricter published limits are gated
/// further by [`TileProvider::max_concurrent_fetches`] via [`FetchPermits`].
const WORKER_COUNT: usize = 4;

/// How long a worker sleeps between attempts to acquire a per-provider
/// network permit. Visibility and shutdown are re-checked on every attempt.
const TILE_PERMIT_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Counts in-flight network fetches per provider so the worker pool can
/// honor per-provider connection limits (OSM allows at most 2).
#[derive(Debug, Default)]
struct FetchPermits {
    in_flight: Mutex<HashMap<TileProviderId, usize>>,
}

impl FetchPermits {
    /// Try to reserve a fetch slot for `provider`. Returns `true` on
    /// success; the caller must balance it with [`Self::release`].
    fn try_acquire(&self, provider: TileProviderId) -> bool {
        let limit = provider_for(provider).max_concurrent_fetches().max(1);
        let mut in_flight = self.in_flight.lock().expect("fetch permits poisoned");
        let count = in_flight.entry(provider).or_insert(0);
        if *count < limit {
            *count += 1;
            true
        } else {
            false
        }
    }

    fn release(&self, provider: TileProviderId) {
        let mut in_flight = self.in_flight.lock().expect("fetch permits poisoned");
        if let Some(count) = in_flight.get_mut(&provider) {
            *count = count.saturating_sub(1);
        }
    }
}

/// Lazy singleton tile source. Owns the worker thread pool, the in-flight
/// set, the raw-bytes cache, and the on-GPU texture cache. UI thread uses
/// [`Self::request`] each frame for the tiles it wants to draw.
///
/// Decoding is split from fetching so the same tile can be re-decoded into
/// a different visual mode (e.g. greyscale) without re-fetching from disk
/// or network. Texture cache is keyed on `(TileKey, grayscale)`.
pub struct TileSource {
    tx: SyncSender<WorkerMsg>,
    rx: Mutex<Receiver<FetchedTile>>,
    in_flight: Mutex<HashSet<TileKey>>,
    visible_tiles: Arc<RwLock<HashSet<TileKey>>>,
    shutdown: Arc<AtomicBool>,
    failed_until: Mutex<HashMap<TileKey, Instant>>,
    disk_cache: Arc<DiskTileCache>,
    /// Raw PNG/JPEG bytes per tile, populated by the worker pool.
    /// Decoding into a `TextureHandle` happens lazily in [`Self::request`] so the
    /// same bytes can serve both colour and greyscale variants.
    bytes_cache: Mutex<HashMap<TileKey, Arc<Vec<u8>>>>,
    textures: Mutex<HashMap<TextureKey, egui::TextureHandle>>,
    /// Soft cap on resident textures; oldest entries get evicted en masse
    /// when exceeded. Simpler than tracking strict LRU for the v1 budget.
    capacity: usize,
    /// Soft cap on cached PNG/JPEG bytes - typically 10-30 KB each, so 512
    /// is well under 20 MB and lets several zooms stay warm.
    bytes_capacity: usize,
}

/// Key for the GPU texture cache. Includes the visual mode so a tile drawn
/// once in colour and once in greyscale produces two distinct textures.
type TextureKey = (TileKey, bool);

#[derive(Debug)]
struct DiskTileCache {
    root: Option<PathBuf>,
    max_bytes: u64,
    state: Mutex<DiskCacheState>,
}

#[derive(Debug, Default)]
struct DiskCacheState {
    initialized: bool,
    total_bytes: u64,
    next_access: u64,
    entries: HashMap<PathBuf, DiskCacheEntry>,
}

#[derive(Debug)]
struct DiskCacheEntry {
    size: u64,
    last_used: u64,
}

impl DiskTileCache {
    fn new(max_mb: u32) -> Self {
        Self::with_root(
            tiles_cache_root(),
            u64::from(max_mb).saturating_mul(1024 * 1024),
        )
    }

    fn with_root(root: Option<PathBuf>, max_bytes: u64) -> Self {
        Self {
            root,
            max_bytes,
            state: Mutex::new(DiskCacheState::default()),
        }
    }

    fn read(&self, key: TileKey) -> Option<Vec<u8>> {
        let path = self.path(key)?;
        let mut state = self.state.lock().expect("disk cache poisoned");
        self.ensure_index(&mut state);

        match fs::read(&path) {
            Ok(bytes) => {
                let size = bytes.len() as u64;
                let previous_size = state.entries.get(&path).map_or(0, |entry| entry.size);
                state.total_bytes = state
                    .total_bytes
                    .saturating_sub(previous_size)
                    .saturating_add(size);
                let last_used = next_access(&mut state);
                state
                    .entries
                    .insert(path, DiskCacheEntry { size, last_used });
                self.evict_to_fit(&mut state, 0);
                Some(bytes)
            }
            Err(_) => {
                remove_index_entry(&mut state, &path);
                None
            }
        }
    }

    fn write(&self, key: TileKey, bytes: &[u8]) {
        let size = bytes.len() as u64;
        if self.max_bytes == 0 || size > self.max_bytes {
            return;
        }
        let Some(path) = self.path(key) else {
            return;
        };

        let mut state = self.state.lock().expect("disk cache poisoned");
        self.ensure_index(&mut state);
        remove_index_entry(&mut state, &path);
        self.evict_to_fit(&mut state, size);

        let Some(parent) = path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }

        let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
        if fs::write(&temporary, bytes).is_err() {
            return;
        }
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        if fs::rename(&temporary, &path).is_err() {
            let _ = fs::remove_file(temporary);
            return;
        }

        let last_used = next_access(&mut state);
        state.total_bytes = state.total_bytes.saturating_add(size);
        state
            .entries
            .insert(path, DiskCacheEntry { size, last_used });
    }

    fn remove(&self, key: TileKey) {
        let Some(path) = self.path(key) else {
            return;
        };
        let mut state = self.state.lock().expect("disk cache poisoned");
        self.ensure_index(&mut state);
        let _ = fs::remove_file(&path);
        remove_index_entry(&mut state, &path);
    }

    fn path(&self, key: TileKey) -> Option<PathBuf> {
        self.root.as_ref().map(|root| tile_disk_path_in(root, key))
    }

    fn ensure_index(&self, state: &mut DiskCacheState) {
        if state.initialized {
            return;
        }
        state.initialized = true;
        let Some(root) = self.root.as_ref() else {
            return;
        };

        let mut files = Vec::new();
        collect_cache_files(root, &mut files);
        files.sort_by_key(|(_, _, modified)| *modified);
        for (path, size, _) in files {
            let last_used = next_access(state);
            state.total_bytes = state.total_bytes.saturating_add(size);
            state
                .entries
                .insert(path, DiskCacheEntry { size, last_used });
        }
        self.evict_to_fit(state, 0);
    }

    fn evict_to_fit(&self, state: &mut DiskCacheState, incoming: u64) {
        while state.total_bytes.saturating_add(incoming) > self.max_bytes {
            let Some(oldest) = state
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(path, _)| path.clone())
            else {
                break;
            };

            match fs::remove_file(&oldest) {
                Ok(()) => remove_index_entry(state, &oldest),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    remove_index_entry(state, &oldest);
                }
                Err(error) => {
                    tracing::warn!(
                        path = %oldest.display(),
                        error = %error,
                        "failed to evict map tile from disk cache"
                    );
                    break;
                }
            }
        }
    }
}

fn next_access(state: &mut DiskCacheState) -> u64 {
    state.next_access = state.next_access.wrapping_add(1);
    state.next_access
}

fn remove_index_entry(state: &mut DiskCacheState, path: &PathBuf) {
    if let Some(entry) = state.entries.remove(path) {
        state.total_bytes = state.total_bytes.saturating_sub(entry.size);
    }
}

fn collect_cache_files(directory: &std::path::Path, files: &mut Vec<(PathBuf, u64, SystemTime)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            collect_cache_files(&path, files);
        } else if metadata.is_file() {
            files.push((
                path,
                metadata.len(),
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            ));
        }
    }
}

impl TileSource {
    pub fn new(repaint_ctx: egui::Context, disk_cache_max_mb: u32) -> Self {
        let (req_tx, req_rx) = mpsc::sync_channel::<WorkerMsg>(TILE_REQUEST_QUEUE_CAPACITY);
        let (resp_tx, resp_rx) = mpsc::channel::<FetchedTile>();
        let disk_cache = Arc::new(DiskTileCache::new(disk_cache_max_mb));
        let http_agent = build_http_agent();
        let visible_tiles = Arc::new(RwLock::new(HashSet::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        // Wrap the request receiver in Arc<Mutex> so multiple workers can
        // share it (mpsc::Receiver is !Sync; Mutex makes it act like an MPMC
        // for our coarse Fetch granularity).
        let req_rx = Arc::new(Mutex::new(req_rx));
        let fetch_permits = Arc::new(FetchPermits::default());
        for i in 0..WORKER_COUNT {
            let rx_clone = Arc::clone(&req_rx);
            let tx_clone = resp_tx.clone();
            let cache_clone = Arc::clone(&disk_cache);
            let agent_clone = http_agent.clone();
            let ctx_clone = repaint_ctx.clone();
            let visible_clone = Arc::clone(&visible_tiles);
            let shutdown_clone = Arc::clone(&shutdown);
            let permits_clone = Arc::clone(&fetch_permits);
            thread::Builder::new()
                .name(format!("ultralog-tile-worker-{i}"))
                .spawn(move || {
                    worker_loop(
                        rx_clone,
                        tx_clone,
                        cache_clone,
                        agent_clone,
                        ctx_clone,
                        visible_clone,
                        shutdown_clone,
                        permits_clone,
                    )
                })
                .ok();
        }

        Self {
            tx: req_tx,
            rx: Mutex::new(resp_rx),
            in_flight: Mutex::new(HashSet::new()),
            visible_tiles,
            shutdown,
            failed_until: Mutex::new(HashMap::new()),
            disk_cache,
            bytes_cache: Mutex::new(HashMap::new()),
            textures: Mutex::new(HashMap::new()),
            capacity: 256,
            bytes_capacity: 512,
        }
    }

    /// Drain any tiles fetched since the last call into the bytes cache.
    /// Texture upload happens lazily in [`Self::request`] so callers can render
    /// the same tile in different visual modes (e.g. greyscale).
    pub fn poll(&self, ctx: &egui::Context) {
        let rx = self.rx.lock().expect("rx poisoned");
        let mut got_any = false;
        while let Ok(tile) = rx.try_recv() {
            let mut in_flight = self.in_flight.lock().expect("in_flight poisoned");
            in_flight.remove(&tile.key);
            drop(in_flight);

            if tile.cancelled {
                continue;
            }
            let Some(bytes) = tile.bytes else {
                self.failed_until
                    .lock()
                    .expect("failed_until poisoned")
                    .insert(tile.key, Instant::now() + TILE_RETRY_DELAY);
                continue;
            };
            self.failed_until
                .lock()
                .expect("failed_until poisoned")
                .remove(&tile.key);
            let mut bc = self.bytes_cache.lock().expect("bytes_cache poisoned");
            if bc.len() >= self.bytes_capacity {
                let drop_n = self.bytes_capacity / 2;
                let to_drop: Vec<TileKey> = bc.keys().copied().take(drop_n).collect();
                for k in to_drop {
                    bc.remove(&k);
                }
            }
            bc.insert(tile.key, Arc::new(bytes));
            got_any = true;
        }
        if got_any {
            ctx.request_repaint();
        }
    }

    /// Replace the current visible tile set. Workers consult this set before
    /// starting disk or network work, so queued requests from an older view
    /// are discarded without delaying the current map position.
    pub fn set_visible_tiles(&self, keys: &[TileKey]) {
        let mut visible = self
            .visible_tiles
            .write()
            .unwrap_or_else(|error| error.into_inner());
        visible.clear();
        visible.extend(keys.iter().copied());
    }

    /// Mark every queued request as obsolete.
    pub fn cancel_pending(&self) {
        self.set_visible_tiles(&[]);
    }

    /// Returns the texture for `(key, grayscale)` if it's already uploaded
    /// or the raw bytes are cached locally; otherwise returns `None` and
    /// kicks off a background fetch. Caller draws a placeholder for misses.
    pub fn request(
        &self,
        ctx: &egui::Context,
        key: TileKey,
        grayscale: bool,
    ) -> Option<egui::TextureHandle> {
        if !tile_is_visible(&self.visible_tiles, key) {
            return None;
        }
        let tex_key = (key, grayscale);
        {
            let textures = self.textures.lock().expect("textures poisoned");
            if let Some(t) = textures.get(&tex_key) {
                return Some(t.clone());
            }
        }

        let bytes_opt = {
            let bc = self.bytes_cache.lock().expect("bytes_cache poisoned");
            bc.get(&key).cloned()
        };
        if let Some(bytes) = bytes_opt {
            if let Some(image) = decode_image(&bytes, grayscale) {
                let name = format!(
                    "tile_{:?}_{}_{}_{}_{}",
                    key.provider,
                    key.z,
                    key.x,
                    key.y,
                    if grayscale { "g" } else { "c" }
                );
                let handle = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
                let mut textures = self.textures.lock().expect("textures poisoned");
                if textures.len() >= self.capacity {
                    let drop_n = self.capacity / 2;
                    let to_drop: Vec<TextureKey> = textures.keys().copied().take(drop_n).collect();
                    for k in to_drop {
                        textures.remove(&k);
                    }
                }
                textures.insert(tex_key, handle.clone());
                return Some(handle);
            }

            self.bytes_cache
                .lock()
                .expect("bytes_cache poisoned")
                .remove(&key);
            self.disk_cache.remove(key);
        }

        let retry_after = self
            .failed_until
            .lock()
            .expect("failed_until poisoned")
            .get(&key)
            .copied();
        if let Some(until) = retry_after {
            let now = Instant::now();
            if now < until {
                ctx.request_repaint_after(until - now);
                return None;
            }
            self.failed_until
                .lock()
                .expect("failed_until poisoned")
                .remove(&key);
        }

        let mut in_flight = self.in_flight.lock().expect("in_flight poisoned");
        if !in_flight.contains(&key) {
            in_flight.insert(key);
            match self.tx.try_send(WorkerMsg::Fetch(key)) {
                Ok(()) => {}
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                    in_flight.remove(&key);
                    ctx.request_repaint_after(TILE_QUEUE_RETRY_DELAY);
                }
            }
        }
        None
    }

    /// Stop the worker thread. Currently unused (workers live for the app
    /// lifetime) but exposed for tests / future settings UI.
    pub fn shutdown(&self) {
        self.cancel_pending();
        self.shutdown.store(true, Ordering::Release);
    }
}

impl Default for TileSource {
    fn default() -> Self {
        Self::new(egui::Context::default(), DEFAULT_TILE_CACHE_MAX_MB)
    }
}

#[allow(clippy::too_many_arguments)]
fn worker_loop(
    rx: Arc<Mutex<Receiver<WorkerMsg>>>,
    resp_tx: Sender<FetchedTile>,
    disk_cache: Arc<DiskTileCache>,
    http_agent: ureq::Agent,
    repaint_ctx: egui::Context,
    visible_tiles: Arc<RwLock<HashSet<TileKey>>>,
    shutdown: Arc<AtomicBool>,
    fetch_permits: Arc<FetchPermits>,
) {
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let msg = {
            // Brief lock to pop the next message; release before doing the
            // network/disk work so siblings can pull in parallel.
            //
            // Holding the mutex across `recv_timeout` does serialize the
            // *waiting* (one worker blocks in recv, siblings block on the
            // mutex), but not anything that matters: the producer side
            // (`SyncSender::try_send` on the UI thread) never touches this
            // mutex, the waiter wakes immediately when a message arrives,
            // and the lock is released before the fetch begins - so the
            // per-dequeue handoff is microseconds against the hundreds of
            // milliseconds each fetch takes. A true MPMC channel would buy
            // nothing here at the cost of a new dependency.
            let guard = rx.lock().expect("rx poisoned");
            guard.recv_timeout(TILE_WORKER_POLL_INTERVAL)
        };
        let msg = match msg {
            Ok(msg) => msg,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match msg {
            WorkerMsg::Fetch(key) => {
                if !tile_is_visible(&visible_tiles, key) {
                    if resp_tx
                        .send(FetchedTile {
                            key,
                            bytes: None,
                            cancelled: true,
                        })
                        .is_err()
                    {
                        break;
                    }
                    repaint_ctx.request_repaint();
                    continue;
                }
                let outcome = load_or_fetch(
                    key,
                    &disk_cache,
                    &http_agent,
                    &fetch_permits,
                    &visible_tiles,
                    &shutdown,
                );
                // Always send back a FetchedTile (Some/None) so the UI can
                // clear in_flight even on failure - otherwise a dropped tile
                // would never be retried until restart.
                let tile = match outcome {
                    FetchOutcome::Shutdown => break,
                    FetchOutcome::Bytes(bytes) => FetchedTile {
                        key,
                        bytes: Some(bytes),
                        cancelled: false,
                    },
                    FetchOutcome::Failed => FetchedTile {
                        key,
                        bytes: None,
                        cancelled: false,
                    },
                    FetchOutcome::Cancelled => FetchedTile {
                        key,
                        bytes: None,
                        cancelled: true,
                    },
                };
                if resp_tx.send(tile).is_err() {
                    break;
                }
                repaint_ctx.request_repaint();
            }
        }
    }
}

fn tile_is_visible(visible_tiles: &RwLock<HashSet<TileKey>>, key: TileKey) -> bool {
    visible_tiles
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .contains(&key)
}

/// Result of one tile load attempt, distinguishing polite bail-outs from
/// real failures so only the latter enter the retry backoff.
enum FetchOutcome {
    Bytes(Vec<u8>),
    Failed,
    Cancelled,
    Shutdown,
}

fn load_or_fetch(
    key: TileKey,
    disk_cache: &DiskTileCache,
    http_agent: &ureq::Agent,
    fetch_permits: &FetchPermits,
    visible_tiles: &RwLock<HashSet<TileKey>>,
    shutdown: &AtomicBool,
) -> FetchOutcome {
    if let Some(bytes) = disk_cache.read(key) {
        if valid_tile_bytes(&bytes) {
            return FetchOutcome::Bytes(bytes);
        }
        disk_cache.remove(key);
    }

    // Wait for a per-provider network permit. Visibility and shutdown are
    // re-checked while waiting so a queued tile that scrolled out of view
    // never consumes a connection slot.
    while !fetch_permits.try_acquire(key.provider) {
        if shutdown.load(Ordering::Acquire) {
            return FetchOutcome::Shutdown;
        }
        if !tile_is_visible(visible_tiles, key) {
            return FetchOutcome::Cancelled;
        }
        thread::sleep(TILE_PERMIT_POLL_INTERVAL);
    }

    let provider = provider_for(key.provider);
    let url = provider.url(key.z, key.x, key.y);
    let bytes = http_get(http_agent, &url);
    fetch_permits.release(key.provider);

    let Some(bytes) = bytes else {
        return FetchOutcome::Failed;
    };
    if !valid_tile_bytes(&bytes) {
        return FetchOutcome::Failed;
    }
    disk_cache.write(key, &bytes);
    FetchOutcome::Bytes(bytes)
}

fn build_http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TILE_REQUEST_TIMEOUT))
        .timeout_connect(Some(TILE_CONNECT_TIMEOUT))
        .timeout_recv_body(Some(TILE_REQUEST_TIMEOUT))
        .build()
        .into()
}

fn http_get(agent: &ureq::Agent, url: &str) -> Option<Vec<u8>> {
    let mut response = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_TILE_BYTES)
        .read_to_vec()
        .ok()
}

fn valid_tile_bytes(bytes: &[u8]) -> bool {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes));
    let Ok(reader) = reader.with_guessed_format() else {
        return false;
    };
    let dimensions_are_valid = reader
        .into_dimensions()
        .is_ok_and(|(width, height)| valid_tile_dimensions(width, height));
    dimensions_are_valid && image::load_from_memory(bytes).is_ok()
}

fn valid_tile_dimensions(width: u32, height: u32) -> bool {
    width > 0
        && height > 0
        && width <= MAX_TILE_DIMENSION
        && height <= MAX_TILE_DIMENSION
        && u64::from(width) * u64::from(height) <= MAX_TILE_PIXELS
}

/// Decode tile bytes (PNG or JPEG, sniffed by `image`) into an
/// `egui::ColorImage`. When `grayscale` is set, RGB is collapsed to luma
/// using Rec. 601 weights so the texture goes monochrome at upload time.
fn decode_image(bytes: &[u8], grayscale: bool) -> Option<egui::ColorImage> {
    let mut img = image::load_from_memory(bytes).ok()?.to_rgba8();
    if grayscale {
        for px in img.pixels_mut() {
            let y = (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32)
                .round()
                .clamp(0.0, 255.0) as u8;
            px[0] = y;
            px[1] = y;
            px[2] = y;
        }
    }
    let (w, h) = img.dimensions();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        img.as_raw(),
    ))
}

/// Tile cache root: `{data_dir}/UltraLog/tiles/`.
pub fn tiles_cache_root() -> Option<PathBuf> {
    dirs::data_dir().map(|base| base.join("UltraLog").join(TILES_CACHE_DIR))
}

fn tile_disk_path_in(root: &std::path::Path, key: TileKey) -> PathBuf {
    let provider = provider_for(key.provider);
    root.join(provider.cache_subdir())
        .join(key.z.to_string())
        .join(key.x.to_string())
        .join(format!("{}.png", key.y))
}

// ============================================================================
// Web Mercator math
// ============================================================================

/// Convert a lat/lon (degrees) to fractional tile coordinates at zoom `z`.
pub fn lonlat_to_tile_xy(lon_deg: f64, lat_deg: f64, z: u8) -> (f64, f64) {
    let n = 2f64.powi(z as i32);
    let x = (lon_deg + 180.0) / 360.0 * n;
    let lat_rad = lat_deg.clamp(-85.051_128_78, 85.051_128_78).to_radians();
    let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    (x, y)
}

/// Convert lat/lon (degrees) to absolute Mercator pixel coordinates at zoom
/// `z`. One tile = 256 px.
pub fn lonlat_to_pixel(lon_deg: f64, lat_deg: f64, z: u8) -> (f64, f64) {
    let (tx, ty) = lonlat_to_tile_xy(lon_deg, lat_deg, z);
    (tx * 256.0, ty * 256.0)
}

/// Pick the largest zoom level at which a lon/lat bbox of `(west, east,
/// south, north)` fits within `screen_size` pixels (with a 10% margin).
pub fn fit_zoom(
    west: f64,
    east: f64,
    south: f64,
    north: f64,
    screen_size: egui::Vec2,
    max_zoom: u8,
) -> u8 {
    let target_w = (screen_size.x as f64 * 0.9).max(64.0);
    let target_h = (screen_size.y as f64 * 0.9).max(64.0);
    for z in (1..=max_zoom).rev() {
        let (x0, y0) = lonlat_to_pixel(west, north, z);
        let (x1, y1) = lonlat_to_pixel(east, south, z);
        let w = (x1 - x0).abs();
        let h = (y1 - y0).abs();
        if w <= target_w && h <= target_h {
            return z;
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lonlat_zero_is_origin_at_zoom_0() {
        let (x, y) = lonlat_to_tile_xy(0.0, 0.0, 0);
        assert!((x - 0.5).abs() < 1e-9);
        assert!((y - 0.5).abs() < 1e-9);
    }

    #[test]
    fn lonlat_round_trip_at_zoom_10() {
        let (px, _py) = lonlat_to_pixel(9.0, 45.0, 10);
        // At zoom 10 the world is 256 * 2^10 = 262144 pixels wide. lon 9°
        // sits at (9 + 180)/360 * 262144 ~= 137625.6.
        let expected = ((9.0 + 180.0) / 360.0) * 256.0 * (1u32 << 10) as f64;
        assert!((px - expected).abs() < 1e-3);
    }

    #[test]
    fn mercator_projection_clamps_polar_latitudes() {
        let (_, north) = lonlat_to_tile_xy(0.0, 90.0, 10);
        let (_, south) = lonlat_to_tile_xy(0.0, -90.0, 10);
        assert!(north.is_finite());
        assert!(south.is_finite());
        assert!(north >= -1e-7);
        assert!(south <= 1024.0 + 1e-7);
    }

    #[test]
    fn fit_zoom_picks_higher_for_smaller_bbox() {
        let small = fit_zoom(9.000, 9.001, 44.999, 45.000, egui::vec2(400.0, 400.0), 19);
        let big = fit_zoom(0.0, 90.0, 0.0, 60.0, egui::vec2(400.0, 400.0), 19);
        assert!(
            small > big,
            "small bbox should fit at higher zoom: {small} vs {big}"
        );
    }

    #[test]
    fn provider_for_returns_distinct_subdirs() {
        let a = provider_for(TileProviderId::EsriWorldImagery).cache_subdir();
        let b = provider_for(TileProviderId::OpenStreetMap).cache_subdir();
        assert_ne!(a, b);
    }

    #[test]
    fn tile_validation_rejects_non_images() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        assert!(valid_tile_bytes(png.get_ref()));
        assert!(!valid_tile_bytes(b"not an image"));
    }

    #[test]
    fn tile_validation_limits_decoded_pixel_count() {
        assert!(valid_tile_dimensions(256, 256));
        assert!(valid_tile_dimensions(1024, 1024));
        assert!(!valid_tile_dimensions(4096, 4096));
        assert!(!valid_tile_dimensions(0, 256));
    }

    #[test]
    fn bounded_request_queue_rejects_excess_work() {
        let (tx, _rx) = mpsc::sync_channel(TILE_REQUEST_QUEUE_CAPACITY);
        let key = TileKey {
            provider: TileProviderId::OpenStreetMap,
            z: 1,
            x: 0,
            y: 0,
        };
        for _ in 0..TILE_REQUEST_QUEUE_CAPACITY {
            tx.try_send(WorkerMsg::Fetch(key)).unwrap();
        }

        assert!(matches!(
            tx.try_send(WorkerMsg::Fetch(key)),
            Err(TrySendError::Full(_))
        ));
    }

    #[test]
    fn fetch_permits_enforce_the_osm_connection_limit() {
        let permits = FetchPermits::default();

        assert!(permits.try_acquire(TileProviderId::OpenStreetMap));
        assert!(permits.try_acquire(TileProviderId::OpenStreetMap));
        assert!(
            !permits.try_acquire(TileProviderId::OpenStreetMap),
            "OSM allows at most 2 simultaneous fetches"
        );

        // A saturated OSM pool must not block other providers.
        assert!(permits.try_acquire(TileProviderId::EsriWorldImagery));

        permits.release(TileProviderId::OpenStreetMap);
        assert!(permits.try_acquire(TileProviderId::OpenStreetMap));
    }

    #[test]
    fn worker_discards_tile_that_left_the_visible_view() {
        let (req_tx, req_rx) = mpsc::sync_channel(1);
        let (resp_tx, resp_rx) = mpsc::channel();
        let visible_tiles = Arc::new(RwLock::new(HashSet::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let key = TileKey {
            provider: TileProviderId::OpenStreetMap,
            z: 1,
            x: 0,
            y: 0,
        };
        req_tx.try_send(WorkerMsg::Fetch(key)).unwrap();

        let worker_visible = Arc::clone(&visible_tiles);
        let worker_shutdown = Arc::clone(&shutdown);
        let worker = thread::spawn(move || {
            worker_loop(
                Arc::new(Mutex::new(req_rx)),
                resp_tx,
                Arc::new(DiskTileCache::with_root(None, 0)),
                build_http_agent(),
                egui::Context::default(),
                worker_visible,
                worker_shutdown,
                Arc::new(FetchPermits::default()),
            );
        });

        let response = resp_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(response.key, key);
        assert!(response.cancelled);
        assert!(response.bytes.is_none());

        shutdown.store(true, Ordering::Release);
        worker.join().unwrap();
    }

    #[test]
    fn disk_cache_evicts_oldest_file_to_respect_limit() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ultralog-tile-cache-test-{}-{unique}",
            std::process::id()
        ));
        let cache = DiskTileCache::with_root(Some(root.clone()), 6);
        let first = TileKey {
            provider: TileProviderId::OpenStreetMap,
            z: 1,
            x: 0,
            y: 0,
        };
        let second = TileKey { x: 1, ..first };

        cache.write(first, &[1, 2, 3, 4]);
        cache.write(second, &[5, 6, 7, 8]);

        let state = cache.state.lock().unwrap();
        assert!(state.total_bytes <= 6);
        assert_eq!(state.entries.len(), 1);
        assert!(state.entries.contains_key(&cache.path(second).unwrap()));
        drop(state);

        let _ = fs::remove_dir_all(root);
    }
}
