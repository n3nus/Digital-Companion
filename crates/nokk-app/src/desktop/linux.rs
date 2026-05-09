use std::fs::File;
use std::io::{BufWriter, Write};
use std::os::fd::AsFd;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use ::wayland_client::protocol::wl_buffer::WlBuffer;
use anyhow::{Context, Result, bail};
use layershellev::reexport::*;
use layershellev::*;
use memmap2::MmapMut;
use nokk_core::{AppConfig, Bounds, GestureEvent, GestureTracker, PetBrain, SpriteSheet, Surface};

use crate::app_assets;

const DEFAULT_MONITOR_WIDTH: i32 = 1920;
const DEFAULT_MONITOR_HEIGHT: i32 = 1080;
const SURFACE_PADDING: i32 = 18;
const FRAME_INTERVAL_MS: u64 = 100;
const SHM_BUFFER_COUNT: usize = 3;
const LEFT_BUTTON: u32 = 0x110;
const DRAG_THRESHOLD_PX: i32 = 4;

#[derive(Clone, Debug)]
enum DesktopCommand {
    TogglePause,
    ResetPosition,
    Quit,
}

pub fn run() -> Result<()> {
    if std::env::var("XDG_SESSION_TYPE").ok().as_deref() != Some("wayland") {
        bail!(
            "Nøkk desktop on Linux V1 needs a Wayland session; use `nokk console` outside Wayland"
        );
    }

    let sheet = app_assets::load_sprites()?;
    let config = AppConfig::load_or_default().unwrap_or_default();
    let (tx, rx) = mpsc::channel();
    spawn_tray(tx.clone()).context("start StatusNotifier tray")?;

    let scale = config.scale.max(1);
    let pet_size = sheet.frame_size() as i32 * scale as i32;
    let surface_size = (pet_size + SURFACE_PADDING * 2 + 24) as u32;
    let default_position = (48, (DEFAULT_MONITOR_HEIGHT - pet_size - 72).max(24));
    let start_position = config
        .position
        .filter(|position| *position != (32, 32))
        .or(Some(default_position));

    let mut state = LayerState {
        sheet,
        brain: PetBrain::from_config(
            nokk_core::pet::unix_time_seed(),
            start_position,
            config.last_pose,
            config.mood,
            config.paused,
        ),
        gesture: GestureTracker::default(),
        started: Instant::now(),
        rx,
        scale,
        last_pointer: (0, 0),
        quit: false,
        drag: None,
        buffers: Vec::new(),
        buffer_cursor: 0,
        bounds: Bounds {
            width: DEFAULT_MONITOR_WIDTH,
            height: DEFAULT_MONITOR_HEIGHT,
            pet_size,
        },
    };

    let event_loop: WindowState<()> = WindowState::new("Nøkk")
        .with_size((surface_size, surface_size))
        .with_layer(Layer::Top)
        .with_margin((32, 0, 0, 32))
        .with_anchor(Anchor::Top | Anchor::Left)
        .with_keyboard_interacivity(KeyboardInteractivity::None)
        .with_exclusive_zone(-1)
        .build()
        .context("create Wayland layer-shell surface")?;

    event_loop
        .running(move |event, event_loop, index| {
            state.process_commands();
            if state.quit {
                state.persist();
                return ReturnData::RequestExit;
            }

            match event {
                LayerShellEvent::InitRequest => ReturnData::RequestCompositor,
                LayerShellEvent::CompositorProvide(_, _) => {
                    event_loop.request_refresh_all(RefreshRequest::NextFrame);
                    ReturnData::None
                }
                LayerShellEvent::XdgInfoChanged(_) => {
                    if let Some(index) = index {
                        if let Some(unit) = event_loop.get_unit_with_id(index) {
                            if let Some(info) = unit.get_xdgoutput_info() {
                                let text = format!("{info:?}");
                                state.apply_output_hint(&text);
                            }
                            state.apply_margin(unit);
                        }
                    }
                    event_loop.request_refresh_all(RefreshRequest::NextFrame);
                    ReturnData::None
                }
                LayerShellEvent::RequestBuffer(file, shm, qh, width, height) => {
                    if let Err(err) = state.recreate_buffers(shm, qh, width, height) {
                        eprintln!("Nøkk: could not create animated Wayland buffer: {err}");
                        state.draw_to_file(file, (width, height));
                        let pool =
                            shm.create_pool(file.as_fd(), (width * height * 4) as i32, qh, ());
                        return ReturnData::WlBuffer(pool.create_buffer(
                            0,
                            width as i32,
                            height as i32,
                            (width * 4) as i32,
                            wl_shm::Format::Argb8888,
                            qh,
                            (),
                        ));
                    }

                    let Some(buffer) = state.render_buffer(0) else {
                        state.draw_to_file(file, (width, height));
                        let pool =
                            shm.create_pool(file.as_fd(), (width * height * 4) as i32, qh, ());
                        return ReturnData::WlBuffer(pool.create_buffer(
                            0,
                            width as i32,
                            height as i32,
                            (width * 4) as i32,
                            wl_shm::Format::Argb8888,
                            qh,
                            (),
                        ));
                    };

                    state.buffer_cursor = 0;
                    ReturnData::WlBuffer(buffer)
                }
                LayerShellEvent::NormalDispatch => {
                    event_loop.request_refresh_all(RefreshRequest::At(
                        Instant::now() + Duration::from_millis(FRAME_INTERVAL_MS),
                    ));
                    ReturnData::None
                }
                LayerShellEvent::RequestMessages(DispatchMessage::RequestRefresh { .. }) => {
                    let buffer = state.render_next_buffer();

                    if let Some(index) = index {
                        if let Some(unit) = event_loop.get_unit_with_id(index) {
                            state.apply_margin(unit);
                            if let Some(buffer) = buffer.as_ref() {
                                let (width, height) = unit.get_size();
                                let surface = unit.get_wlsurface();
                                surface.attach(Some(buffer), 0, 0);
                                surface.damage(0, 0, width as i32, height as i32);
                                surface.commit();
                            }
                        }
                    }

                    event_loop.request_refresh_all(RefreshRequest::At(
                        Instant::now() + Duration::from_millis(FRAME_INTERVAL_MS),
                    ));
                    ReturnData::None
                }
                LayerShellEvent::RequestMessages(DispatchMessage::MouseMotion {
                    time,
                    surface_x,
                    surface_y,
                }) => {
                    let x = *surface_x as i32;
                    let y = *surface_y as i32;
                    state.last_pointer = (x, y);

                    if state.drag_to_pointer(x, y) {
                        event_loop.request_refresh_all(RefreshRequest::NextFrame);
                        return ReturnData::None;
                    }

                    let local_x = (x - SURFACE_PADDING) / state.scale as i32;
                    let local_y = (y - SURFACE_PADDING) / state.scale as i32;
                    if state.gesture.pointer_moved(
                        local_x,
                        local_y,
                        u64::from(*time),
                        state.sheet.manifest(),
                    ) == Some(GestureEvent::Stroked)
                    {
                        state.brain.stroke(
                            state.started.elapsed().as_millis() as u64,
                            state.sheet.manifest(),
                        );
                        event_loop.request_refresh_all(RefreshRequest::NextFrame);
                    }
                    ReturnData::None
                }
                LayerShellEvent::RequestMessages(DispatchMessage::MouseButton {
                    state: button_state,
                    button,
                    ..
                }) => {
                    if *button == LEFT_BUTTON {
                        match button_state {
                            ::wayland_client::WEnum::Value(
                                ::wayland_client::protocol::wl_pointer::ButtonState::Pressed,
                            ) => state.start_drag_if_body(),
                            ::wayland_client::WEnum::Value(
                                ::wayland_client::protocol::wl_pointer::ButtonState::Released,
                            ) => state.finish_drag_or_poke(),
                            _ => {}
                        }
                        event_loop.request_refresh_all(RefreshRequest::NextFrame);
                    }
                    ReturnData::None
                }
                _ => ReturnData::None,
            }
        })
        .context("run Wayland layer-shell event loop")?;

    Ok(())
}

