//! GPU device creation.

use std::fmt;

#[derive(Debug)]
pub enum GpuError {
    NoAdapter(String),
    Device(String),
    Readback(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoAdapter(e) => write!(f, "no suitable GPU adapter: {e}"),
            GpuError::Device(e) => write!(f, "could not create a GPU device: {e}"),
            GpuError::Readback(e) => write!(f, "could not read the frame back: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// A device and queue, plus the instance and adapter they came from.
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// A device with no window, for tests, screenshots and offscreen rendering.
    pub fn headless() -> Result<Gpu, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
        Gpu::from_adapter(instance, adapter)
    }

    /// A device on an adapter that can present to `surface`.
    pub fn for_surface(instance: wgpu::Instance, surface: &wgpu::Surface<'_>) -> Result<Gpu, GpuError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(surface),
            ..Default::default()
        }))
        .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
        Gpu::from_adapter(instance, adapter)
    }

    fn from_adapter(instance: wgpu::Instance, adapter: wgpu::Adapter) -> Result<Gpu, GpuError> {
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("vui device"),
            ..Default::default()
        }))
        .map_err(|e| GpuError::Device(e.to_string()))?;
        Ok(Gpu { instance, adapter, device, queue })
    }

    pub fn info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }
}
