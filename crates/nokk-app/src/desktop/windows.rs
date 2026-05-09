use std::ffi::{c_void, OsStr};
use std::iter::once;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use std::time::Instant;

use anyhow::{Context, Result};
use nokk_core::{AppConfig, Bounds, GestureEvent, GestureTracker, PetBrain, SpriteSheet, Surface};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP,
    HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CREATESTRUCTW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyMenu, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW, IDC_ARROW,
    IDI_APPLICATION, LoadCursorW, LoadIconW, MSG, PostQuitMessage, RegisterClassW, SW_SHOW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TPM_RIGHTBUTTON,
    TrackPopupMenu, TranslateMessage, UpdateLayeredWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_COMMAND,
    WM_CREATE, WM_DESTROY, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, GetWindowLongPtrW, ULW_ALPHA,
    MF_STRING,
};

use crate::app_assets;

const WM_TRAY: u32 = WM_APP + 1;
const TIMER_ID: usize = 1;
const MENU_PAUSE: usize = 1001;
const MENU_RESET: usize = 1002;
const MENU_QUIT: usize = 1003;
const SURFACE_PADDING: i32 = 18;

pub fn run() -> Result<()> {
    let sheet = app_assets::load_sprites()?;
    let config = AppConfig::load_or_default().unwrap_or_default();
    let scale = config.scale.max(1);
    let pet_size = sheet.frame_size() as i32 * scale as i32;
    let surface_size = pet_size + SURFACE_PADDING * 2 + 24;
    let bounds = Bounds {
        width: 1920,
        height: 1080,
        pet_size,
    };

    let app = Box::new(WinApp {
        sheet,
        brain: PetBrain::from_config(
            nokk_core::pet::unix_time_seed(),
            config.position.or(Some((32, 32))),
            config.last_pose,
            config.mood,
            config.paused,
        ),
        gesture: GestureTracker::default(),
        started: Instant::now(),
        scale,
        surface_size,
        bounds,
        last_pointer: (0, 0),
    });

    unsafe { run_native(app).context("run native Windows overlay") }
}

struct WinApp {
    sheet: SpriteSheet,
    brain: PetBrain,
    gesture: GestureTracker,
    started: Instant,
    scale: u32,
    surface_size: i32,
    bounds: Bounds,
    last_pointer: (i32, i32),
}

impl WinApp {
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn tick(&mut self) {
        self.brain.tick(self.now_ms(), self.bounds);
    }

    fn persist(&self) {
        let snapshot = self.brain.snapshot();
        let _ = AppConfig {
            monitor: None,
            position: Some((snapshot.x, snapshot.y)),
            last_pose: snapshot.animation,
            mood: snapshot.mood,
            scale: self.scale,
            paused: snapshot.paused,
        }
        .save();
    }

    fn render_surface(&mut self) -> Surface {
        self.tick();
        let mut surface = Surface::new(self.surface_size as u32, self.surface_size as u32);
        let snapshot = self.brain.snapshot();
        let frame = self.brain.current_frame(self.sheet.manifest(), self.now_ms());
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
        surface
    }
}

