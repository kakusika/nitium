// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore blits DEXT Parameteriv
use std::cell::RefCell;

use ash::vk;
use glow::HasContext;
use winit::dpi::PhysicalSize;

use crate::gl_bindings as gl;
use gl::Gles2 as Gl;
use slint::wgpu_30::wgpu::{self, hal};

#[derive(thiserror::Error, Debug)]
pub enum VulkanTextureError {
    #[error("{0:?}")]
    Surfman(surfman::Error),
    #[error("{0}")]
    Vulkan(#[from] ash::vk::Result),
    #[error("No surface returned when the surface was unbound from the context")]
    NoSurface,
    #[error("The surface didn't have a framebuffer object")]
    NoFramebuffer,
    #[error("Wgpu is not using the vulkan backend")]
    WgpuNotVulkan,
    #[error("{0}")]
    OpenGL(String),
}

/// Size-dependent Vulkan and OpenGL resources reused across frames.
pub struct VulkanSizeDependentState {
    pub vulkan_image: vk::Image,
    pub vulkan_memory: vk::DeviceMemory,
    pub gl_memory_object: u32,
    pub gl_texture: glow::Texture,
    pub gl_draw_framebuffer: glow::Framebuffer,
    pub wgpu_texture: wgpu::Texture,
    pub ash_device: ash::Device,
    pub size: PhysicalSize<u32>,
}

impl VulkanSizeDependentState {
    pub fn destroy(
        self,
        device: &surfman::Device,
        context: &mut surfman::Context,
        gl: &glow::Context,
    ) {
        let _ = device.make_context_current(context);
        unsafe {
            gl.delete_framebuffer(self.gl_draw_framebuffer);
            gl.delete_texture(self.gl_texture);
            let gl_with_extensions =
                Gl::load_with(|function_name| device.get_proc_address(context, function_name));
            gl_with_extensions.DeleteMemoryObjectsEXT(1, &self.gl_memory_object);
            // Drop wgpu_texture first before destroying Vulkan image/memory
            drop(self.wgpu_texture);
            self.ash_device.destroy_image(self.vulkan_image, None);
            self.ash_device.free_memory(self.vulkan_memory, None);
        }
    }
}

/// Shared Vulkan state owning size-dependent cached resources.
pub struct VulkanSharedState {
    pub size_dependent: RefCell<Option<VulkanSizeDependentState>>,
}

impl VulkanSharedState {
    pub fn new() -> Self {
        Self {
            size_dependent: RefCell::new(None),
        }
    }

    pub fn destroy(&self, device: &surfman::Device, context: &mut surfman::Context) {
        if let Some(state) = self.size_dependent.borrow_mut().take() {
            let gl = unsafe {
                glow::Context::from_loader_function(|name| device.get_proc_address(context, name))
            };
            state.destroy(device, context, &gl);
        }
    }
}

impl super::GPURenderingContext {
    fn init_vulkan_size_dependent_state(
        &self,
        device: &surfman::Device,
        context: &surfman::Context,
        wgpu_device: &wgpu::Device,
        size: PhysicalSize<u32>,
    ) -> Result<VulkanSizeDependentState, VulkanTextureError> {
        let gl = &self.surfman_rendering_info.glow_gl;

        unsafe {
            let hal_device = wgpu_device
                .as_hal::<wgpu::wgc::api::Vulkan>()
                .ok_or(VulkanTextureError::WgpuNotVulkan)?;
            let vulkan_ctx = origiri_viewport::VulkanDeviceContext::from_wgpu(wgpu_device)
                .map_err(|_| VulkanTextureError::WgpuNotVulkan)?;
            let vulkan_device = &vulkan_ctx.ash_device;
            let vulkan_instance = &vulkan_ctx.raw_instance;

            // 1. Create Vulkan image with external memory for sharing with OpenGL
            let mut external_memory_image_info = vk::ExternalMemoryImageCreateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);

            let vulkan_image = vulkan_device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: size.width,
                        height: size.height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut external_memory_image_info),
                None,
            )?;

            // 2. Allocate dedicated Vulkan memory and bind to the created image
            let memory_requirements = vulkan_device.get_image_memory_requirements(vulkan_image);
            let memory_type_index = origiri_viewport::find_memory_type_index(
                &vulkan_ctx.memory_properties,
                memory_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .unwrap_or(0);

            let mut dedicated_allocate_info =
                vk::MemoryDedicatedAllocateInfo::default().image(vulkan_image);

            let mut export_info = vk::ExportMemoryAllocateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD);

