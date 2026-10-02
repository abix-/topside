//! Outlines (operator, 2026-10-01): a dark line wherever the depth or the
//! way a surface faces jumps, so every wall, corner, step and doorway
//! reads, low poly as Borderlands and Sable draw them. One full-screen
//! pass after the scene and before tonemapping, reading the depth and
//! normal prepasses; run the way Bevy's own `FullscreenMaterial`
//! (bevy_core_pipeline fullscreen_material.rs) runs its pass, with those
//! two textures bound as well. assets/shaders/outline.wgsl draws it.

use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3d, Core3dSystems, FullscreenShader};
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_depth_2d};
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId, ColorTargetState,
    ColorWrites, FragmentState, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, TextureFormat,
    TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::ViewTarget;
use bevy::render::{RenderApp, RenderStartup};

/// On a camera: its view is outlined. The camera needs the depth and
/// normal prepasses (`DepthPrepass`, `NormalPrepass`) and no MSAA.
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct Outlined;

pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<Outlined>::default());
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Core3d, outline.in_set(Core3dSystems::PostProcess).before(tonemapping));
    }
}

/// The pass's pipeline, its bind group layout, and the screen's sampler.
#[derive(Resource)]
struct OutlinePipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    id: CachedRenderPipelineId,
}

fn init_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "outline_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_depth_2d(),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("outline_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load("shaders/outline.wgsl"),
            // The camera is not HDR: its main texture is sRGB, 8 bits.
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(OutlinePipeline { layout, sampler, id });
}

/// Each outlined view: the scene read, the lines drawn over it, written to
/// the other main texture (`post_process_write`).
fn outline(
    view: ViewQuery<(&ViewTarget, &ViewPrepassTextures), With<Outlined>>,
    outline: Option<Res<OutlinePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let Some(outline) = outline else {
        return;
    };
    let (view_target, prepass) = view.into_inner();
    let (Some(pipeline), Some(depth), Some(normal)) = (pipeline_cache.get_render_pipeline(outline.id), prepass.depth_view(), prepass.normal_view()) else {
        return;
    };
    let post_process = view_target.post_process_write();
    let bind_group = render_device.create_bind_group(
        "outline_bind_group",
        &pipeline_cache.get_bind_group_layout(&outline.layout),
        &BindGroupEntries::sequential((post_process.source, &outline.sampler, depth, normal)),
    );
    let pass = RenderPassDescriptor {
        label: Some("outline_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post_process.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };
    let mut render_pass = ctx.command_encoder().begin_render_pass(&pass);
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}
