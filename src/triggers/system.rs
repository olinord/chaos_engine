use crate::ecs::{system::ChaosSystem, world::ChaosWorld};
use crate::triggers::trigger::Trigger;

pub struct TriggerSystem {
    triggers: Vec<Trigger>,
}

impl TriggerSystem {
    pub fn new() -> Self {
        Self {
            triggers: Vec::new(),
        }
    }

    pub fn add_trigger(&mut self, trigger: Trigger) {
        self.triggers.push(trigger);
    }

    pub fn get_triggers(&self) -> &Vec<Trigger> {
        &self.triggers
    }
}

impl ChaosSystem for TriggerSystem {
    fn initialize(&mut self, _world: &mut ChaosWorld) -> Result<(), &'static str> {
        Ok(())
    }

    fn update(&mut self, world: &mut ChaosWorld) -> Result<(), &'static str> {
        let delta_time = world.get_time().delta_time();
        for trigger in &mut self.triggers {
            trigger.update(delta_time);
        }
        Ok(())
    }
}
