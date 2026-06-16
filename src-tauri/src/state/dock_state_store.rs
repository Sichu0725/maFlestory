use std::collections::HashMap;
use std::sync::Mutex;

use crate::models::DockStateSnapshot;

#[derive(Default)]
pub struct DockStateStore {
    pub entries: Mutex<HashMap<isize, DockStateSnapshot>>,
}