unsafe fn run_native(app: Box<WinApp>) -> Result<()> {
    let instance = HINSTANCE(GetModuleHandleW(None)?.0);
    let class_name = w!("NokkOverlayWindow");

    let wc = WNDCLASSW {
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        hIcon: LoadIconW(None, IDI_APPLICATION)?,
        hInstance: instance,
        lpszClassName: class_name,
        lpfnWndProc: Some(window_proc),
        ..Default::default()
    };
    RegisterClassW(&wc);

    let raw = Box::into_raw(app);
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(WS_EX_LAYERED.0 | WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0),
        class_name,
        w!("Nøkk"),
        WINDOW_STYLE(WS_POPUP.0),
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        1,
        1,
        None,
        None,
        Some(instance),
        Some(raw.cast::<c_void>() as *const c_void),
    )
    .inspect_err(|_| {
        unsafe {
            let _ = Box::from_raw(raw);
        }
    })?;

    ShowWindow(hwnd, SW_SHOW);
    add_tray(hwnd)?;
    SetTimer(hwnd, TIMER_ID, 33, None);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).into() {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        let create = lparam.0 as *const CREATESTRUCTW;
        let app = (*create).lpCreateParams as *mut WinApp;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app as isize);
        return LRESULT(0);
    }

    let app = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WinApp;

    match msg {
        WM_TIMER => {
            if !app.is_null() {
                (*app).tick();
                render_layered(hwnd, &mut *app);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if !app.is_null() {
                let x = low_word(lparam.0 as u32) as i32;
                let y = high_word(lparam.0 as u32) as i32;
                (*app).last_pointer = (x, y);
                let local_x = (x - SURFACE_PADDING) / (*app).scale as i32;
                let local_y = (y - SURFACE_PADDING) / (*app).scale as i32;
                if (*app).gesture.pointer_moved(
                    local_x,
                    local_y,
                    (*app).now_ms(),
                    (*app).sheet.manifest(),
                ) == Some(GestureEvent::Stroked)
                {
                    (*app).brain.stroke((*app).now_ms(), (*app).sheet.manifest());
                    render_layered(hwnd, &mut *app);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            if !app.is_null() {
                let (x, y) = (*app).last_pointer;
                let local_x = (x - SURFACE_PADDING) / (*app).scale as i32;
                let local_y = (y - SURFACE_PADDING) / (*app).scale as i32;
                if (*app).sheet.manifest().is_body_zone(local_x, local_y) {
                    (*app).brain.poke((*app).now_ms());
                    render_layered(hwnd, &mut *app);
                }
            }
            LRESULT(0)
        }
        WM_TRAY => {
            if lparam.0 as u32 == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            if !app.is_null() {
                match low_word(wparam.0 as u32) as usize {
                    MENU_PAUSE => (*app).brain.toggle_paused(),
                    MENU_RESET => (*app).brain.reset_position((*app).bounds),
                    MENU_QUIT => {
                        (*app).persist();
                        PostQuitMessage(0);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = delete_tray(hwnd);
            if !app.is_null() {
                (*app).persist();
                drop(Box::from_raw(app));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn render_layered(hwnd: HWND, app: &mut WinApp) {
    let snapshot = app.brain.snapshot();
    let surface = app.render_surface();
    let mut bgra = Vec::with_capacity(surface.pixels.len());
    for px in surface.pixels.chunks_exact(4) {
        bgra.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }

    let mem_dc = CreateCompatibleDC(None);
    if mem_dc.0 == 0 {
        return;
    }

    let bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: surface.width as i32,
            biHeight: -(surface.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..unsafe { zeroed() }
    };

    let mut bits: *mut c_void = null_mut();
    let Ok(bitmap) = CreateDIBSection(
        Some(mem_dc),
        &bitmap_info,
        DIB_RGB_COLORS,
        &mut bits,
        None,
        0,
    ) else {
        DeleteDC(mem_dc);
        return;
    };
    if bitmap == HBITMAP(0) || bits.is_null() {
        DeleteDC(mem_dc);
        return;
    }

    std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast::<u8>(), bgra.len());
    let old = SelectObject(mem_dc, HGDIOBJ(bitmap.0));

    let dst = POINT {
        x: snapshot.x,
        y: snapshot.y,
    };
    let size = SIZE {
        cx: surface.width as i32,
        cy: surface.height as i32,
    };
    let src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA,
    };

    let _ = UpdateLayeredWindow(
        hwnd,
        None,
        Some(&dst as *const POINT),
        Some(&size as *const SIZE),
        Some(mem_dc),
        Some(&src as *const POINT),
        COLORREF(0),
        Some(&blend as *const BLENDFUNCTION),
        ULW_ALPHA,
    );
    let _ = SetWindowPos(
        hwnd,
        Some(windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST),
        snapshot.x,
        snapshot.y,
        surface.width as i32,
        surface.height as i32,
        Default::default(),
    );

    SelectObject(mem_dc, old);
    DeleteObject(HGDIOBJ(bitmap.0));
    DeleteDC(mem_dc);
}

unsafe fn add_tray(hwnd: HWND) -> Result<()> {
    let mut data = NOTIFYICONDATAW::default();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = LoadIconW(None, IDI_APPLICATION)?;
    write_tip(&mut data.szTip, "Nøkk");
    Shell_NotifyIconW(NIM_ADD, &data).ok()?;
    Ok(())
}

unsafe fn delete_tray(hwnd: HWND) -> Result<()> {
    let mut data = NOTIFYICONDATAW::default();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &data).ok()?;
    Ok(())
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else {
        return;
    };
    let pause = wide("Pause / Resume");
    let reset = wide("Reset Position");
    let quit = wide("Quit");
    let _ = AppendMenuW(menu, MF_STRING, MENU_PAUSE, PCWSTR(pause.as_ptr()));
    let _ = AppendMenuW(menu, MF_STRING, MENU_RESET, PCWSTR(reset.as_ptr()));
    let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, PCWSTR(quit.as_ptr()));
    let mut point = POINT::default();
    let _ = GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, None, hwnd, None);
    let _ = DestroyMenu(menu);
}

fn write_tip(dst: &mut [u16], tip: &str) {
    let wide = wide(tip);
    for (target, source) in dst.iter_mut().zip(wide.into_iter()) {
        *target = source;
    }
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(once(0)).collect()
}

fn low_word(value: u32) -> u16 {
    (value & 0xffff) as u16
}

fn high_word(value: u32) -> u16 {
    ((value >> 16) & 0xffff) as u16
}
