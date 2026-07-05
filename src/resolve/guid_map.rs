use std::collections::HashMap;

pub struct GuidMap {
    guid_to_file_id: HashMap<String, i64>,
    guid_to_rel_path: HashMap<String, String>,
}

impl Default for GuidMap {
    fn default() -> Self {
        Self::new()
    }
}

impl GuidMap {
    pub fn new() -> Self {
        GuidMap {
            guid_to_file_id: HashMap::new(),
            guid_to_rel_path: HashMap::new(),
        }
    }

    pub fn insert(&mut self, guid: &str, file_id: i64, rel_path: &str) {
        self.guid_to_file_id.insert(guid.to_string(), file_id);
        self.guid_to_rel_path
            .insert(guid.to_string(), rel_path.to_string());
    }

    pub fn lookup_file_id(&self, guid: &str) -> Option<i64> {
        self.guid_to_file_id.get(guid).copied()
    }

    pub fn lookup_rel_path(&self, guid: &str) -> Option<&str> {
        self.guid_to_rel_path.get(guid).map(|s| s.as_str())
    }
}
