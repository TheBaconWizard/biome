use crate::{js_kinds_src::AstSrc, language_kind::LanguageKind};
use xtask_glue::Result;

pub fn generate_grit_mappings(ast: &AstSrc, language_kind: LanguageKind) -> Result<String> {
    let lang = LanguageConfig::new(language_kind);

    let legacy_patterns = lang.legacy_patterns;
    let native_patterns = ast
        .nodes
        .iter()
        // Filter out nodes that are lists or start with "Any"
        .filter(|node| !node.name.contains("List") && !node.name.starts_with("Any"))
        .map(|node| {
            format!(
                r#"        "{}" => lang::{}::KIND_SET.iter().next(),"#,
                node.name, node.name,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let header = "//! Maps GritQL pattern names to Biome's internal syntax kinds.";
    let has_legacy_patterns = !legacy_patterns.is_empty();
    let legacy_section = if has_legacy_patterns {
        format!(
            r#"
use crate::grit_target_language::SlotLiteral;

/// A legacy TreeSitter pattern for backward compatibility.
pub struct LegacyTreeSitterPattern {{
    pub name: &'static str,
    pub kind: {syntax_kind_type},
    pub slots: &'static [(&'static str, u32)],
    pub slot_parameters: &'static [(u32, SlotLiteral)],
}}

/// A list of legacy TreeSitter patterns for compatibility.
pub const LEGACY_TREESITTER_COMPATIBILITY_PATTERNS: &[LegacyTreeSitterPattern] = &[
{legacy_pattern_structs}
];

/// Returns the snake_case name for a syntax kind if it's part of the legacy set.
pub fn legacy_treesitter_name_for_kind(kind: {syntax_kind_type}) -> Option<&'static str> {{
    LEGACY_TREESITTER_COMPATIBILITY_PATTERNS
        .iter()
        .find(|p| p.kind == kind)
        .map(|p| p.name)
}}

/// Returns the slot mappings for a syntax kind if it's part of the legacy set.
pub fn legacy_treesitter_slots_for_kind(kind: {syntax_kind_type}) -> &'static [(&'static str, u32)] {{
    LEGACY_TREESITTER_COMPATIBILITY_PATTERNS
        .iter()
        .find(|p| p.kind == kind)
        .map_or(&[], |p| p.slots)
}}