            let memory = vulkan_device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(memory_requirements.size)
                    .memory_type_index(memory_type_index)
                    .push_next(&mut dedicated_allocate_info)
                    .push_next(&mut export_info),
                None,
            )?;

            vulkan_device.bind_image_memory(vulkan_image, memory, 0)?;

            // 3. Export Vulkan memory as a file descriptor for OpenGL import
            let external_memory_fd_api =
                ash::khr::external_memory_fd::Device::new(vulkan_instance, vulkan_device);

            let memory_handle = external_memory_fd_api.get_memory_fd(
                &vk::MemoryGetFdInfoKHR::default()
                    .memory(memory)
                    .handle_type(vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD),
            )?;

            // 4. Import Vulkan memory into OpenGL using EXT_external_objects
            let gl_with_extensions =
                Gl::load_with(|function_name| device.get_proc_address(context, function_name));

            let mut memory_object = 0;
            gl_with_extensions.CreateMemoryObjectsEXT(1, &mut memory_object);
            gl_with_extensions.MemoryObjectParameterivEXT(
                memory_object,
                gl::DEDICATED_MEMORY_OBJECT_EXT,
                &1,
            );
            gl_with_extensions.ImportMemoryFdEXT(
                memory_object,
                memory_requirements.size,
                gl::HANDLE_TYPE_OPAQUE_FD_EXT,
                memory_handle,
            );

            // 5. Create GL texture bound to imported memory
            let gl_texture = gl.create_texture().map_err(VulkanTextureError::OpenGL)?;
            gl.bind_texture(gl::TEXTURE_2D, Some(gl_texture));
            gl_with_extensions.TexStorageMem2DEXT(
                gl::TEXTURE_2D,
                1,
                gl::RGBA8,
                size.width as i32,
                size.height as i32,
                memory_object,
                0,
            );

            // 6. Create GL draw framebuffer targeting this texture
            let draw_framebuffer = gl.create_framebuffer().map_err(VulkanTextureError::OpenGL)?;
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, Some(draw_framebuffer));
            gl.framebuffer_texture_2d(
                gl::DRAW_FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                Some(gl_texture),
                0,
            );
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, None);

            // 7. Wrap Vulkan image into WGPU texture
            let wgpu_texture = wgpu_device.create_texture_from_hal::<wgpu::wgc::api::Vulkan>(
                hal_device.texture_from_raw(
                    vulkan_image,
                    &hal::TextureDescriptor {
                        label: Some("nitium_vulkan_shared_texture"),
                        size: wgpu::Extent3d {
                            width: size.width,
                            height: size.height,
                            depth_or_array_layers: 1,
                        },
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        dimension: wgpu::TextureDimension::D2,
                        mip_level_count: 1,
                        sample_count: 1,
                        usage: wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COLOR_TARGET,
                        view_formats: Vec::new(),
                        memory_flags: hal::MemoryFlags::empty(),
                    },
                    None,
                    hal::vulkan::TextureMemory::External,
                ),
                &wgpu::TextureDescriptor {
                    label: Some("nitium_wgpu_vulkan_texture"),
                    size: wgpu::Extent3d {
                        width: size.width,
                        height: size.height,
                        depth_or_array_layers: 1,
                    },
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    dimension: wgpu::TextureDimension::D2,
                    mip_level_count: 1,
                    sample_count: 1,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
                wgpu::TextureUses::UNINITIALIZED,
            );

            Ok(VulkanSizeDependentState {
                vulkan_image,
                vulkan_memory: memory,
                gl_memory_object: memory_object,
                gl_texture,
                gl_draw_framebuffer: draw_framebuffer,
                wgpu_texture,
                ash_device: vulkan_device.clone(),
                size,
            })
        }
    }

    /// Imports Vulkan surface as a WGPU texture for rendering on Linux.
    /// Reuses existing size-dependent external memory and blits content using FBO.
    pub fn get_wgpu_texture_from_vulkan(
        &self,
        wgpu_device: &wgpu::Device,
    ) -> Result<wgpu::Texture, VulkanTextureError> {
        let size = self.size.get();

        let device = &self.surfman_rendering_info.device.borrow();
        let mut context = self.surfman_rendering_info.context.borrow_mut();

        // 1. Check if we need to recreate size-dependent resources
        let needs_recreate = self.vulkan_state.size_dependent.borrow().as_ref().map_or(
            true,
            |s| s.size.width != size.width || s.size.height != size.height,
        );

        if needs_recreate {
            if let Some(old_state) = self.vulkan_state.size_dependent.borrow_mut().take() {
                old_state.destroy(device, &mut context, &self.surfman_rendering_info.glow_gl);
            }
            let new_state =
                self.init_vulkan_size_dependent_state(device, &context, wgpu_device, size)?;
            *self.vulkan_state.size_dependent.borrow_mut() = Some(new_state);
        }

        let size_dep = self.vulkan_state.size_dependent.borrow();
        let state = size_dep.as_ref().unwrap();

        // 2. Unbind surface from context to access its surface_info and blit
        let surface = device
            .unbind_surface_from_context(&mut context)
            .map_err(VulkanTextureError::Surfman)?
            .ok_or(VulkanTextureError::NoSurface)?;

        device
            .make_context_current(&mut context)
            .map_err(VulkanTextureError::Surfman)?;

        let surface_info = device.surface_info(&surface);
        let read_framebuffer = surface_info
            .framebuffer_object
            .ok_or(VulkanTextureError::NoFramebuffer)?;

        // 3. Blit from Servo's framebuffer into our cached GL draw framebuffer
        let gl = &self.surfman_rendering_info.glow_gl;
        unsafe {
            gl.bind_framebuffer(gl::READ_FRAMEBUFFER, Some(read_framebuffer));
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, Some(state.gl_draw_framebuffer));

            gl.blit_framebuffer(
                0,
                0,
                size.width as i32,
                size.height as i32,
                0,
                size.height as i32,
                size.width as i32,
                0,
                gl::COLOR_BUFFER_BIT,
                gl::NEAREST,
            );
            gl.flush();

            gl.bind_framebuffer(gl::READ_FRAMEBUFFER, None);
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, None);
        }

        // 4. Re-bind surface to context
        device
            .bind_surface_to_context(&mut context, surface)
            .map_err(|(err, mut surface)| {
                let _ = device.destroy_surface(&mut context, &mut surface);
                VulkanTextureError::Surfman(err)
            })?;

        let wgpu_texture = state.wgpu_texture.clone();
        drop(size_dep);
        Ok(wgpu_texture)
    }
}
