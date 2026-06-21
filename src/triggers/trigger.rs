use crate::triggers::conditions::TriggerCondition;

pub struct Trigger {
    conditions: Vec<Box<dyn TriggerCondition>>,
    callback: Box<dyn FnMut()>,
}

impl Trigger {
    pub fn new(conditions: Vec<Box<dyn TriggerCondition>>, callback: Box<dyn FnMut()>) -> Self {
        Trigger {
            conditions,
            callback,
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        for condition in &self.conditions {
            if !condition.check_condition(delta_time) {
                return;
            }
        }

        (self.callback)();
    }
}

pub struct TriggerBuilder {
    conditions: Vec<Box<dyn TriggerCondition>>,
    callback: Option<Box<dyn FnMut()>>,
}

impl TriggerBuilder {
    pub fn new() -> Self {
        TriggerBuilder {
            conditions: Vec::new(),
            callback: None,
        }
    }
    pub fn with_condition(mut self, condition: Box<dyn TriggerCondition>) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn with_callback(mut self, callback: Box<dyn FnMut()>) -> Self {
        self.callback = Some(callback);
        self
    }

    pub fn build(self) -> Trigger {
        Trigger::new(
            self.conditions,
            self.callback.expect("Callback must be set"),
        )
    }
}