struct SharedBuffer {
    _file: File,
    mmap: MmapMut,
    buffer: WlBuffer,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug)]
struct DragState {
    offset_x: i32,
    offset_y: i32,
    moved: bool,
}

struct LayerState {
    sheet: SpriteSheet,
    brain: PetBrain,
    gesture: GestureTracker,
    started: Instant,
    rx: Receiver<DesktopCommand>,
    scale: u32,
    last_pointer: (i32, i32),
    quit: bool,
    drag: Option<DragState>,
    buffers: Vec<SharedBuffer>,
    buffer_cursor: usize,
    bounds: Bounds,
}

impl LayerState {
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn process_commands(&mut self) {
        while let Ok(command) = self.rx.try_recv() {
            match command {
                DesktopCommand::TogglePause => self.brain.toggle_paused(),
                DesktopCommand::ResetPosition => self.brain.reset_position(self.bounds),
                DesktopCommand::Quit => self.quit = true,
            }
        }
    }

    fn tick(&mut self) {
        self.brain.tick(self.now_ms(), self.bounds);
    }

    fn apply_margin<T>(&self, unit: &WindowStateUnit<T>) {
        let snapshot = self.brain.snapshot();
        unit.set_margin((snapshot.y.max(0), 0, 0, snapshot.x.max(0)));
    }