/// Returns injected slot parameters for a legacy pattern name.
pub fn legacy_treesitter_slot_parameters_for_name(
    name: &str,
) -> &'static [(u32, SlotLiteral)] {{
    LEGACY_TREESITTER_COMPATIBILITY_PATTERNS
        .iter()
        .find(|p| p.name == name)
        .map_or(&[], |p| p.slot_parameters)
}}"#,
            syntax_kind_type = lang.syntax_kind_type,
            legacy_pattern_structs = legacy_patterns
                .iter()
                .map(|pattern| {
                    let slots = pattern
                        .slots
                        .iter()
                        .map(|(name, index)| format!(r#"("{}", {})"#, name, index))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let slot_parameters = pattern
                        .slot_parameters
                        .iter()
                        .map(|(index, literal)| {
                            format!("({}, {})", index, format_slot_literal(literal))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");

                    format!(
                        "    LegacyTreeSitterPattern {{ name: \"{name}\", kind: {syntax_kind_type}::{biome_kind}, slots: &[{slots}], slot_parameters: &[{slot_parameters}] }},",
                        name = pattern.name,
                        biome_kind = pattern.biome_kind,
                        slots = slots,
                        slot_parameters = slot_parameters,
                        syntax_kind_type = lang.syntax_kind_type,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        String::new()
    };

    let legacy_mappings_section = if has_legacy_patterns {
        format!(
            "        // Legacy TreeSitter patterns\n{}",
            legacy_patterns
                .iter()
                .map(|p| {
                    format!(
                        r#"        "{}" => Some({syntax_kind_type}::{}),"#,
                        p.name,
                        p.biome_kind,
                        syntax_kind_type = lang.syntax_kind_type
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        String::new()
    };

    let result = format!(
        r#"
{header}
use biome_rowan::AstNode;
use {syntax_module} as lang;
use lang::{syntax_kind_type};

{legacy_section}


/// Returns the syntax kind for a legacy or native node name.
pub fn kind_by_name(node_name: &str) -> Option<{syntax_kind_type}> {{

    match node_name {{
{legacy_mappings}

        // Native Biome AST patterns
{native_patterns}
        _ => None,
    }}
}}
"#,
        header = header,
        syntax_module = lang.syntax_module,
        syntax_kind_type = lang.syntax_kind_type,
        legacy_section = legacy_section,
        legacy_mappings = legacy_mappings_section,
        native_patterns = native_patterns,
    );

    xtask_glue::reformat(result)
}

fn format_slot_literal(literal: &SlotLiteral) -> String {
    match literal {
        SlotLiteral::Undefined => "SlotLiteral::Undefined".to_string(),
        SlotLiteral::String(value) => format!("SlotLiteral::String({value:?})"),
        SlotLiteral::Boolean(value) => format!("SlotLiteral::Boolean({value})"),
        SlotLiteral::Int(value) => format!("SlotLiteral::Int({value})"),
        SlotLiteral::Float(value) => format!("SlotLiteral::Float({value})"),
    }
}

struct LanguageConfig {
    syntax_kind_type: &'static str,
    syntax_module: &'static str,
    legacy_patterns: &'static [TreeSitterPattern],
}

impl LanguageConfig {
    fn new(language_kind: LanguageKind) -> Self {
        match language_kind {
            LanguageKind::Js => Self {
                syntax_kind_type: "JsSyntaxKind",
                syntax_module: "biome_js_syntax",
                legacy_patterns: JS_TREESITTER_PATTERNS,
            },
            LanguageKind::Css => Self {
                syntax_kind_type: "CssSyntaxKind",
                syntax_module: "biome_css_syntax",
                legacy_patterns: &[],
            },
            _ => unimplemented!("Grit mappings are not supported for {:?}", language_kind),
        }
    }
}

#[derive(Debug, Clone)]
struct TreeSitterPattern {
    name: &'static str,
    biome_kind: &'static str,
    slots: &'static [(&'static str, u32)],
    slot_parameters: &'static [(u32, SlotLiteral)],
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum SlotLiteral {
    Undefined,
    String(&'static str),
    Boolean(bool),
    Int(i64),
    Float(f64),
}

const JS_TREESITTER_PATTERNS: &[TreeSitterPattern] = &[
    TreeSitterPattern {
        name: "identifier",
        biome_kind: "JS_REFERENCE_IDENTIFIER",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "string",
        biome_kind: "JS_STRING_LITERAL_EXPRESSION",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "number",
        biome_kind: "JS_NUMBER_LITERAL_EXPRESSION",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "property_identifier",
        biome_kind: "JS_LITERAL_MEMBER_NAME",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "call_expression",
        biome_kind: "JS_CALL_EXPRESSION",
        slots: &[("function", 0), ("arguments", 3)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "member_expression",
        biome_kind: "JS_STATIC_MEMBER_EXPRESSION",
        slots: &[("object", 0), ("property", 2)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "subscript_expression",
        biome_kind: "JS_COMPUTED_MEMBER_EXPRESSION",
        slots: &[("object", 0), ("index", 3)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "binary_expression",
        biome_kind: "JS_BINARY_EXPRESSION",
        slots: &[("left", 0), ("right", 2)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "assignment_expression",
        biome_kind: "JS_ASSIGNMENT_EXPRESSION",
        slots: &[("left", 0), ("right", 2)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "conditional_expression",
        biome_kind: "JS_CONDITIONAL_EXPRESSION",
        slots: &[("condition", 0), ("consequence", 2), ("alternative", 4)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "arrow_function",
        biome_kind: "JS_ARROW_FUNCTION_EXPRESSION",
        slots: &[("parameters", 2), ("body", 5)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "object",
        biome_kind: "JS_OBJECT_EXPRESSION",
        slots: &[("properties", 1)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "array",
        biome_kind: "JS_ARRAY_EXPRESSION",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "pair",
        biome_kind: "JS_PROPERTY_OBJECT_MEMBER",
        slots: &[("key", 0), ("value", 2)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "if_statement",
        biome_kind: "JS_IF_STATEMENT",
        slots: &[("condition", 2), ("consequence", 4)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "for_statement",
        biome_kind: "JS_FOR_STATEMENT",
        slots: &[("initializer", 2), ("condition", 4), ("body", 8)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "while_statement",
        biome_kind: "JS_WHILE_STATEMENT",
        slots: &[("condition", 2), ("body", 4)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "function_declaration",
        biome_kind: "JS_FUNCTION_DECLARATION",
        slots: &[("name", 2), ("body", 7)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "return_statement",
        biome_kind: "JS_RETURN_STATEMENT",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "variable_declaration",
        biome_kind: "JS_VARIABLE_DECLARATION",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "expression_statement",
        biome_kind: "JS_EXPRESSION_STATEMENT",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "jsx_expression",
        biome_kind: "JSX_EXPRESSION_CHILD",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "jsx_attribute",
        biome_kind: "JSX_ATTRIBUTE",
        slots: &[("name", 0), ("value", 1)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "jsx_element",
        biome_kind: "JSX_ELEMENT",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "jsx_self_closing_element",
        biome_kind: "JSX_SELF_CLOSING_ELEMENT",
        slots: &[("name", 1), ("type_arguments", 2), ("attributes", 3)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "jsx_opening_element",
        biome_kind: "JSX_OPENING_ELEMENT",
        slots: &[("name", 1), ("type_arguments", 2), ("attributes", 3)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "jsx_closing_element",
        biome_kind: "JSX_CLOSING_ELEMENT",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "jsx_text",
        biome_kind: "JSX_TEXT",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "jsx_namespace_name",
        biome_kind: "JSX_NAMESPACE_NAME",
        slots: &[],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "object_type",
        biome_kind: "TS_OBJECT_TYPE",
        slots: &[("signatures", 1)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "type_annotation",
        biome_kind: "TS_TYPE_ANNOTATION",
        slots: &[("type", 1)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "property_signature",
        biome_kind: "TS_PROPERTY_SIGNATURE_TYPE_MEMBER",
        slots: &[("name", 1), ("type", 3)],
        slot_parameters: &[],
    },
    TreeSitterPattern {
        name: "required_parameter",
        biome_kind: "JS_FORMAL_PARAMETER",
        slots: &[("type", 3)],
        slot_parameters: &[(2, SlotLiteral::Undefined)],
    },
];
