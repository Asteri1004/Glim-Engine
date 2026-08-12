use std::sync::Arc;
use wgpu::CurrentSurfaceTexture;
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        // InstanceDescriptor는 Default를 구현하지 않으므로
        // new_without_display_handle()을 베이스로 삼는다.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("사용 가능한 GPU 어댑터를 찾지 못했습니다");

        log::info!("선택된 어댑터: {:?}", adapter.get_info());

        // ..Default::default()가 experimental_features, memory_hints, trace를 채운다.
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("glim device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Self { surface, device, queue, config, size }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Result를 반환하지 않고 내부에서 서피스 상태를 직접 처리한다.
    pub fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            // 정상. 바로 사용.
            CurrentSurfaceTexture::Success(frame) => frame,

            // 쓸 수는 있지만 서피스 속성과 어긋남. 재설정 예약 후 이번 프레임은 그대로 사용.
            CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }

            // 서피스가 낡거나 유실됨. 재설정하고 이번 프레임은 건너뜀.
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }

            // 최소화·가림·타임아웃. 재설정 없이 건너뛰기만 하면 됨.
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return,

            CurrentSurfaceTexture::Validation => {
                log::error!("서피스 획득 중 검증 오류");
                return;
            }
        };

        let view = frame.texture.create_view(&Default::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame encoder"),
            });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.06,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                // multiview_mask 등 나머지는 Default가 채운다.
                ..Default::default()
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}