    fn start_drag_if_body(&mut self) {
        let (x, y) = self.last_pointer;
        let local_x = (x - SURFACE_PADDING) / self.scale as i32;
        let local_y = (y - SURFACE_PADDING) / self.scale as i32;
        if self.sheet.manifest().is_body_zone(local_x, local_y) {
            self.brain.begin_drag(self.now_ms());
            self.drag = Some(DragState {
                offset_x: x,
                offset_y: y,
                moved: false,
            });
        }
    }

    fn drag_to_pointer(&mut self, x: i32, y: i32) -> bool {
        let Some(drag) = self.drag else {
            return false;
        };

        let snapshot = self.brain.snapshot();
        let target_x = snapshot.x + x - drag.offset_x;
        let target_y = snapshot.y + y - drag.offset_y;
        let dx = target_x - snapshot.x;
        let dy = target_y - snapshot.y;

        if let Some(drag) = &mut self.drag {
            drag.moved |= dx.abs() >= DRAG_THRESHOLD_PX || dy.abs() >= DRAG_THRESHOLD_PX;
        }
        self.brain.set_position(target_x, target_y, self.bounds);
        true
    }

    fn finish_drag_or_poke(&mut self) {
        let Some(drag) = self.drag.take() else {
            return;
        };

        let now_ms = self.now_ms();
        if drag.moved {
            self.brain.knockdown(now_ms);
        } else {
            self.brain.poke(now_ms);
        }
    }

    fn apply_output_hint(&mut self, debug_text: &str) {
        let Some(size_start) = debug_text.find("logical_size: Some((") else {
            return;
        };
        let rest = &debug_text[size_start + "logical_size: Some((".len()..];
        let Some(size_end) = rest.find("))") else {
            return;
        };
        let pair = &rest[..size_end];
        let mut parts = pair.split(',').map(str::trim);
        if let (Some(width), Some(height)) = (parts.next(), parts.next()) {
            if let (Ok(width), Ok(height)) = (width.parse::<i32>(), height.parse::<i32>()) {
                self.bounds.width = width.max(self.bounds.pet_size);
                self.bounds.height = height.max(self.bounds.pet_size);
            }
        }
    }

    fn recreate_buffers(
        &mut self,
        shm: &wl_shm::WlShm,
        qh: &wayland_client::QueueHandle<WindowState<()>>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let size = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .context("invalid Wayland buffer size")?;
        let size_i32 = i32::try_from(size).context("Wayland buffer is too large")?;
        let stride_i32 = i32::try_from(width.checked_mul(4).context("invalid buffer stride")?)
            .context("Wayland buffer stride is too large")?;

        self.buffers.clear();
        self.buffer_cursor = 0;

        for _ in 0..SHM_BUFFER_COUNT {
            let file = tempfile::tempfile().context("create Wayland shm tempfile")?;
            file.set_len(u64::from(size))
                .context("size Wayland shm tempfile")?;
            // SAFETY: The file length is set to the full buffer size before mapping and
            // the mmap is kept alive for at least as long as the wl_buffer proxy.
            let mmap = unsafe { MmapMut::map_mut(&file).context("map Wayland shm buffer")? };
            let pool = shm.create_pool(file.as_fd(), size_i32, qh, ());
            let buffer = pool.create_buffer(
                0,
                width as i32,
                height as i32,
                stride_i32,
                wl_shm::Format::Argb8888,
                qh,
                (),
            );

            self.buffers.push(SharedBuffer {
                _file: file,
                mmap,
                buffer,
                width,
                height,
            });
        }

        Ok(())
    }

    fn render_next_buffer(&mut self) -> Option<WlBuffer> {
        if self.buffers.is_empty() {
            return None;
        }
        let index = (self.buffer_cursor + 1) % self.buffers.len();
        self.render_buffer(index)
    }

