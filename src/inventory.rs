use core::panic;
use std::cmp::min;

use crate::{
    attributes::{AttributeModifier, ItemAttributes},
    item::{ItemDisplayMetaData, WorldObject},
    ui::InventorySlotState,
    GameState, TIME_STEP,
};
use bevy::{prelude::*, time::FixedTimestep};

pub const INVENTORY_SIZE: usize = 6 * 4;
pub const INVENTORY_INIT: Option<InventoryItemStack> = None;
pub const MAX_STACK_SIZE: usize = 16;

#[derive(Component, Debug, Clone)]
pub struct Inventory {
    pub items: [Option<InventoryItemStack>; INVENTORY_SIZE],
}
pub struct InventoryPlugin;

#[derive(Component, Debug, PartialEq, Clone)]
pub struct ItemStack {
    pub obj_type: WorldObject,
    pub count: usize,
    pub attributes: ItemAttributes,
    pub metadata: ItemDisplayMetaData,
}

#[derive(Component, Debug, PartialEq, Clone)]

pub struct InventoryItemStack {
    pub item_stack: ItemStack,
    pub slot: usize,
}

#[derive(Debug)]
pub enum InventoryError {
    FailedToMerge(String),
}
impl InventoryItemStack {
    pub fn add_to_inventory(
        &self,
        inv: &mut Query<&mut Inventory>,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        inv.single_mut().items[self.slot] = Some(self.clone());
        InventoryPlugin::mark_slot_dirty(self.slot, inv_slots);
    }
    pub fn remove_from_inventory(self, mut inv: Query<&mut Inventory>) {
        inv.single_mut().items[self.slot] = None
    }
    pub fn modify_attributes(
        &self,
        modifier: AttributeModifier,
        inv: &mut Query<&mut Inventory>,
    ) -> Self {
        let new_item_stack = self
            .item_stack
            .clone()
            .get_copy_with_modified_attributes(modifier);

        let inv_stack = Self {
            item_stack: new_item_stack,
            slot: 0,
        };
        inv.single_mut().items[0] = Some(inv_stack.clone());

        inv_stack
    }
    pub fn modify_count(&mut self, amount: i8) -> Option<Self> {
        self.item_stack.modify_count(amount);
        if self.item_stack.count == 0 {
            return None;
        }
        Some(self.clone())
    }
}
//TODO: abstract all these behind a AddItemToInventoryEvent ? let event drive info needed for sub-fns
impl ItemStack {
    pub fn copy_with_attributes(&self, attributes: &ItemAttributes) -> Self {
        Self {
            obj_type: self.obj_type,
            count: self.count,
            attributes: attributes.clone(),
            metadata: ItemDisplayMetaData {
                name: self.metadata.name.clone(),
                desc: self.metadata.desc.clone(),
                attributes: attributes.clone().get_tooltips(),
                durability: attributes.get_durability_tooltip(),
            },
        }
    }
    pub fn add_to_inventory(
        self,
        inv: &mut Query<&mut Inventory>,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        // if stack of that item exists, add to it, otherwise push as new stack.
        // TODO: add max stack size, and create new stack if reached.
        // TODO: abstract direct access of .obj_type behind a getter
        if let Some(stack) = inv.single_mut().items.iter().find(|i| match i {
            Some(ii) if ii.item_stack.count < MAX_STACK_SIZE => {
                ii.item_stack.obj_type == self.obj_type
                    && ii.item_stack.attributes == self.attributes
            }
            _ => false,
        }) {
            // safe to unwrap, we check for it above
            let slot = stack.clone().unwrap().slot;
            let pre_stack_size = inv.single().items[slot].clone().unwrap().item_stack.count;

            inv.single_mut().items[slot] = Some(InventoryItemStack {
                item_stack: Self {
                    obj_type: self.obj_type,
                    metadata: self.metadata.clone(),
                    attributes: self.attributes.clone(),
                    count: min(self.count + pre_stack_size, MAX_STACK_SIZE),
                },
                slot,
            });
            InventoryPlugin::mark_slot_dirty(slot, inv_slots);

            if pre_stack_size + self.count > MAX_STACK_SIZE {
                Self::add_to_empty_inventory_slot(
                    Self {
                        obj_type: self.obj_type,
                        metadata: self.metadata.clone(),
                        attributes: self.attributes.clone(),
                        count: pre_stack_size + self.count - MAX_STACK_SIZE,
                    },
                    inv,
                    inv_slots,
                );
            }
        } else {
            Self::add_to_empty_inventory_slot(self, inv, inv_slots);
        }
    }
    pub fn add_to_empty_inventory_slot(
        self,
        inv: &mut Query<&mut Inventory>,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        let slot = InventoryPlugin::get_first_empty_slot(inv);
        if let Some(slot) = slot {
            let item = InventoryItemStack {
                item_stack: self,
                slot,
            };
            item.add_to_inventory(inv, inv_slots);
        }
    }
    pub fn try_add_to_target_inventory_slot(
        self,
        slot: usize,
        inv: &mut Query<&mut Inventory>,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) -> Result<(), InventoryError> {
        if let Some(mut existing_stack) = inv.single_mut().items[slot].clone() {
            if existing_stack.item_stack.obj_type == self.obj_type {
                existing_stack.modify_count(self.count as i8);
                return Ok(());
            }
            Err(InventoryError::FailedToMerge(
                "Target item stack is not the same WorldObject type.".to_string(),
            ))
        } else {
            let item = InventoryItemStack {
                item_stack: self,
                slot,
            };
            item.add_to_inventory(inv, inv_slots);
            Ok(())
        }
    }
    pub fn split(self) -> (usize, usize) {
        let split_count = self.count / 2;
        (self.count - split_count, split_count)
    }
    pub fn get_copy_with_modified_attributes(&mut self, modifier: AttributeModifier) -> Self {
        self.clone()
            .copy_with_attributes(self.attributes.change_attribute(modifier))
    }
    pub fn modify_count(&mut self, amount: i8) -> Self {
        if (self.count as i8) + amount <= 0 {
            self.count = 0;
        } else {
            self.count = ((self.count as i8) + amount) as usize;
        }
        self.clone()
    }
}

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_update(GameState::Main)
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64)), // .with_system(Self::update_inventory),
        );
    }
}

