//! wgpu device/surface ownership. All raw GPU access lives behind this layer
//! (ADR-0001): scene code sees wgpu types, never a backend.

// Hot-path panic-denial pragma (Plan 0002 Phase 2). GPU bring-up returns
// Result; the render path must not panic.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use wgpu::{CreateSurfaceError, RequestAdapterError, RequestDeviceError, SurfaceTarget};

use crate::audio::FormatError;

/// Offscreen texture format for the headless capture path (Plan 0013). A tight
/// 8-bit RGBA the readback strips straight into a [`crate::render::CaptureImage`].
pub(crate) const HEADLESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Something went wrong bringing up or drawing with the GPU context.
#[derive(Debug)]
pub enum RenderError {
    /// Creating the wgpu surface for the window failed.
    CreateSurface(CreateSurfaceError),
    /// No GPU adapter compatible with the surface was found.
    RequestAdapter(RequestAdapterError),
    /// Requesting a logical device from the adapter failed.
    RequestDevice(RequestDeviceError),
    /// The surface reported no supported configuration on this adapter.
    UnsupportedSurface,
    /// Frames are being produced at a texture format whose channel order has no
    /// name a frame consumer knows.
    ///
    /// Refused rather than published under a guessed name: a consumer reading
    /// four bytes per pixel and told the wrong order draws the right picture in
    /// the wrong colours, which looks like an authoring mistake rather than a
    /// protocol one (ADR-0187).
    UnnameablePixelOrder(wgpu::TextureFormat),
    /// Acquiring the frame raised a validation error — a bug, not a
    /// recoverable surface state.
    SurfaceValidation,
    /// A headless capture failed to map or read back its offscreen buffer
    /// (Plan 0013 tooling path — never the live render path).
    CaptureReadback,
    /// A capture requested a preset name not in the loaded roster (Plan 0013).
    UnknownPreset(String),
    /// An audio-driven capture was handed a PCM format the analyzer rejected at
    /// the intake boundary (Plan 0013).
    AudioFormat(FormatError),
    /// A requested graphics adapter is not on this machine. Carries the roster
    /// so the message can name what is available rather than an index nobody
    /// can interpret (ADR-0146).
    NoSuchAdapter {
        /// What the caller asked for, as they wrote it.
        requested: String,
        /// Every adapter this machine enumerates, described.
        available: Vec<String>,
    },
    /// A requested adapter name matched more than one adapter. A substring
    /// cannot separate two adapters whose descriptions share it, so the caller
    /// is told which ones collided rather than handed an arbitrary pick.
    AmbiguousAdapter {
        /// What the caller asked for, as they wrote it.
        requested: String,
        /// The adapters the request matched.
        matched: Vec<String>,
    },
    /// A named or indexed adapter exists on this machine but cannot present to
    /// the window that asked for it.
    ///
    /// Only reachable on the surface path and only for the enumerated variants:
    /// the preference variants hand the surface to wgpu as `compatible_surface`
    /// and so cannot select an adapter that fails this, while `Named`/`Index`
    /// pick out of the unfiltered roster. Refusing is the point — falling back
    /// to a working adapter would render on a GPU the operator did not ask for
    /// and say nothing, which is the failure `--gpu` exists to end (ADR-0155).
    AdapterCannotPresent {
        /// What the caller asked for, as they wrote it.
        requested: String,
        /// The adapter that request resolved to, described.
        adapter: String,
    },
    /// The consumer of a **streamed** capture refused a frame — the offline
    /// render mode's pipe closed, the encoder died, the file could not be
    /// written (Plan 0101 / ADR-0114). The string is the consumer's own message,
    /// carried verbatim rather than flattened to "write failed": that consumer
    /// is a child process, and a mystery broken pipe is the obvious way this
    /// path goes wrong.
    Sink(String),
    /// An adapter change was asked of a context that has no surface — the
    /// headless capture path, which renders on the adapter it was built on
    /// for the life of the run (ADR-0246). Refused by name rather than
    /// ignored, so a caller cannot take a switch that never happened for one
    /// that did; [`adapter_change_permitted`] is the condition.
    Headless,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::CreateSurface(e) => write!(f, "surface creation failed: {e}"),
            RenderError::RequestAdapter(e) => write!(f, "no suitable GPU adapter: {e}"),
            RenderError::RequestDevice(e) => write!(f, "device request failed: {e}"),
            RenderError::UnsupportedSurface => write!(f, "surface has no supported config"),
            RenderError::UnnameablePixelOrder(format) => write!(
                f,
                "frames are produced at {format:?}, whose channel order has no \
                 name a frame consumer knows (expected an 8-bit RGBA or BGRA \
                 format)"
            ),
            RenderError::SurfaceValidation => {
                write!(f, "surface texture acquisition failed validation")
            }
            RenderError::CaptureReadback => {
                write!(f, "headless capture readback failed")
            }
            RenderError::UnknownPreset(name) => {
                write!(f, "no preset named '{name}' in the roster")
            }
            RenderError::AudioFormat(e) => write!(f, "invalid audio format for capture: {e}"),
            RenderError::NoSuchAdapter {
                requested,
                available,
            } => write!(
                f,
                "no graphics adapter matching '{requested}'; this machine has: {}",
                available.join("; ")
            ),
            RenderError::AmbiguousAdapter { requested, matched } => write!(
                f,
                "'{requested}' matches {} adapters: {}",
                matched.len(),
                matched.join("; ")
            ),
            RenderError::AdapterCannotPresent { requested, adapter } => write!(
                f,
                "'{requested}' resolves to {adapter}, which cannot draw into this window; \
                 pick an adapter that can drive the display this window is on"
            ),
            RenderError::Sink(msg) => write!(f, "{msg}"),
            RenderError::Headless => write!(
                f,
                "this renderer has no window surface, so it cannot change adapter; a \
                 headless capture renders on the adapter it was built on"
            ),
        }
    }
}