    fn render_buffer(&mut self, index: usize) -> Option<WlBuffer> {
        let (width, height) = {
            let buffer = self.buffers.get(index)?;
            (buffer.width, buffer.height)
        };
        let bytes = self.render_surface_bytes(width, height);
        let expected = (width * height * 4) as usize;
        let buffer = self.buffers.get_mut(index)?;
        if bytes.len() == expected && buffer.mmap.len() == expected {
            buffer.mmap.copy_from_slice(&bytes);
        } else {
            buffer.mmap.fill(0);
        }
        self.buffer_cursor = index;
        Some(buffer.buffer.clone())
    }

    fn render_surface_bytes(&mut self, width: u32, height: u32) -> Vec<u8> {
        self.tick();
        let mut surface = Surface::new(width, height);
        let snapshot = self.brain.snapshot();
        let frame = self
            .brain
            .current_frame(self.sheet.manifest(), self.now_ms());
        surface.blit_frame(
            &self.sheet,
            frame,
            SURFACE_PADDING,
            SURFACE_PADDING,
            self.scale,
        );

        for particle in self.brain.particles() {
            let x = (particle.x - snapshot.x as f32) as i32 + SURFACE_PADDING - 6;
            let y = (particle.y - snapshot.y as f32) as i32 + SURFACE_PADDING - 6;
            surface.blit_frame_with_alpha(
                &self.sheet,
                particle.frame,
                x,
                y,
                self.scale.min(2),
                particle.alpha(),
            );
        }

        surface.as_argb8888_native_endian()
    }

    fn draw_to_file(&mut self, file: &mut File, (width, height): (u32, u32)) {
        let bytes = self.render_surface_bytes(width, height);
        let mut writer = BufWriter::new(file);
        let expected = (width * height * 4) as usize;
        if bytes.len() == expected {
            let _ = writer.write_all(&bytes);
        } else {
            let _ = writer.write_all(&vec![0; expected]);
        }
        let _ = writer.flush();
    }

    fn persist(&self) {
        let snapshot = self.brain.snapshot();
        let _ = AppConfig {
            position: Some((snapshot.x, snapshot.y)),
            last_pose: snapshot.animation,
            mood: snapshot.mood,
            scale: self.scale,
            paused: snapshot.paused,
            monitor: None,
        }
        .save();
    }
}

fn spawn_tray(tx: Sender<DesktopCommand>) -> Result<()> {
    let (ready_tx, ready_rx) = mpsc::channel();
    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                let _ = ready_tx.send(Err(format!("could not start tray runtime: {err}")));
                return;
            }
        };

        runtime.block_on(async move {
            use ksni::TrayMethods;

            let tray = LinuxTray { tx, paused: false };
            match tray.spawn().await {
                Ok(_handle) => {
                    let _ = ready_tx.send(Ok(()));
                    std::future::pending::<()>().await;
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                }
            }
        });
    });

    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => bail!("{message}"),
        Err(_) => Ok(()),
    }
}

#[derive(Clone, Debug)]
struct LinuxTray {
    tx: Sender<DesktopCommand>,
    paused: bool,
}

impl ksni::Tray for LinuxTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "nokk".into()
    }

    fn title(&self) -> String {
        "Nøkk".into()
    }

    fn icon_name(&self) -> String {
        "applications-games".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![tray_icon()]
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{CheckmarkItem, StandardItem};

        vec![
            CheckmarkItem {
                label: "Pause".into(),
                checked: self.paused,
                activate: Box::new(|tray: &mut Self| {
                    tray.paused = !tray.paused;
                    let _ = tray.tx.send(DesktopCommand::TogglePause);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Choose Monitor: active output".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Reset Position".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(DesktopCommand::ResetPosition);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Console Mode: run `nokk console`".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(DesktopCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn tray_icon() -> ksni::Icon {
    let width = 22;
    let height = 22;
    let mut data = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let dx = x as i32 - 11;
            let dy = y as i32 - 11;
            let inside = dx * dx + dy * dy <= 92;
            let (a, r, g, b) = if inside {
                (255, 92, 186, 83)
            } else {
                (0, 0, 0, 0)
            };
            data.extend_from_slice(&[a, r, g, b]);
        }
    }
    ksni::Icon {
        width: width as i32,
        height: height as i32,
        data,
    }
}
