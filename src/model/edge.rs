#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    ChildOf,
    DefinedIn,
    BindsTo,
    DependsOn,
    InstanceOf,
    Refs,
    Calls,
    ParentOf,
    Contains,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::ChildOf => "child_of",
            EdgeKind::DefinedIn => "defined_in",
            EdgeKind::BindsTo => "binds_to",
            EdgeKind::DependsOn => "depends_on",
            EdgeKind::InstanceOf => "instance_of",
            EdgeKind::Refs => "refs",
            EdgeKind::Calls => "calls",
            EdgeKind::ParentOf => "parent_of",
            EdgeKind::Contains => "contains",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "child_of" => Some(EdgeKind::ChildOf),
            "defined_in" => Some(EdgeKind::DefinedIn),
            "binds_to" => Some(EdgeKind::BindsTo),
            "depends_on" => Some(EdgeKind::DependsOn),
            "instance_of" => Some(EdgeKind::InstanceOf),
            "refs" => Some(EdgeKind::Refs),
            "calls" => Some(EdgeKind::Calls),
            "parent_of" => Some(EdgeKind::ParentOf),
            "contains" => Some(EdgeKind::Contains),
            _ => None,
        }
    }

    pub fn all_ref_edge_kinds() -> Vec<&'static str> {
        vec!["calls", "binds_to", "depends_on", "instance_of", "refs"]
    }
}

#[derive(Debug, Clone)]
pub struct EntityEdge {
    pub id: i64,
    pub from_entity_id: i64,
    pub to_entity_id: i64,
    pub edge_kind: EdgeKind,
    pub edge_subkind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VfsEntryType {
    File,
    Directory,
    Node,
    Link,
}

impl VfsEntryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            VfsEntryType::File => "file",
            VfsEntryType::Directory => "directory",
            VfsEntryType::Node => "node",
            VfsEntryType::Link => "link",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "file" => Some(VfsEntryType::File),
            "directory" => Some(VfsEntryType::Directory),
            "node" => Some(VfsEntryType::Node),
            "link" => Some(VfsEntryType::Link),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VfsEntry {
    pub id: i64,
    pub entry_type: VfsEntryType,
    pub entry_kind: String,
    pub vfs_path: String,
    pub parent_vfs_path: Option<String>,
    pub source_file_id: Option<i64>,
    pub source_entity_id: Option<i64>,
    pub display_name: String,
    pub content: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub target_vfs_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VfsEdge {
    pub id: i64,
    pub from_entry_id: i64,
    pub to_entry_id: i64,
    pub edge_kind: EdgeKind,
    pub edge_subkind: Option<String>,
}