/// **Whether a runtime adapter change is allowed at all** (ADR-0246): only on
/// a context that has a surface.
///
/// The same guard [`super::tier::tier_change_permitted`] gives the tier, and
/// for the same reason: a surface-less context is the headless capture path,
/// where the adapter is part of what makes a capture a pure function of its
/// inputs (NFR 6) — a baseline is blessed on one rasterizer, and a public
/// mutator that could move a capture onto another mid-run would be the hole in
/// that guarantee. Expressed as a value rather than a branch so both
/// directions are assertable: a `Renderer` **with** a surface cannot be built
/// in CI, and a test that only observed the headless refusal would pass
/// against a `set_adapter` that did nothing at all.
pub fn adapter_change_permitted(has_surface: bool) -> bool {
    has_surface
}

impl std::error::Error for RenderError {}

/// One line naming a GPU and its driver, for an ADR-0071 report.
///
/// Every field is taken verbatim from wgpu rather than interpreted; a report
/// that paraphrases its machine is worse than one that quotes it. Empty fields
/// are dropped so a backend that reports no driver string does not print an
/// empty pair of parentheses.
fn describe_adapter(info: &wgpu::AdapterInfo) -> String {
    let mut out = if info.name.is_empty() {
        "unnamed adapter".to_string()
    } else {
        info.name.clone()
    };
    out.push_str(&format!(" ({:?}, {:?})", info.backend, info.device_type));
    if !info.driver.is_empty() {
        out.push_str(&format!(", driver {}", info.driver));
    }
    if !info.driver_info.is_empty() {
        out.push_str(&format!(" {}", info.driver_info));
    }
    out
}

/// The device features to ask `adapter` for: the ones the engine can **use**
/// where it offers them, and nothing it cannot run without.
///
/// wgpu grants a device exactly the requested set, so a feature not named here
/// is unavailable even on hardware that has it — and a feature named here that
/// the adapter lacks makes `request_device` fail outright. Intersecting with
/// the adapter's own set is what makes this a capability query rather than a
/// requirement.
///
/// **`TIMESTAMP_QUERY` is the whole list**, and it buys per-pass GPU timings
/// for a tapped run ([`PassTimer`](super::gpu::PassTimer)). Pass-boundary
/// writes only, so `TIMESTAMP_QUERY_INSIDE_PASSES` — which tile-based GPUs
/// generally do not have — is deliberately not asked for. An adapter without
/// even this one (the software rasterizers) renders exactly as before and
/// reports no table.
fn optional_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features() & wgpu::Features::TIMESTAMP_QUERY
}

