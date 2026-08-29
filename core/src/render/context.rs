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
    /// The consumer of a **streamed** capture refused a frame — the offline
    /// render mode's pipe closed, the encoder died, the file could not be
    /// written (Plan 0101 / ADR-0114). The string is the consumer's own message,
    /// carried verbatim rather than flattened to "write failed": that consumer
    /// is a child process, and a mystery broken pipe is the obvious way this
    /// path goes wrong.
    Sink(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::CreateSurface(e) => write!(f, "surface creation failed: {e}"),
            RenderError::RequestAdapter(e) => write!(f, "no suitable GPU adapter: {e}"),
            RenderError::RequestDevice(e) => write!(f, "device request failed: {e}"),
            RenderError::UnsupportedSurface => write!(f, "surface has no supported config"),
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
            RenderError::Sink(msg) => write!(f, "{msg}"),
        }
    }
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

/// Which graphics adapter a headless context should render on.
///
/// Stated in wgpu's own vocabulary and nothing else: no platform type, no
/// vendor branch, no backend branch, so `core` stays GPU-abstract (ADR-0001)
/// while still letting a shell say which GPU it means.
///
/// **The variants are not interchangeable views of one preference.** The first
/// three ask wgpu to choose and accept whatever it returns; the last two name
/// one adapter out of the enumerated roster and fail if it is not there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterChoice {
    /// Whatever wgpu picks with default options. On a hybrid machine this is
    /// the power-saving GPU for a console process, which is why a live path
    /// wants `HighPerformance` instead.
    Default,
    /// Force a fallback (software) adapter - WARP on DX12 - so captures
    /// rasterize identically across machines. What the golden suite asks for.
    Software,
    /// `PowerPreference::HighPerformance`: the discrete GPU on a hybrid
    /// machine.
    HighPerformance,
    /// The one enumerated adapter whose name contains this string, matched
    /// case-insensitively. More than one match is an error, not a pick.
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
fn resolve_adapter(
    instance: &wgpu::Instance,
    choice: &AdapterChoice,
) -> Result<wgpu::Adapter, RenderError> {
    let by_preference = |options: wgpu::RequestAdapterOptions<'_, '_>| {
        pollster::block_on(instance.request_adapter(&options)).map_err(RenderError::RequestAdapter)
    };
    match choice {
        AdapterChoice::Default => by_preference(wgpu::RequestAdapterOptions::default()),
        AdapterChoice::Software => by_preference(wgpu::RequestAdapterOptions {
            force_fallback_adapter: true,
            ..Default::default()
        }),
        AdapterChoice::HighPerformance => by_preference(wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }),
        AdapterChoice::Index(wanted) => {
            let roster = describe_roster(instance);
            pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                .into_iter()
                .nth(*wanted)
                .ok_or_else(|| RenderError::NoSuchAdapter {
                    requested: format!("index {wanted}"),
                    available: roster.into_iter().map(|entry| entry.detail).collect(),
                })
        }
        AdapterChoice::Named(wanted) => {
            let needle = wanted.to_lowercase();
            let roster = describe_roster(instance);
            let hits: Vec<usize> = roster
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.name.to_lowercase().contains(&needle))
                .map(|(at, _)| at)
                .collect();
            match hits.as_slice() {
                [] => Err(RenderError::NoSuchAdapter {
                    requested: wanted.clone(),
                    available: roster.into_iter().map(|entry| entry.detail).collect(),
                }),
                [only] => pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
                    .into_iter()
                    .nth(*only)
                    .ok_or_else(|| RenderError::NoSuchAdapter {
                        requested: wanted.clone(),
                        available: roster.iter().map(|entry| entry.detail.clone()).collect(),
                    }),
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
    /// Whether the selected adapter is a CPU/software rasterizer (WARP on DX12,
    /// llvmpipe on Vulkan). The headless capture path forces this for
    /// reproducibility; visual-QA tests read it to skip checks the software
    /// rasterizer can't render faithfully (e.g. fullscreen-scene + background
    /// pipeline coexistence, a documented WARP quirk).
    is_software: bool,
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
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(target)
            .map_err(RenderError::CreateSurface)?;
        Self::from_surface(&instance, surface, width, height)
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
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = unsafe { instance.create_surface_unsafe(target) }
            .map_err(RenderError::CreateSurface)?;
        Self::from_surface(&instance, surface, width, height)
    }

    fn from_surface(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(RenderError::RequestAdapter)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("lmv-device"),
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
        surface.configure(&device, &config);

        let info = adapter.get_info();
        let is_software = info.device_type == wgpu::DeviceType::Cpu;
        let adapter = describe_adapter(&info);
        Ok(Self {
            surface: Some(surface),
            device,
            queue,
            config,
            is_software,
            adapter,
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
        let adapter = resolve_adapter(&instance, choice)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("lmv-headless-device"),
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
        let adapter = describe_adapter(&info);
        Ok(Self {
            surface: None,
            device,
            queue,
            config,
            is_software,
            adapter,
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

    /// Whether the active adapter is a CPU/software rasterizer (see the field).
    pub(crate) fn is_software(&self) -> bool {
        self.is_software
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
}
