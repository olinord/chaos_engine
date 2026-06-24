use std::sync::Arc;

use log::info;
use vulkano::{
    device::{
        DeviceExtensions, QueueFlags,
        physical::{PhysicalDevice, PhysicalDeviceType},
    },
    instance::Instance,
    swapchain::Surface,
};

pub fn select_physical_device(
    instance: &Arc<Instance>,
    surface: &Arc<Surface>,
    device_extensions: &DeviceExtensions,
) -> (Arc<PhysicalDevice>, u32) {
    info!("Selecting physical device...");
    // print out the devices and their properties
    for device in instance
        .enumerate_physical_devices()
        .expect("failed to enumerate physical devices")
    {
        let properties = device.properties();
        info!(
            "Device: {} (type: {:?}, api version: {}, driver version: {}, vendor ID: {}, device ID: {})",
            properties.device_name,
            properties.device_type,
            properties.api_version,
            properties.driver_version,
            properties.vendor_id,
            properties.device_id,
        );
    }

    let (physical_device, queue_family_index) = instance
        .enumerate_physical_devices()
        .expect("failed to enumerate physical devices")
        .filter(|p| p.supported_extensions().contains(device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.contains(QueueFlags::GRAPHICS)
                        && p.surface_support(i as u32, surface).unwrap_or(false)
                })
                .map(|q| (p, q as u32))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            _ => 4,
        })
        .expect("no device available");
    info!(
        "Using device: {} (type: {:?})",
        physical_device.properties().device_name,
        physical_device.properties().device_type,
    );

    (physical_device, queue_family_index)
}
