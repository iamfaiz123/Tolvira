//! GraphQL formatting, validation and schema inspection (graphql-parser).

use engine_shared::{EngineError, EngineResult};
use graphql_parser::query::{Definition as QDef, OperationDefinition, Selection};
use graphql_parser::schema::{
    Definition as SDef, Document as SchemaDoc, Type, TypeDefinition, TypeExtension,
};
use serde_json::{json, Value};

const MAX: usize = 2 * 1024 * 1024;

fn detect_kind(text: &str) -> &'static str {
    let t = text.trim_start();
    // Strip leading comments / descriptions for the heuristic.
    let first_word = t
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or("")
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("");
    match first_word {
        "type" | "schema" | "interface" | "enum" | "input" | "scalar" | "union" | "directive"
        | "extend" => "schema",
        "query" | "mutation" | "subscription" | "fragment" | "{" => "query",
        _ => {
            if t.starts_with('{') || t.starts_with("query") {
                "query"
            } else if t.starts_with("\"\"\"") || t.starts_with('"') {
                "schema"
            } else {
                "query"
            }
        },
    }
}

fn parse_error(e: impl std::fmt::Display) -> Value {
    let msg = e.to_string();
    // graphql-parser errors look like "query parse error: Parse error at 1:8\nUnexpected ..."
    let mut line = None;
    let mut col = None;
    if let Some(at) = msg.find(" at ") {
        let rest = &msg[at + 4..];
        let pos: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == ':').collect();
        if let Some((l, c)) = pos.split_once(':') {
            line = l.parse::<usize>().ok();
            col = c.parse::<usize>().ok();
        }
    }
    json!({
        "message": msg.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" – "),
        "line": line,
        "column": col
    })
}

/// Formats a query or schema document (auto-detected unless `kind` given).
pub fn format(text: &str, kind: &str) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "graphql")?;
    engine_shared::limits::check_size(text.len(), MAX, "graphql")?;
    let k = if kind.is_empty() || kind == "auto" {
        detect_kind(text)
    } else {
        kind
    };
    let formatted = match k {
        "schema" | "sdl" => graphql_parser::parse_schema::<String>(text)
            .map(|d| d.to_string())
            .map_err(|e| e.to_string()),
        "query" | "operation" => graphql_parser::parse_query::<String>(text)
            .map(|d| d.to_string())
            .map_err(|e| e.to_string()),
        other => {
            return Err(EngineError::invalid(format!("Unknown kind `{other}` (query, schema)")))
        },
    };
    match formatted {
        Ok(f) => Ok(json!({ "formatted": f, "kind": k, "valid": true })),
        Err(e) => Err(EngineError::malformed(format!(
            "GraphQL parse error: {}",
            parse_error(e)["message"].as_str().unwrap_or("")
        ))),
    }
}

fn type_string(t: &Type<'_, String>) -> (String, bool, bool) {
    // returns (display, non_null, is_list)
    match t {
        Type::NamedType(n) => (n.clone(), false, false),
        Type::ListType(inner) => {
            let (d, _, _) = type_string(inner);
            (format!("[{d}]"), false, true)
        },
        Type::NonNullType(inner) => {
            let (d, _, l) = type_string(inner);
            (format!("{d}!"), true, l)
        },
    }
}

fn base_type(t: &Type<'_, String>) -> String {
    match t {
        Type::NamedType(n) => n.clone(),
        Type::ListType(i) | Type::NonNullType(i) => base_type(i),
    }
}

