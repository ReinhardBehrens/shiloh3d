//! Optional windowed host — owns winit window + device lifecycle.

use std::sync::Arc;

use shiloh_rhi::{Device, NullDevice};
use tracing::{info, warn};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::app::AppBuilder;

#[cfg(feature = "gpu")]
use shiloh_rhi::wgpu_backend::WgpuDevice;

/// Which RHI path the windowed host should prefer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RhiBackendKind {
    /// wgpu bootstrap (default for bring-up).
    #[default]
    Wgpu,
    /// Placeholder for future native Vulkan/D3D12/Metal.
    Native,
    /// CI / no GPU.
    Null,
}

/// Runs `App` with a real window. Surface present for demos stays in
/// `shiloh-render::{ForwardRenderer,SliceRenderer}`; this host owns lifecycle
/// and installs a selectable RHI device on `App`.
pub fn run_windowed(builder: AppBuilder, backend: RhiBackendKind) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut host = WindowHost {
        builder: Some(builder),
        app: None,
        window: None,
        backend,
        frames: 0,
    };
    event_loop.run_app(&mut host)?;
    Ok(())
}

struct WindowHost {
    builder: Option<AppBuilder>,
    app: Option<crate::app::App>,
    window: Option<Arc<Window>>,
    backend: RhiBackendKind,
    frames: u64,
}

impl ApplicationHandler for WindowHost {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // Same product mark as Studio chrome — OS taskbar / dock / Alt-Tab.
        // App id / WM_CLASS so Linux start bar matches packaging/linux/*.desktop.
        // Pattern adapted from winit platform examples (Apache-2.0/MIT):
        // https://github.com/rust-windowing/winit — WindowAttributesExt{X11,Wayland}::with_name
        let attrs = Window::default_attributes()
            .with_title("Shiloh3D")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_window_icon(Some(crate::icon::window_icon()));
        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        let attrs = {
            use winit::platform::wayland::WindowAttributesExtWayland as Wl;
            use winit::platform::x11::WindowAttributesExtX11 as X11;
            let attrs = Wl::with_name(attrs, crate::icon::SHILOH_APP_ID, "");
            X11::with_class(
                attrs,
                crate::icon::SHILOH_APP_ID.to_string(),
                "Shiloh3D".to_string(),
            )
        };
        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                let mut app = self.builder.take().expect("builder").build();
                let device: Box<dyn Device> = match self.backend {
                    RhiBackendKind::Wgpu => {
                        #[cfg(feature = "gpu")]
                        {
                            info!("windowed host ready (wgpu stub device)");
                            Box::new(WgpuDevice::stub())
                        }
                        #[cfg(not(feature = "gpu"))]
                        {
                            warn!("gpu feature off — using null device");
                            Box::new(NullDevice::new())
                        }
                    }
                    RhiBackendKind::Native => {
                        warn!("native RHI not wired yet — using null device");
                        Box::new(NullDevice::new())
                    }
                    RhiBackendKind::Null => Box::new(NullDevice::new()),
                };
                app.set_device(device);
                self.app = Some(app);
                self.window = Some(window);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            Err(err) => {
                warn!(?err, "window create failed");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(app) = self.app.as_mut()
                    && let winit::keyboard::PhysicalKey::Code(code) = event.physical_key
                {
                    let key = crate::winit_map::map_key(code);
                    if event.state == winit::event::ElementState::Pressed {
                        app.input.key_down(key);
                    } else {
                        app.input.key_up(key);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(app) = self.app.as_mut() {
                    let btn = crate::winit_map::map_mouse(button);
                    if state == winit::event::ElementState::Pressed {
                        app.input.mouse_down(btn);
                    } else {
                        app.input.mouse_up(btn);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(app) = self.app.as_mut() {
                    app.input.set_mouse_position(glam::Vec2::new(
                        position.x as f32,
                        position.y as f32,
                    ));
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(app) = self.app.as_mut() {
                    if let Err(err) = app.tick_once() {
                        warn!(?err, "tick failed");
                        event_loop.exit();
                        return;
                    }
                    self.frames = self.frames.wrapping_add(1);
                    if let Some(max) = app.max_frames()
                        && self.frames >= max
                    {
                        event_loop.exit();
                        return;
                    }
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