/// Which graphics adapter a headless context should render on.
///
/// Stated in wgpu's own vocabulary and nothing else: no platform type, no
/// vendor branch, no backend branch, so `core` stays GPU-abstract (ADR-0001)
/// while still letting a shell say which GPU it means.
///
/// **The variants are not interchangeable views of one preference.** The first
/// three ask wgpu to choose and accept whatever it returns; the last two name
/// one adapter out of the enumerated roster and fail if it is not there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AdapterChoice {
    /// Whatever wgpu picks with default options. On a hybrid machine this is
    /// the power-saving GPU for a console process, which is why a live path
    /// wants `HighPerformance` instead.
    ///
    /// The `Default` **impl** resolves here, which is what keeps every caller
    /// that does not care — the C ABI window path included — asking for exactly
    /// what it asked for before the choice existed.
    #[default]
    Default,
    /// Force a fallback (software) adapter - WARP on DX12 - so captures
    /// rasterize identically across machines. What the golden suite asks for.
    Software,
    /// `PowerPreference::HighPerformance`: the discrete GPU on a hybrid
    /// machine.
    HighPerformance,
    /// The one enumerated adapter whose name contains this string, matched
    /// case-insensitively — an adapter whose whole name **equals** it wins
    /// over any that merely contain it, so a full name read back from the
    /// roster resolves to exactly that adapter even where it is a prefix of
    /// another's. More than one match is an error, not a pick.
    Named(String),
    /// The adapter at this position in [`list_adapters`]'s roster.
    Index(usize),
}

impl From<bool> for AdapterChoice {
    /// The `prefer_software` bool every capture path already passes.
    fn from(prefer_software: bool) -> Self {
        if prefer_software {
            AdapterChoice::Software
        } else {
            AdapterChoice::Default
        }
    }
}

/// One enumerated adapter: the name a caller matches against, and the full
/// description a caller prints.
///
/// Two fields because the two jobs want different strings. `name` is wgpu's
/// bare `AdapterInfo::name`, which is what a substring is matched against and
/// what a DXGI `Description` is expected to equal; `detail` adds backend,
/// device type and driver, which help a reader choose and would wreck a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterDescription {
    /// wgpu's bare adapter name - the match key.
    pub name: String,
    /// Name plus backend, device type and driver, for printing.
    pub detail: String,
}

/// Every graphics adapter wgpu enumerates on this machine, in wgpu's order.
///
/// The order is the enumeration's own and is **not** promised to agree with any
/// other API's roster; a caller that needs one adapter across two APIs matches
/// by name on each side rather than by shared index (ADR-0146).
pub fn list_adapters() -> Vec<AdapterDescription> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    describe_roster(&instance)
}

fn describe_roster(instance: &wgpu::Instance) -> Vec<AdapterDescription> {
    pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .iter()
        .map(|adapter| {
            let info = adapter.get_info();
            AdapterDescription {
                name: info.name.clone(),
                detail: describe_adapter(&info),
            }
        })
        .collect()
}

