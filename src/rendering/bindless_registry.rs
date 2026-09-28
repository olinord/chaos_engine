use crate::utils::result::unwrap_or_panic;
use std::collections::VecDeque;
use std::sync::Arc;
use vulkano::descriptor_set::{
    DescriptorSet, WriteDescriptorSet,
    allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo},
    layout::{DescriptorSetLayout, DescriptorSetLayoutCreateInfo},
};

use crate::rendering::{context::ChaosRenderContext, image::ChaosImage, sampler::ChaosSampler};

pub struct BindlessRegistry {
    descriptor_set: Arc<DescriptorSet>, // the single persistent update-after-bind set
    set_layout: Arc<DescriptorSetLayout>,

    slots: Vec<Option<BindlessSlot>>, // dense, index == bindless slot id; holds the strong refs
    generations: Vec<usize>,          // bumped every free/reuse, to catch stale handles
    free_list: Vec<usize>,            // recycled indices ready for reuse
    next_unused: usize,               // watermark if free_list is empty

    pending_writes: Vec<WriteDescriptorSet>, // batched, flushed once per frame before recording
    pending_frees: VecDeque<(usize, usize)>, // (frame_submitted, slot) — held until GPU is done
}

// Bindless Slot, just a memory storage structure to hold strong references to the image and sampler
#[derive(Clone, Debug)]
struct BindlessSlot {
    #[allow(dead_code)]
    image: Arc<ChaosImage>,
    #[allow(dead_code)]
    sampler: Arc<ChaosSampler>,
}

#[derive(Clone, Debug)]
pub struct BindlessSlotHandle {
    pub index: usize,
    pub generation: usize,
}

const DEFAULT_INITIAL_CAPACITY: usize = 128;
const DEFAULT_INCREMENT: usize = 32;

impl BindlessRegistry {
    pub fn new(render_context: &Arc<ChaosRenderContext>, capacity: Option<usize>) -> Self {
        let descriptor_layout_creation_info = DescriptorSetLayoutCreateInfo::default();
        let device = render_context.device();
        let descriptor_set_allocator_create_info =
            StandardDescriptorSetAllocatorCreateInfo::default();
        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
            device.clone(),
            descriptor_set_allocator_create_info,
        ));

        let descriptor_layout = unwrap_or_panic(
            DescriptorSetLayout::new(device.clone(), descriptor_layout_creation_info),
            "BindlessRegistry: Failed to create descriptor set layout",
        );

        let descriptor_set = unwrap_or_panic(
            DescriptorSet::new(
                descriptor_set_allocator.clone(),
                descriptor_layout.clone(),
                vec![],
                vec![],
            ),
            "BindlessRegistry: Failed to create descriptor set",
        );

        Self {
            descriptor_set: descriptor_set.clone(),
            set_layout: descriptor_layout.clone(),

            slots: Vec::with_capacity(capacity.unwrap_or(DEFAULT_INITIAL_CAPACITY)),
            generations: Vec::with_capacity(capacity.unwrap_or(DEFAULT_INITIAL_CAPACITY)),
            free_list: Vec::new(),
            next_unused: 0,

            pending_writes: Vec::new(),
            pending_frees: VecDeque::new(),
        }
    }

    pub fn register_image(
        &mut self,
        image: Arc<ChaosImage>,
        sampler: Arc<ChaosSampler>,
    ) -> BindlessSlotHandle {
        let slot_index = match self.free_list.pop() {
            Some(index) => index,
            None => {
                let index = self.next_unused;
                self.next_unused += 1;
                index
            }
        };

        if slot_index >= self.slots.len() {
            self.slots.resize(slot_index + DEFAULT_INCREMENT, None);
        }
        if slot_index >= self.generations.len() {
            self.generations.resize(slot_index + DEFAULT_INCREMENT, 0);
        }

        // increment the generation of the slot
        let generation = self.generations.get(slot_index).cloned().unwrap_or(0) + 1;
        self.generations[slot_index] = generation;
        // store the image and sampler so they don't get dropped prematurely
        // write the write descriptor set
        self.pending_writes.push(WriteDescriptorSet::image_view(
            slot_index as u32,
            image.image_view(),
        ));

        self.slots[slot_index] = Some(BindlessSlot { image, sampler });

        BindlessSlotHandle {
            index: slot_index,
            generation,
        }
    }

    pub fn free(
        &mut self,
        render_context: &Arc<ChaosRenderContext>,
        slot_handle: BindlessSlotHandle,
    ) {
        if let Some(generation) = self.generations.get_mut(slot_handle.index) {
            if *generation == slot_handle.generation {
                *generation += 1;
                self.free_list.push(slot_handle.index);
            }
        }

        self.pending_frees
            .push_back((render_context.frame_index(), slot_handle.index));
    }

    pub fn is_valid(&self, slot_handle: BindlessSlotHandle) -> bool {
        if let Some(generation) = self.generations.get(slot_handle.index) {
            *generation == slot_handle.generation
        } else {
            false
        }
    }

    pub fn flush(&mut self, render_context: &Arc<ChaosRenderContext>) {
        // flush pending writes to the descriptor set
        unwrap_or_panic(
            unsafe {
                self.descriptor_set
                    .update_by_ref(self.pending_writes.drain(..), vec![])
            },
            "Could not update descriptor set",
        );
        for (frame_submitted, slot_index) in self.pending_frees.iter() {
            if *frame_submitted < render_context.frame_index() {
                self.slots[*slot_index] = None;
            }
        }

        self.pending_frees
            .retain(|(frame_submitted, _)| *frame_submitted > render_context.frame_index());
    }

    pub fn descriptor_set(&self) -> Arc<DescriptorSet> {
        self.descriptor_set.clone()
    }
}
