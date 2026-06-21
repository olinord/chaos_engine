use crate::ecs::system::ChaosSystem;
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
    fn initialize(
        &mut self,
        _component_manager: &mut crate::ecs::component::ChaosComponentManager,
    ) -> Result<(), &'static str> {
        Ok(())
    }

    fn update(
        &mut self,
        _delta_time: f32,
        _component_manager: &mut crate::ecs::component::ChaosComponentManager,
    ) -> Result<(), &'static str> {
        for trigger in &mut self.triggers {
            trigger.update(_delta_time);
        }
        Ok(())
    }
}
