use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AsmDefInfo {
    pub name: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub includePlatforms: Vec<String>,
    #[serde(default)]
    pub rootNamespace: Option<String>,
}

impl AsmDefInfo {
    pub fn is_editor_only(&self) -> bool {
        self.includePlatforms
            .iter()
            .any(|p| p.eq_ignore_ascii_case("editor"))
    }
}

pub fn parse_asmdef(content: &str) -> Option<AsmDefInfo> {
    serde_json::from_str(content).ok()
}
