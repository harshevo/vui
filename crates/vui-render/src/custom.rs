//! Custom shader passes: drawing the stock pipelines cannot do, such as a shape with its own distance
//! field or a glow, done by the app's own shaders in the same render pass as everything else.

use vui_core::CustomCmd;

/// A shader pass the app adds to a [`Renderer`](crate::Renderer) under a name. Widgets paint
/// [`CustomCmd`]s naming it; the renderer hands the pass all of a frame's commands once, then asks it
/// to draw each one in its place among the quads, images and text, so layering is kept.
///
/// The pass draws into the renderer's target format with premultiplied colour in encoded sRGB, like
/// the stock pipelines; `CustomCmd::opacity` and `clip` are the pass's to honour.
pub trait CustomPass {
    /// Called once per frame before anything is drawn, with every command for this pass in order.
    /// `viewport` is the target's size in device pixels and `scale` the device pixels per point.
    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, viewport: [f32; 2], scale: f32, commands: &[&CustomCmd]);

    /// Draws the `index`-th of this frame's commands.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, index: usize);
}