/// Resolve a choice to one adapter, or say why it could not be.
///
/// `surface`, when present, is the window this adapter has to be able to draw
/// into, and it constrains the two kinds of variant differently. The preference
/// variants pass it to wgpu as `compatible_surface`, so wgpu picks only from
/// adapters that can present to it. The enumerated variants — `Named` and
/// `Index` — name one adapter out of the whole roster, which wgpu has not
/// filtered, so the surface check is ours to make: an operator can name the one
/// adapter that cannot drive their window, and the honest answer is a refusal
/// that says so rather than a silent fall-back to a different GPU (ADR-0155).
fn resolve_adapter(
    instance: &wgpu::Instance,
    choice: &AdapterChoice,
    surface: Option<&wgpu::Surface<'static>>,
) -> Result<wgpu::Adapter, RenderError> {
    let by_preference = |options: wgpu::RequestAdapterOptions<'_, '_>| {
        pollster::block_on(instance.request_adapter(&options)).map_err(RenderError::RequestAdapter)
    };
    // Every enumerated pick goes through here, so the surface constraint cannot
    // be honoured on one variant and forgotten on the other.
    let presenting = |adapter: wgpu::Adapter, requested: &str| match surface {
        Some(surface) if !adapter.is_surface_supported(surface) => {
            Err(RenderError::AdapterCannotPresent {
                requested: requested.to_owned(),
                adapter: describe_adapter(&adapter.get_info()),
            })
        }
        _ => Ok(adapter),
    };
    match choice {
        AdapterChoice::Default => by_preference(wgpu::RequestAdapterOptions {
            compatible_surface: surface,
            ..Default::default()
        }),
        AdapterChoice::Software => by_preference(wgpu::RequestAdapterOptions {
            force_fallback_adapter: true,
            compatible_surface: surface,
            ..Default::default()
        }),
        AdapterChoice::HighPerformance => by_preference(wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            ..Default::default()
        }),
        AdapterChoice::Index(wanted) => {
            let roster = describe_roster(instance);
            let picked = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                .into_iter()
                .nth(*wanted)
                .ok_or_else(|| RenderError::NoSuchAdapter {
                    requested: format!("index {wanted}"),
                    available: roster.into_iter().map(|entry| entry.detail).collect(),
                })?;
            presenting(picked, &format!("index {wanted}"))
        }
        AdapterChoice::Named(wanted) => {
            let needle = wanted.to_lowercase();
            let roster = describe_roster(instance);
            // Exact before containment: a name written back from this roster
            // — by the settings row into `[output] gpu` — must select the
            // adapter it was read from, and a substring rule alone would call
            // `RTX 3080` ambiguous beside `RTX 3080 Ti`.
            let exact: Vec<usize> = roster
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.name.to_lowercase() == needle)
                .map(|(at, _)| at)
                .collect();
            let hits: Vec<usize> = if exact.len() == 1 {
                exact
            } else {
                roster
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| entry.name.to_lowercase().contains(&needle))
                    .map(|(at, _)| at)
                    .collect()
            };
            match hits.as_slice() {
                [] => Err(RenderError::NoSuchAdapter {
                    requested: wanted.clone(),
                    available: roster.into_iter().map(|entry| entry.detail).collect(),
                }),
                [only] => {
                    let picked =
                        pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                            .into_iter()
                            .nth(*only)
                            .ok_or_else(|| RenderError::NoSuchAdapter {
                                requested: wanted.clone(),
                                available: roster
                                    .iter()
                                    .map(|entry| entry.detail.clone())
                                    .collect(),
                            })?;
                    presenting(picked, wanted)
                }
                several => Err(RenderError::AmbiguousAdapter {
                    requested: wanted.clone(),
                    matched: several
                        .iter()
                        .filter_map(|at| roster.get(*at).map(|entry| entry.detail.clone()))
                        .collect(),
                }),
            }
        }
    }
}

/// The kind of adapter a context runs on, as far as the renderer's own
/// decisions care (ADR-0245).
///
/// Crate-local on purpose: it feeds one table (`tier::grid_scale_for`) and no
/// scene ever branches on it, so a scene cannot grow a per-GPU look. Everything
/// wgpu names beyond these four — a virtual GPU, an unknown type — is `Other`,
/// which the table treats as it treats a discrete part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdapterClass {
    /// A GPU sharing system memory with the CPU — the bandwidth-bound case the
    /// grid scale exists for.
    Integrated,
    /// A GPU with its own memory.
    Discrete,
    /// A CPU rasterizer (WARP, llvmpipe) — what every golden capture runs on.
    Software,
    /// Anything wgpu reports that is none of the above.
    Other,
}

impl AdapterClass {
    /// The class of an adapter wgpu describes as `device_type`.
    pub(crate) fn of(device_type: wgpu::DeviceType) -> Self {
        match device_type {
            wgpu::DeviceType::IntegratedGpu => AdapterClass::Integrated,
            wgpu::DeviceType::DiscreteGpu => AdapterClass::Discrete,
            wgpu::DeviceType::Cpu => AdapterClass::Software,
            wgpu::DeviceType::VirtualGpu | wgpu::DeviceType::Other => AdapterClass::Other,
        }
    }
}

