/*
The input strategy is this:
- receive input information from winit
- register input events without details in the manager
- Convert input information from winit to chaos specific events (this button was pressed etc)
- Keep a track of which buttons were pressed (at which time) or released (at which time) so we can properly handle double, triple, ... clicks
 */

pub mod manager;
pub mod events;