impl InventoryPlugin {
    // get the lowest slot number occupied

    pub fn get_first_empty_slot(inv: &Query<&mut Inventory>) -> Option<usize> {
        //TODO: maybe move the actual inv to a type in this file, and move this fn into that struct
        (0..INVENTORY_SIZE).find(|&i| inv.single().items[i].is_none())
    }
    /// Attempt to merge item at slot a into b. Panics if
    /// either slot is empty, or not matching WorldObject types.
    /// Keeps remainder where it was, if overflow.
    pub fn merge_item_stacks(
        to_merge: ItemStack,
        merge_into: InventoryItemStack,
        inv: &mut Query<&mut Inventory>,
    ) -> Option<ItemStack> {
        let item_type = to_merge.obj_type;
        //TODO: should this return  None, or the original stack??
        println!(
            "{:?} |||| {:?}",
            merge_into.item_stack.metadata, to_merge.metadata
        );
        if item_type != merge_into.item_stack.obj_type
            || merge_into.item_stack.metadata != to_merge.metadata
        {
            return None;
        }
        let item_a_count = to_merge.count;
        let item_b_count = merge_into.item_stack.count;
        let combined_size = item_a_count + item_b_count;

        inv.single_mut().items[merge_into.slot] = Some(InventoryItemStack {
            item_stack: ItemStack {
                obj_type: item_type,
                metadata: to_merge.metadata.clone(),
                attributes: to_merge.attributes.clone(),
                count: min(combined_size, MAX_STACK_SIZE),
            },
            slot: merge_into.slot,
        });

        // if we overflow, keep remainder where it was
        if combined_size > MAX_STACK_SIZE {
            return Some(ItemStack {
                obj_type: item_type,
                metadata: to_merge.metadata.clone(),
                attributes: to_merge.attributes.clone(),
                count: combined_size - MAX_STACK_SIZE,
            });
        }

        None
    }
    fn swap_items(
        item: ItemStack,
        target_slot: usize,
        inv: &mut Query<&mut Inventory>,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) -> ItemStack {
        let target_item_option = inv.single().items[target_slot].clone();
        if let Some(target_item_stack) = target_item_option {
            inv.single_mut().items[target_slot] = Some(InventoryItemStack {
                item_stack: item,
                slot: target_item_stack.slot,
            });
            Self::mark_slot_dirty(target_slot, inv_slots);
            return target_item_stack.item_stack;
        }
        item
    }
    pub fn drop_item_on_slot(
        item: ItemStack,
        drop_slot: usize,
        inv: &mut Query<&mut Inventory>,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) -> Option<ItemStack> {
        let obj_type = item.obj_type;
        let target_item_option = inv.single().items[drop_slot].clone();
        if let Some(target_item) = target_item_option {
            if target_item.item_stack.obj_type == obj_type
                && target_item.item_stack.metadata == item.metadata
            {
                Self::mark_slot_dirty(drop_slot, inv_slots);
                return Self::merge_item_stacks(item, target_item, inv);
            } else {
                return Some(Self::swap_items(item, drop_slot, inv, inv_slots));
            }
        } else if item
            .try_add_to_target_inventory_slot(drop_slot, inv, inv_slots)
            .is_err()
        {
            panic!("Failed to drop item on stot");
        }

        None
    }

    // to split a stack, we right click on an existing stack.
    // we do not know where the target stack is, and since the current stack
    // is not moving, we are creating a new entity visual to drag
    // the drag
    pub fn split_stack(
        item_stack: ItemStack,
        item_slot: usize,
        item_slot_state: &mut InventorySlotState,
        inv: &mut Query<&mut Inventory>,
    ) -> ItemStack {
        let (amount_split, remainder_left) = item_stack.clone().split();
        inv.single_mut().items[item_slot] = if remainder_left > 0 {
            Some(InventoryItemStack {
                item_stack: ItemStack {
                    obj_type: item_stack.obj_type.clone(),
                    metadata: item_stack.metadata.clone(),
                    attributes: item_stack.attributes.clone(),
                    count: remainder_left,
                },
                slot: item_slot,
            })
        } else {
            None
        };
        item_slot_state.dirty = true;
        ItemStack {
            obj_type: item_stack.obj_type.clone(),
            metadata: item_stack.metadata.clone(),
            attributes: item_stack.attributes.clone(),
            count: amount_split,
        }
    }
    //TODO: Maybe make a resource to instead store slot indexs, and then mark them all dirty in a system?
    // benefit: dont need to pass in the inv slot query anymore
    pub fn mark_slot_dirty(slot_index: usize, inv_slots: &mut Query<&mut InventorySlotState>) {
        for mut state in inv_slots.iter_mut() {
            if state.slot_index == slot_index {
                state.dirty = true;
            }
        }
    }
}