/// Validates a document and reports structural details. For queries,
/// checks fragment usage and (when a schema is supplied) that root fields and
/// referenced types exist.
pub fn validate(text: &str, kind: &str, schema: Option<&str>) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "graphql")?;
    engine_shared::limits::check_size(text.len(), MAX, "graphql")?;
    let k = if kind.is_empty() || kind == "auto" {
        detect_kind(text)
    } else {
        kind
    };
    let mut warnings: Vec<String> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    match k {
        "schema" | "sdl" => match graphql_parser::parse_schema::<String>(text) {
            Ok(doc) => {
                let info = schema_info(&doc);
                // Semantic checks: unknown referenced types, missing Query.
                let schema_types = info["types"].as_array().cloned().unwrap_or_default();
                let defined: Vec<String> = schema_types
                    .iter()
                    .filter_map(|t| t["name"].as_str())
                    .map(str::to_string)
                    .collect();
                let builtins = ["String", "Int", "Float", "Boolean", "ID"];
                for t in &schema_types {
                    for f in t["fields"].as_array().cloned().unwrap_or_default() {
                        let b = f["base_type"].as_str().unwrap_or("");
                        if !b.is_empty()
                            && !defined.iter().any(|d| d == b)
                            && !builtins.contains(&b)
                        {
                            errors.push(json!({
                                "message": format!("Field `{}.{}` references unknown type `{b}`", t["name"].as_str().unwrap_or(""), f["name"].as_str().unwrap_or("")),
                                "line": f["line"],
                                "column": f["column"]
                            }));
                        }
                    }
                }
                if !defined
                    .iter()
                    .any(|d| d == info["root_types"]["query"].as_str().unwrap_or("Query"))
                {
                    warnings.push("No `Query` root type defined".into());
                }
                let mut out = info;
                out["kind"] = json!("schema");
                out["valid"] = json!(errors.is_empty());
                out["errors"] = json!(errors);
                out["warnings"] = json!(warnings);
                Ok(out)
            },
            Err(e) => Ok(json!({
                "kind": "schema",
                "valid": false,
                "errors": [parse_error(e)],
                "warnings": []
            })),
        },
        "query" | "operation" => match graphql_parser::parse_query::<String>(text) {
            Ok(doc) => {
                let mut operations = Vec::new();
                let mut fragments_defined: Vec<String> = Vec::new();
                let mut fragments_used: Vec<String> = Vec::new();
                let mut anonymous = 0;
                let mut max_depth = 0usize;
                for d in &doc.definitions {
                    match d {
                        QDef::Operation(op) => {
                            let (kind, name, vars, sel) = match op {
                                OperationDefinition::SelectionSet(s) => ("query", None, vec![], s),
                                OperationDefinition::Query(q) => ("query", q.name.clone(), q.variable_definitions.iter().map(|v| json!({
                                    "name": format!("${}", v.name),
                                    "type": type_string(&v.var_type).0,
                                    "default": v.default_value.as_ref().map(|d| d.to_string())
                                })).collect(), &q.selection_set),
                                OperationDefinition::Mutation(m) => ("mutation", m.name.clone(), m.variable_definitions.iter().map(|v| json!({
                                    "name": format!("${}", v.name),
                                    "type": type_string(&v.var_type).0
                                })).collect(), &m.selection_set),
                                OperationDefinition::Subscription(s) => ("subscription", s.name.clone(), s.variable_definitions.iter().map(|v| json!({
                                    "name": format!("${}", v.name),
                                    "type": type_string(&v.var_type).0
                                })).collect(), &s.selection_set),
                            };
                            if name.is_none() {
                                anonymous += 1;
                            }
                            let mut root_fields = Vec::new();
                            let depth =
                                walk_selection(sel, 1, &mut fragments_used, &mut root_fields, true);
                            max_depth = max_depth.max(depth);
                            operations.push(json!({
                                "type": kind,
                                "name": name,
                                "variables": vars,
                                "root_fields": root_fields,
                                "depth": depth
                            }));
                        },
                        QDef::Fragment(f) => {
                            fragments_defined.push(f.name.clone());
                            let mut ignored = Vec::new();
                            let depth = walk_selection(
                                &f.selection_set,
                                1,
                                &mut fragments_used,
                                &mut ignored,
                                false,
                            );
                            max_depth = max_depth.max(depth);
                        },
                    }
                }
                for u in &fragments_used {
                    if !fragments_defined.contains(u) {
                        errors.push(json!({
                            "message": format!("Fragment `{u}` is used but not defined"),
                            "line": Value::Null,
                            "column": Value::Null
                        }));
                    }
                }
                for d in &fragments_defined {
                    if !fragments_used.contains(d) {
                        warnings.push(format!("Fragment `{d}` is defined but never used"));
                    }
                }
                if anonymous > 1 || (anonymous == 1 && operations.len() > 1) {
                    errors.push(json!({
                        "message": "Anonymous operations must be the only operation in a document",
                        "line": Value::Null,
                        "column": Value::Null
                    }));
                }
                if max_depth > 10 {
                    warnings.push(format!(
                        "Query depth {max_depth} is very deep; servers often limit depth to 10"
                    ));
                }
                if let Some(schema_text) = schema.filter(|s| !s.trim().is_empty()) {
                    match graphql_parser::parse_schema::<String>(schema_text) {
                        Ok(sdoc) => {
                            let info = schema_info(&sdoc);
                            for op in &operations {
                                let op_type = op["type"].as_str().unwrap_or("query");
                                let root_name = info["root_types"][op_type].as_str().unwrap_or("").to_string();
                                let root = info["types"].as_array().into_iter().flatten().find(|t| t["name"] == root_name);
                                match root {
                                    None => errors.push(json!({
                                        "message": format!("Schema has no {op_type} root type"),
                                        "line": Value::Null,
                                        "column": Value::Null
                                    })),
                                    Some(r) => {
                                        for f in op["root_fields"].as_array().into_iter().flatten() {
                                            let fname = f.as_str().unwrap_or("");
                                            if fname != "__typename" && fname != "__schema" && fname != "__type" && !r["fields"].as_array().into_iter().flatten().any(|x| x["name"] == fname) {
                                                errors.push(json!({
                                                    "message": format!("Field `{fname}` does not \
                                                                        exist on type \
                                                                        `{root_name}`"),
                                                    "line": Value::Null,
                                                    "column": Value::Null
                                                }));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => errors.push(json!({
                            "message": format!("Schema could not be parsed: {}", parse_error(e)["message"].as_str().unwrap_or("")),
                            "line": Value::Null,
                            "column": Value::Null
                        })),
                    }
                }
                Ok(json!({
                    "kind": "query",
                    "valid": errors.is_empty(),
                    "errors": errors,
                    "warnings": warnings,
                    "operations": operations,
                    "fragments": fragments_defined,
                    "max_depth": max_depth,
                    "formatted": doc.to_string()
                }))
            },
            Err(e) => Ok(json!({
                "kind": "query",
                "valid": false,
                "errors": [parse_error(e)],
                "warnings": []
            })),
        },
        other => Err(EngineError::invalid(format!("Unknown kind `{other}` (query, schema)"))),
    }
}

fn walk_selection(
    set: &graphql_parser::query::SelectionSet<'_, String>,
    depth: usize,
    fragments_used: &mut Vec<String>,
    root_fields: &mut Vec<Value>,
    collect_roots: bool,
) -> usize {
    let mut max = depth;
    for item in &set.items {
        match item {
            Selection::Field(f) => {
                if collect_roots {
                    root_fields.push(json!(f.name));
                }
                if !f.selection_set.items.is_empty() {
                    max = max.max(walk_selection(
                        &f.selection_set,
                        depth + 1,
                        fragments_used,
                        root_fields,
                        false,
                    ));
                }
            },
            Selection::FragmentSpread(s) => {
                if !fragments_used.contains(&s.fragment_name) {
                    fragments_used.push(s.fragment_name.clone());
                }
            },
            Selection::InlineFragment(i) => {
                max = max.max(walk_selection(
                    &i.selection_set,
                    depth,
                    fragments_used,
                    root_fields,
                    collect_roots,
                ));
            },
        }
    }
    max
}

fn field_json(f: &graphql_parser::schema::Field<'_, String>) -> Value {
    let (display, non_null, is_list) = type_string(&f.field_type);
    json!({
        "name": f.name,
        "type": display,
        "base_type": base_type(&f.field_type),
        "non_null": non_null,
        "is_list": is_list,
        "description": f.description,
        "deprecated": f.directives.iter().any(|d| d.name == "deprecated"),
        "arguments": f.arguments.iter().map(|a| json!({ "name": a.name, "type": type_string(&a.value_type).0, "default": a.default_value.as_ref().map(|d| d.to_string()), "description": a.description })).collect::<Vec<_>>(),
        "line": f.position.line,
        "column": f.position.column,
    })
}

fn schema_info(doc: &SchemaDoc<'_, String>) -> Value {
    let mut types = Vec::new();
    let mut directives = Vec::new();
    let mut root =
        json!({ "query": "Query", "mutation": "Mutation", "subscription": "Subscription" });
    let mut counts = json!({
        "object": 0,
        "interface": 0,
        "enum": 0,
        "input": 0,
        "scalar": 0,
        "union": 0,
        "extension": 0
    });
    let bump = |counts: &mut Value, k: &str| counts[k] = json!(counts[k].as_u64().unwrap_or(0) + 1);
    for d in &doc.definitions {
        match d {
            SDef::SchemaDefinition(s) => {
                if let Some(q) = &s.query {
                    root["query"] = json!(q);
                }
                if let Some(m) = &s.mutation {
                    root["mutation"] = json!(m);
                }
                if let Some(s) = &s.subscription {
                    root["subscription"] = json!(s);
                }
            },
            SDef::TypeDefinition(t) => match t {
                TypeDefinition::Object(o) => {
                    bump(&mut counts, "object");
                    types.push(json!({
                        "kind": "object",
                        "name": o.name,
                        "description": o.description,
                        "implements": o.implements_interfaces,
                        "fields": o.fields.iter().map(field_json).collect::<Vec<_>>(),
                        "line": o.position.line
                    }));
                },
                TypeDefinition::Interface(i) => {
                    bump(&mut counts, "interface");
                    types.push(json!({
                        "kind": "interface",
                        "name": i.name,
                        "description": i.description,
                        "implements": i.implements_interfaces,
                        "fields": i.fields.iter().map(field_json).collect::<Vec<_>>(),
                        "line": i.position.line
                    }));
                },
                TypeDefinition::Enum(e) => {
                    bump(&mut counts, "enum");
                    types.push(json!({
                        "kind": "enum",
                        "name": e.name,
                        "description": e.description,
                        "values": e.values.iter().map(|v| json!({ "name": v.name, "description": v.description, "deprecated": v.directives.iter().any(|d| d.name == "deprecated") })).collect::<Vec<_>>(),
                        "line": e.position.line
                    }));
                },
                TypeDefinition::InputObject(i) => {
                    bump(&mut counts, "input");
                    types.push(json!({
                        "kind": "input",
                        "name": i.name,
                        "description": i.description,
                        "fields": i.fields.iter().map(|a| { let (display, non_null, is_list) = type_string(&a.value_type); json!({ "name": a.name, "type": display, "base_type": base_type(&a.value_type), "non_null": non_null, "is_list": is_list, "default": a.default_value.as_ref().map(|d| d.to_string()), "description": a.description, "line": a.position.line, "column": a.position.column }) }).collect::<Vec<_>>(),
                        "line": i.position.line
                    }));
                },
                TypeDefinition::Scalar(s) => {
                    bump(&mut counts, "scalar");
                    types.push(json!({
                        "kind": "scalar",
                        "name": s.name,
                        "description": s.description,
                        "line": s.position.line
                    }));
                },
                TypeDefinition::Union(u) => {
                    bump(&mut counts, "union");
                    types.push(json!({
                        "kind": "union",
                        "name": u.name,
                        "description": u.description,
                        "types": u.types,
                        "line": u.position.line
                    }));
                },
            },
            SDef::TypeExtension(e) => {
                bump(&mut counts, "extension");
                let name = match e {
                    TypeExtension::Scalar(s) => s.name.clone(),
                    TypeExtension::Object(o) => o.name.clone(),
                    TypeExtension::Interface(i) => i.name.clone(),
                    TypeExtension::Union(u) => u.name.clone(),
                    TypeExtension::Enum(e) => e.name.clone(),
                    TypeExtension::InputObject(i) => i.name.clone(),
                };
                types.push(json!({ "kind": "extension", "name": name }));
            },
            SDef::DirectiveDefinition(dd) => {
                directives.push(json!({
                    "name": format!("@{}", dd.name),
                    "description": dd.description,
                    "locations": dd.locations.iter().map(|l| format!("{l:?}")).collect::<Vec<_>>(),
                    "arguments": dd.arguments.iter().map(|a| json!({ "name": a.name, "type": type_string(&a.value_type).0 })).collect::<Vec<_>>()
                }));
            },
        }
    }
    // Relationship graph: object → referenced object types.
    let object_names: Vec<String> = types
        .iter()
        .filter(|t| t["kind"] == "object" || t["kind"] == "interface")
        .filter_map(|t| t["name"].as_str())
        .map(str::to_string)
        .collect();
    let mut edges = Vec::new();
    for t in &types {
        if let Some(fields) = t["fields"].as_array() {
            for f in fields {
                let b = f["base_type"].as_str().unwrap_or("");
                if object_names.iter().any(|o| o == b) {
                    edges.push(json!({
                        "from": t["name"],
                        "to": b,
                        "field": f["name"],
                        "list": f["is_list"]
                    }));
                }
            }
        }
    }
    let mut mermaid = String::from("classDiagram\n");
    for t in &types {
        if let Some(fields) = t["fields"].as_array() {
            mermaid.push_str(&format!("    class {} {{\n", t["name"].as_str().unwrap_or("")));
            for f in fields {
                mermaid.push_str(&format!(
                    "        {} {}\n",
                    f["type"].as_str().unwrap_or("").replace(['[', ']', '!'], ""),
                    f["name"].as_str().unwrap_or("")
                ));
            }
            mermaid.push_str("    }\n");
        }
    }
    for e in &edges {
        mermaid.push_str(&format!(
            "    {} --> {} : {}\n",
            e["from"].as_str().unwrap_or(""),
            e["to"].as_str().unwrap_or(""),
            e["field"].as_str().unwrap_or("")
        ));
    }
    json!({
        "types": types,
        "directives": directives,
        "root_types": root,
        "counts": counts,
        "type_count": types.len(),
        "relationships": edges,
        "mermaid": mermaid
    })
}

/// Schema viewer: types, fields, relationships and a Mermaid class diagram.
pub fn schema(text: &str) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "schema")?;
    engine_shared::limits::check_size(text.len(), MAX, "schema")?;
    let doc = graphql_parser::parse_schema::<String>(text).map_err(|e| {
        EngineError::malformed(format!(
            "GraphQL schema parse error: {}",
            parse_error(e)["message"].as_str().unwrap_or("")
        ))
    })?;
    let mut info = schema_info(&doc);
    info["formatted"] = json!(doc.to_string());
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SDL: &str = "type Query { user(id: ID!): User users: [User!]! }\n\"\"\"A \
                       user\"\"\"\ntype User { id: ID! name: String @deprecated(reason: \"x\") \
                       posts: [Post] }\ntype Post { id: ID! title: String! author: User }\nenum \
                       Role { ADMIN USER }";

    #[test]
    fn schema_and_query() {
        let s = schema(SDL).unwrap();
        assert_eq!(s["counts"]["object"], 3);
        assert_eq!(s["counts"]["enum"], 1);
        assert_eq!(s["relationships"].as_array().unwrap().len(), 4);
        assert_eq!(s["types"][1]["fields"][1]["deprecated"], true);
        assert!(s["mermaid"].as_str().unwrap().contains("User --> Post : posts"));
        let v = validate(
            "query Q($id: ID!) { user(id: $id) { ...F posts { title } } } fragment F on User { \
             name } fragment Unused on User { id }",
            "",
            Some(SDL),
        )
        .unwrap();
        assert_eq!(v["valid"], true, "{v}");
        assert_eq!(v["operations"][0]["depth"], 3);
        assert!(v["warnings"][0].as_str().unwrap().contains("Unused"));
        let v = validate("{ nope { id } ...Missing }", "", Some(SDL)).unwrap();
        assert_eq!(v["valid"], false);
        assert_eq!(v["errors"].as_array().unwrap().len(), 2);
        let bad = validate("query {", "", None).unwrap();
        assert_eq!(bad["valid"], false);
        assert!(bad["errors"][0]["line"].is_number());
        let sv = validate("type Query { a: Missing }", "", None).unwrap();
        assert_eq!(sv["valid"], false);
        let f = format("query{user(id:1){name}}", "").unwrap();
        assert_eq!(f["formatted"], "query {\n  user(id: 1) {\n    name\n  }\n}\n");
        assert!(format("type {", "schema").is_err());
        assert_eq!(detect_kind("\"\"\"doc\"\"\"\ntype Query { a: Int }"), "schema");
    }
}