/// Owns the wgpu instance, surface, device, and queue for one output window.
///
/// `surface` is `None` for a **headless** context (Plan 0013): a device+queue
/// with no swapchain, drawing into offscreen capture textures. The on-surface
/// present path always has `Some`; `config` still carries the render size and
/// format for both paths.
pub struct RenderContext {
    pub(crate) surface: Option<wgpu::Surface<'static>>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    /// The instance the primary surface came from, and the adapter the device
    /// was requested on. Both are retained solely so a *secondary* surface can
    /// be created later on this same device (ADR-0143): a surface is only
    /// usable with a device whose adapter came from the same instance, and
    /// `get_default_config` needs the adapter to negotiate a format. Retaining
    /// them costs two handles and no GPU memory.
    ///
    /// Both are `Some` on the on-surface path. The headless path leaves them
    /// filled too — it has both in hand — but nothing there attaches an
    /// auxiliary surface.
    pub(crate) instance: wgpu::Instance,
    pub(crate) gpu: wgpu::Adapter,
    /// Whether the selected adapter is a CPU/software rasterizer (WARP on DX12,
    /// llvmpipe on Vulkan). The headless capture path forces this for
    /// reproducibility; visual-QA tests read it to skip checks the software
    /// rasterizer can't render faithfully (e.g. fullscreen-scene + background
    /// pipeline coexistence, a documented WARP quirk).
    is_software: bool,
    /// What kind of adapter this is, for the one decision that reads it: the
    /// internal-grid scale (ADR-0245). Set beside [`is_software`](Self::is_software)
    /// from the same `device_type`, so the two cannot disagree about a CPU
    /// rasterizer.
    class: AdapterClass,
    /// The selected adapter's own description — name, backend, device type and
    /// driver — kept as a formatted string rather than as `wgpu::AdapterInfo` so
    /// no consumer has to name a wgpu type to read it.
    ///
    /// **This exists for `ADR-0071` reports, and only for them.** A frame time is
    /// a fact about a GPU and a driver rather than about the code, so a test that
    /// prints one has to be able to say which GPU and which driver; before Plan
    /// 0113 Phase 2 nothing in the crate could. Nothing on a render path reads
    /// it.
    adapter: String,
}

impl RenderContext {
    /// Create a context rendering into `target` (any window-handle provider —
    /// the core never sees the windowing library behind it).
    pub fn new(
        target: impl Into<SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        adapter: &AdapterChoice,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(target)
            .map_err(RenderError::CreateSurface)?;
        Self::from_surface(&instance, surface, width, height, adapter)
    }

    /// Context from raw display/window handles — the C ABI path, where the
    /// host (e.g. the foobar2000 shim) owns the window.
    ///
    /// # Safety
    /// The handles must be valid and the window must outlive this context.
    pub unsafe fn new_unsafe(
        target: wgpu::SurfaceTargetUnsafe,
        width: u32,
        height: u32,
        adapter: &AdapterChoice,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = unsafe { instance.create_surface_unsafe(target) }
            .map_err(RenderError::CreateSurface)?;
        Self::from_surface(&instance, surface, width, height, adapter)
    }

