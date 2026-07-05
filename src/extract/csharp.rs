use tree_sitter::{Language, Node, Parser};

#[derive(Debug, Clone)]
pub struct CsDeclaration {
    pub decl_kind: String,
    pub simple_name: String,
    pub qualified_name: String,
    pub signature: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone)]
pub struct CsMention {
    pub mention_kind: String,
    pub text: String,
    pub receiver_text: Option<String>,
    pub containing_declaration: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
}

pub struct CsExtractionResult {
    pub declarations: Vec<CsDeclaration>,
    pub mentions: Vec<CsMention>,
}

const DECL_TYPE_MAP: &[(&str, &str)] = &[
    ("class_declaration", "class"),
    ("struct_declaration", "struct"),
    ("interface_declaration", "interface"),
    ("enum_declaration", "enum"),
    ("namespace_declaration", "namespace"),
    ("method_declaration", "method"),
    ("property_declaration", "property"),
    ("constructor_declaration", "constructor"),
    ("delegate_declaration", "delegate"),
    ("event_declaration", "event"),
    ("event_field_declaration", "event_field"),
    ("field_declaration", "field"),
    ("record_declaration", "record"),
];

pub fn extract_from_csharp(source: &str) -> Option<CsExtractionResult> {
    let mut parser = Parser::new();
    let language: Language = tree_sitter_c_sharp::LANGUAGE.into();
    parser.set_language(&language).ok()?;

    let tree = parser.parse(source, None)?;
    let root = tree.root_node();

    let mut declarations = Vec::new();
    let mut mentions = Vec::new();

    walk_node(root, source, &[], None, &mut declarations, &mut mentions);

    Some(CsExtractionResult {
        declarations,
        mentions,
    })
}

fn walk_node(
    node: Node,
    source: &str,
    scope_path: &[String],
    containing_decl: Option<&str>,
    declarations: &mut Vec<CsDeclaration>,
    mentions: &mut Vec<CsMention>,
) {
    let node_kind = node.kind();

    // Check if this is a declaration
    let decl_info = DECL_TYPE_MAP
        .iter()
        .find(|(ts_kind, _)| *ts_kind == node_kind);

    let mut current_scope = scope_path.to_vec();
    let mut current_containing: Option<String> = containing_decl.map(|s| s.to_string());

    if let Some((_, kind_name)) = decl_info {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name_text = name_node
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .to_string();
            let simple_name = name_text
                .split('.')
                .next_back()
                .unwrap_or(&name_text)
                .to_string();
            let qualified = if scope_path.is_empty() {
                name_text.clone()
            } else {
                format!("{}.{}", scope_path.join("."), name_text)
            };

            let signature = node
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .to_string();

            declarations.push(CsDeclaration {
                decl_kind: kind_name.to_string(),
                simple_name: simple_name.clone(),
                qualified_name: qualified.clone(),
                signature,
                line_start: node.start_position().row + 1,
                line_end: node.end_position().row + 1,
            });

            current_scope.push(name_text);
            current_containing = Some(simple_name);
        }
    }

    // Check for mentions (identifier references)
    if node_kind == "identifier" {
        let parent = node.parent();
        let should_record = should_record_mention(node, parent);

        if should_record {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
            let receiver = parent
                .filter(|p| p.kind() == "member_access_expression")
                .and_then(|p| p.child_by_field_name("expression"))
                .map(|e| e.utf8_text(source.as_bytes()).unwrap_or("").to_string());

            mentions.push(CsMention {
                mention_kind: "identifier".to_string(),
                text,
                receiver_text: receiver,
                containing_declaration: current_containing.clone(),
                line_start: node.start_position().row + 1,
                line_end: node.end_position().row + 1,
            });
        }
    } else if node_kind == "qualified_name" || node_kind == "member_access_expression" {
        // Record qualified name mentions
        let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        let receiver = if node_kind == "member_access_expression" {
            node.child_by_field_name("expression")
                .map(|e| e.utf8_text(source.as_bytes()).unwrap_or("").to_string())
        } else {
            None
        };

        if !text.is_empty() && !is_declaration_name(node) {
            mentions.push(CsMention {
                mention_kind: "qualified-name".to_string(),
                text,
                receiver_text: receiver,
                containing_declaration: current_containing.clone(),
                line_start: node.start_position().row + 1,
                line_end: node.end_position().row + 1,
            });
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(
            child,
            source,
            &current_scope,
            current_containing.as_deref(),
            declarations,
            mentions,
        );
    }
}

fn should_record_mention(node: Node, parent: Option<Node>) -> bool {
    let parent = match parent {
        Some(p) => p,
        None => return true,
    };

    // Skip if this identifier is a declaration name
    if let Some(name_node) = parent.child_by_field_name("name") {
        if name_node.id() == node.id() {
            return false;
        }
    }

    // Skip if inside member_access_expression or qualified_name (those are recorded separately)
    let parent_kind = parent.kind();
    if parent_kind == "member_access_expression" || parent_kind == "qualified_name" {
        return false;
    }

    true
}

fn is_declaration_name(node: Node) -> bool {
    if let Some(parent) = node.parent() {
        if let Some(name_node) = parent.child_by_field_name("name") {
            return name_node.id() == node.id();
        }
    }
    false
}
