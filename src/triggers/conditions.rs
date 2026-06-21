pub trait TriggerCondition {
    fn check_condition(&self, delta_time: f32) -> bool;
}

pub struct AndCondition {
    conditions: Vec<Box<dyn TriggerCondition>>,
}

impl TriggerCondition for AndCondition {
    fn check_condition(&self, delta_time: f32) -> bool {
        for condition in &self.conditions {
            if !condition.check_condition(delta_time) {
                return false;
            }
        }
        true
    }
}

pub struct OrCondition {
    conditions: Vec<Box<dyn TriggerCondition>>,
}

impl TriggerCondition for OrCondition {
    fn check_condition(&self, delta_time: f32) -> bool {
        for condition in &self.conditions {
            if condition.check_condition(delta_time) {
                return true;
            }
        }
        false
    }
}