    fn from_surface(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        choice: &AdapterChoice,
    ) -> Result<Self, RenderError> {
        let adapter = resolve_adapter(instance, choice, Some(&surface))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rlx-device"),
            required_features: optional_features(&adapter),
            ..Default::default()
        }))
        .map_err(RenderError::RequestDevice)?;

        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or(RenderError::UnsupportedSurface)?;
        // Vsync everywhere; the render loop paces itself off the display.
        config.present_mode = wgpu::PresentMode::AutoVsync;
        // Explicit swapchain depth (NFR 12 secondary lever): pin a 2-frame
        // latency (double-buffered) rather than leaving it to the backend
        // default, so the in-flight image count - and its VRAM - is bounded and
        // stated, not implicit.
        config.desired_maximum_frame_latency = 2;
        // `COPY_DST` where the surface offers it, so a frame drawn into the
        // preview intermediate can reach this swapchain by an exact
        // `copy_texture_to_texture` rather than through a sampling blit, which
        // would round-trip the encoded values ADR-0096 dithers. A usage flag
        // costs nothing while nothing copies; the caps query is what decides,
        // and `Renderer::open_preview` reports the refusal rather than
        // degrading to an inexact path behind the operator's back.
        let caps = surface.get_capabilities(&adapter);
        if caps.usages.contains(wgpu::TextureUsages::COPY_DST) {
            config.usage |= wgpu::TextureUsages::COPY_DST;
        }
        surface.configure(&device, &config);

        let info = adapter.get_info();
        let is_software = info.device_type == wgpu::DeviceType::Cpu;
        let description = describe_adapter(&info);
        Ok(Self {
            surface: Some(surface),
            device,
            queue,
            config,
            is_software,
            class: AdapterClass::of(info.device_type),
            adapter: description,
            instance: instance.clone(),
            gpu: adapter,
        })
    }

    /// Build a surface-less context for headless capture (Plan 0013): a device
    /// and queue with no swapchain, drawing into offscreen textures. No window,
    /// no present, no added dependency. `prefer_software` forces a fallback
    /// adapter (WARP on DX12) so tests rasterize identically on any machine.
    ///
    /// The synthesized [`wgpu::SurfaceConfiguration`] carries only the render
    /// size and the offscreen format (`HEADLESS_FORMAT`); its present-related
    /// fields are inert with no surface to configure.
    pub fn new_headless(
        width: u32,
        height: u32,
        prefer_software: bool,
    ) -> Result<Self, RenderError> {
        Self::new_headless_on(width, height, &AdapterChoice::from(prefer_software))
    }

    /// A headless context on a **named** adapter (ADR-0146).
    ///
    /// The one real constructor of the two; [`new_headless`](Self::new_headless)
    /// delegates here. It exists because a live video-out has to render on a
    /// GPU the operator can name - on a hybrid machine Windows hands a console
    /// process the power-saving one - while every capture path wants exactly
    /// the adapter it already asks for.
    pub fn new_headless_on(
        width: u32,
        height: u32,
        choice: &AdapterChoice,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        // No surface: a headless context presents to nothing, so every adapter
        // on the machine is a candidate and there is no compatibility to check.
        let adapter = resolve_adapter(&instance, choice, None)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rlx-headless-device"),
            required_features: optional_features(&adapter),
            ..Default::default()
        }))
        .map_err(RenderError::RequestDevice)?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: HEADLESS_FORMAT,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };

        let info = adapter.get_info();
        let is_software = info.device_type == wgpu::DeviceType::Cpu;
        let description = describe_adapter(&info);
        Ok(Self {
            surface: None,
            device,
            queue,
            config,
            is_software,
            class: AdapterClass::of(info.device_type),
            adapter: description,
            instance,
            gpu: adapter,
        })
    }

    /// Reconfigure the surface for a new size (a zero dimension is ignored).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // minimized; keep the old config until we're visible again
        }
        self.config.width = width;
        self.config.height = height;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }

    /// The texture format the surface is configured with.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Whether this context's frame destination accepts a texture-to-texture
    /// copy — the requirement the program preview's exact path rests on.
    ///
    /// With a surface it is what the swapchain was actually configured with,
    /// which `from_surface` sets only where the surface's capabilities offer it.
    /// Headless there is no swapchain and the destination is a capture target,
    /// which is built with `COPY_DST` unconditionally.
    pub(crate) fn can_copy_to_target(&self) -> bool {
        match self.surface {
            Some(_) => self.config.usage.contains(wgpu::TextureUsages::COPY_DST),
            None => true,
        }
    }

    /// Whether the active adapter is a CPU/software rasterizer (see the field).
    pub(crate) fn is_software(&self) -> bool {
        self.is_software
    }

    /// The active adapter's class (see the field).
    pub(crate) fn adapter_class(&self) -> AdapterClass {
        self.class
    }

    /// The active adapter's description — name, backend, device type, driver —
    /// for a report that has to name the machine it was taken on (ADR-0071).
    pub(crate) fn adapter(&self) -> &str {
        &self.adapter
    }

    /// Re-apply the current configuration (after a Lost/Outdated surface).
    /// A no-op on a headless context (no surface to reconfigure).
    pub(crate) fn reconfigure(&self) {
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }

    /// Stage a move onto another adapter (ADR-0246): resolve `choice` against
    /// this context's window, request its device, negotiate the surface
    /// configuration, and create a fresh surface for `target` — **without
    /// touching anything this context owns**. Every step that can fail is
    /// here, so an `Err` leaves the running context exactly as it was, and
    /// [`commit`](Self::commit) is the one step that cannot.
    ///
    /// `Ok(None)` when `choice` resolves to the adapter already in use: the
    /// device that would be built is the one that exists, and rebuilding onto
    /// it would restart every accumulation for no change.
    ///
    /// The window's surface is **re-created rather than re-configured**. A
    /// surface is instance-scoped and the instance is retained, but a
    /// swapchain belongs to the device it was configured on, and
    /// `Surface::configure` replaces the previous swapchain through whichever
    /// device it is handed — sound only while that is the same device. A new
    /// surface for the same window carries no swapchain until it is
    /// configured, and dropping the old one releases its swapchain through
    /// the device that made it.
    ///
    /// Refused with [`RenderError::Headless`] on a context without a surface:
    /// there is no window to re-create a surface for, and the headless path is
    /// the one [`adapter_change_permitted`] excludes.
    pub(crate) fn stage_adapter(
        &self,
        choice: &AdapterChoice,
        target: impl Into<SurfaceTarget<'static>>,
    ) -> Result<Option<StagedContext>, RenderError> {
        let Some(current) = self.surface.as_ref() else {
            return Err(RenderError::Headless);
        };
        // Resolved against the surface the window already has: the new one
        // below is for the same window, so presentability is the same question.
        let adapter = resolve_adapter(&self.instance, choice, Some(current))?;
        let info = adapter.get_info();
        let description = describe_adapter(&info);
        if description == self.adapter {
            return Ok(None);
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rlx-device"),
            required_features: optional_features(&adapter),
            ..Default::default()
        }))
        .map_err(RenderError::RequestDevice)?;
        let mut config = current
            .get_default_config(
                &adapter,
                self.config.width.max(1),
                self.config.height.max(1),
            )
            .ok_or(RenderError::UnsupportedSurface)?;
        // The same three choices `from_surface` makes for a fresh window, so a
        // switched context is configured exactly as a launched one.
        config.present_mode = wgpu::PresentMode::AutoVsync;
        config.desired_maximum_frame_latency = 2;
        if current
            .get_capabilities(&adapter)
            .usages
            .contains(wgpu::TextureUsages::COPY_DST)
        {
            config.usage |= wgpu::TextureUsages::COPY_DST;
        }
        // Created last, after everything that can refuse: a surface is a
        // handle on the window and not yet a swapchain, so two of them on one
        // window coexist until one is configured.
        let surface = self
            .instance
            .create_surface(target)
            .map_err(RenderError::CreateSurface)?;
        Ok(Some(StagedContext {
            surface,
            device,
            queue,
            config,
            gpu: adapter,
            is_software: info.device_type == wgpu::DeviceType::Cpu,
            class: AdapterClass::of(info.device_type),
            adapter: description,
        }))
    }

    /// Make a staged context this context, releasing the old device.
    ///
    /// Order is load-bearing. The old surface goes first, because dropping it
    /// is what releases its swapchain through the device that made it, and
    /// because DXGI allows one swap chain per window: the new surface is
    /// configured only once the old swapchain is gone. The old device, queue
    /// and adapter are dropped last; every resource built on them holds its
    /// own reference, so the caller's replacements can be built before this
    /// and the old ones dropped after it.
    pub(crate) fn commit(&mut self, staged: StagedContext) {
        self.surface = None;
        staged.surface.configure(&staged.device, &staged.config);
        self.surface = Some(staged.surface);
        self.config = staged.config;
        self.device = staged.device;
        self.queue = staged.queue;
        self.gpu = staged.gpu;
        self.is_software = staged.is_software;
        self.class = staged.class;
        self.adapter = staged.adapter;
    }
}

/// A device on another adapter, requested and validated, with a fresh surface
/// for the same window — not yet the context's own. Built by
/// [`RenderContext::stage_adapter`], consumed by [`RenderContext::commit`],
/// and dropped whole if the caller's own rebuild between the two fails.
pub(crate) struct StagedContext {
    surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    gpu: wgpu::Adapter,
    is_software: bool,
    class: AdapterClass,
    adapter: String,
}

impl StagedContext {
    /// The class of the adapter this context would move onto — read before the
    /// commit, so the grid scale the rebuilt scenes take is the new adapter's.
    pub(crate) fn adapter_class(&self) -> AdapterClass {
        self.class
    }
}
