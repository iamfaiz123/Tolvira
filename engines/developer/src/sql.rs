//! SQL formatting (sqlformat), minification, validation (sqlparser) and
//! entity-relationship extraction from DDL.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Value};
use sqlparser::ast::{ColumnOption, Statement, TableConstraint};
use sqlparser::dialect::{
    AnsiDialect, BigQueryDialect, ClickHouseDialect, Dialect, DuckDbDialect, GenericDialect,
    HiveDialect, MsSqlDialect, MySqlDialect, PostgreSqlDialect, RedshiftSqlDialect, SQLiteDialect,
    SnowflakeDialect,
};
use sqlparser::parser::Parser;
use std::ops::ControlFlow;

const MAX_SQL: usize = 5 * 1024 * 1024;

fn dialect(name: &str) -> EngineResult<Box<dyn Dialect>> {
    Ok(match name.trim().to_ascii_lowercase().as_str() {
        "" | "generic" => Box::new(GenericDialect {}),
        "postgres" | "postgresql" | "pg" => Box::new(PostgreSqlDialect {}),
        "mysql" | "mariadb" => Box::new(MySqlDialect {}),
        "sqlite" => Box::new(SQLiteDialect {}),
        "mssql" | "sqlserver" | "tsql" => Box::new(MsSqlDialect {}),
        "ansi" => Box::new(AnsiDialect {}),
        "bigquery" => Box::new(BigQueryDialect {}),
        "snowflake" => Box::new(SnowflakeDialect {}),
        "redshift" => Box::new(RedshiftSqlDialect {}),
        "clickhouse" => Box::new(ClickHouseDialect {}),
        "hive" => Box::new(HiveDialect {}),
        "duckdb" => Box::new(DuckDbDialect {}),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown SQL dialect `{other}` (generic, postgres, mysql, sqlite, mssql, ansi, \
                 bigquery, snowflake, redshift, clickhouse, hive, duckdb)"
            )))
        },
    })
}

fn check(sql: &str) -> EngineResult<()> {
    engine_shared::error::require_text(sql, "sql")?;
    engine_shared::limits::check_size(sql.len(), MAX_SQL, "sql")?;
    Ok(())
}

/// Pretty prints SQL.
pub fn format(sql: &str, indent: usize, uppercase: bool, lines_between: u8) -> EngineResult<Value> {
    check(sql)?;
    let opts = sqlformat::FormatOptions {
        indent: if indent == 0 {
            sqlformat::Indent::Tabs
        } else {
            sqlformat::Indent::Spaces(indent.min(16) as u8)
        },
        uppercase: Some(uppercase),
        lines_between_queries: lines_between.max(1),
        ignore_case_convert: None,
    };
    let out = sqlformat::format(sql, &sqlformat::QueryParams::None, &opts);
    Ok(json!({
        "formatted": out,
        "statements": count_statements(sql),
        "input_length": sql.len(),
        "output_length": out.len()
    }))
}

fn count_statements(sql: &str) -> usize {
    strip_comments(sql)
        .split(';')
        .filter(|s| !s.trim().is_empty())
        .count()
}

/// Removes comments and collapses whitespace while preserving string literals
/// and quoted identifiers.
pub fn minify(sql: &str) -> EngineResult<Value> {
    check(sql)?;
    let stripped = strip_comments(sql);
    let chars: Vec<char> = stripped.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    let mut pending_space = false;
    let is_punct = |c: char| {
        matches!(c, ',' | '(' | ')' | ';' | '=' | '<' | '>' | '+' | '-' | '*' | '/' | '.')
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' || c == '`' {
            if pending_space && !out.is_empty() && !out.ends_with(|x: char| is_punct(x)) {
                out.push(' ');
            }
            pending_space = false;
            let quote = c;
            out.push(c);
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == quote {
                    if i + 1 < chars.len() && chars[i + 1] == quote {
                        out.push(quote);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c.is_whitespace() {
            pending_space = true;
            i += 1;
            continue;
        }
        if pending_space && !out.is_empty() {
            let prev = out.chars().last().unwrap_or(' ');
            if !(is_punct(prev) || is_punct(c)) || (prev == '-' && c == '-') {
                out.push(' ');
            }
        }
        pending_space = false;
        out.push(c);
        i += 1;
    }
    let out = out.trim().to_string();
    Ok(json!({
        "minified": out,
        "input_length": sql.len(),
        "output_length": out.len(),
        "saved_percent": if sql.is_empty() { 0.0 } else { ((1.0 - out.len() as f64 / sql.len() as f64) * 1000.0).round() / 10.0 }
    }))
}

/// Strips `--` and `/* */` comments outside string literals.
pub fn strip_comments(sql: &str) -> String {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' || c == '`' {
            let quote = c;
            out.push(c);
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == quote {
                    if i + 1 < chars.len() && chars[i + 1] == quote {
                        out.push(quote);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c == '-' && chars.get(i + 1) == Some(&'-') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            out.push(' ');
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn statement_kind(s: &Statement) -> String {
    let d = format!("{s:?}");
    let name = d.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("Statement");
    // CamelCase → UPPER WORDS
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

fn tables_of(s: &Statement) -> Vec<String> {
    let mut tables = Vec::new();
    let _ = sqlparser::ast::visit_relations(s, |rel| {
        let n = rel.to_string();
        if !tables.contains(&n) {
            tables.push(n);
        }
        ControlFlow::<()>::Continue(())
    });
    tables
}

/// Parses SQL and reports statements, referenced tables and parse errors
/// with positions.
pub fn validate(sql: &str, dialect_name: &str) -> EngineResult<Value> {
    check(sql)?;
    let d = dialect(dialect_name)?;
    match Parser::parse_sql(&*d, sql) {
        Ok(statements) => {
            let list: Vec<Value> = statements
                .iter()
                .map(|s| {
                    json!({
                        "kind": statement_kind(s),
                        "tables": tables_of(s),
                        "normalized": s.to_string()
                    })
                })
                .collect();
            let mut warnings = Vec::new();
            for s in &statements {
                match s {
                    Statement::Delete(d) if d.selection.is_none() => {
                        warnings.push("DELETE without a WHERE clause removes every row".to_string())
                    },
                    Statement::Update { selection: None, .. } => warnings
                        .push("UPDATE without a WHERE clause modifies every row".to_string()),
                    Statement::Query(q) => {
                        let text = q.to_string();
                        if text.contains("SELECT *") {
                            warnings.push(
                                "SELECT * returns all columns; list the columns you need".into(),
                            );
                        }
                    },
                    Statement::Drop { .. } => {
                        warnings.push("DROP statements are destructive".into())
                    },
                    _ => {},
                }
            }
            Ok(json!({
                "valid": true,
                "dialect": if dialect_name.is_empty() { "generic" } else { dialect_name },
                "statement_count": statements.len(),
                "statements": list,
                "warnings": warnings,
                "tables": statements.iter().flat_map(tables_of).fold(Vec::<String>::new(), |mut acc, t| { if !acc.contains(&t) { acc.push(t); } acc })
            }))
        },
        Err(e) => {
            let msg = match &e {
                sqlparser::parser::ParserError::TokenizerError(m)
                | sqlparser::parser::ParserError::ParserError(m) => m.clone(),
                sqlparser::parser::ParserError::RecursionLimitExceeded => {
                    "Recursion limit exceeded (query is nested too deeply)".into()
                },
            };
            let (line, col) = extract_position(&msg);
            Ok(json!({
                "valid": false,
                "dialect": if dialect_name.is_empty() { "generic" } else { dialect_name },
                "error": { "message": msg, "line": line, "column": col },
                "statement_count": count_statements(sql)
            }))
        },
    }
}

fn extract_position(msg: &str) -> (Option<u64>, Option<u64>) {
    let find = |key: &str| {
        msg.split(key).nth(1).and_then(|rest| {
            rest.trim_start()
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|n| n.parse::<u64>().ok())
        })
    };
    (find("Line:"), find("Column:"))
}

/// Extracts tables, columns, keys and relationships from DDL and renders a
/// Mermaid `erDiagram` plus DBML.
pub fn erd(sql: &str, dialect_name: &str, infer_from_names: bool) -> EngineResult<Value> {
    check(sql)?;
    let d = dialect(dialect_name)?;
    let statements = Parser::parse_sql(&*d, sql)
        .map_err(|e| EngineError::malformed(format!("SQL parse error: {e}")))?;
    let mut entities: Vec<Value> = Vec::new();
    let mut relations: Vec<Value> = Vec::new();
    let mut table_names: Vec<String> = Vec::new();
    let strip_quotes = |s: &str| s.trim_matches(['"', '`', '[', ']', '\'']).to_string();
    for s in &statements {
        if let Statement::CreateTable(ct) = s {
            let name = strip_quotes(&ct.name.to_string());
            table_names.push(name.clone());
            let mut pks: Vec<String> = Vec::new();
            let mut uniques: Vec<String> = Vec::new();
            let mut fks: Vec<(Vec<String>, String, Vec<String>)> = Vec::new();
            for c in &ct.constraints {
                match c {
                    TableConstraint::PrimaryKey { columns, .. } => {
                        pks.extend(columns.iter().map(|c| c.value.clone()))
                    },
                    TableConstraint::Unique { columns, .. } => {
                        uniques.extend(columns.iter().map(|c| c.value.clone()))
                    },
                    TableConstraint::ForeignKey {
                        columns,
                        foreign_table,
                        referred_columns,
                        ..
                    } => fks.push((
                        columns.iter().map(|c| c.value.clone()).collect(),
                        strip_quotes(&foreign_table.to_string()),
                        referred_columns.iter().map(|c| c.value.clone()).collect(),
                    )),
                    _ => {},
                }
            }
            let mut columns = Vec::new();
            for col in &ct.columns {
                let mut nullable = true;
                let mut is_pk = pks.contains(&col.name.value);
                let mut is_unique = uniques.contains(&col.name.value);
                let mut default = None;
                let mut is_fk = false;
                for o in &col.options {
                    match &o.option {
                        ColumnOption::NotNull => nullable = false,
                        ColumnOption::Null => nullable = true,
                        ColumnOption::Unique { is_primary, .. } => {
                            if *is_primary {
                                is_pk = true;
                                pks.push(col.name.value.clone());
                            } else {
                                is_unique = true;
                            }
                        },
                        ColumnOption::Default(e) => default = Some(e.to_string()),
                        ColumnOption::ForeignKey { foreign_table, referred_columns, .. } => {
                            is_fk = true;
                            fks.push((
                                vec![col.name.value.clone()],
                                strip_quotes(&foreign_table.to_string()),
                                referred_columns.iter().map(|c| c.value.clone()).collect(),
                            ));
                        },
                        _ => {},
                    }
                }
                if is_pk {
                    nullable = false;
                }
                columns.push(json!({
                    "name": col.name.value,
                    "type": col.data_type.to_string(),
                    "nullable": nullable,
                    "primary_key": is_pk,
                    "unique": is_unique,
                    "foreign_key": is_fk || fks.iter().any(|(cols, _, _)| cols.contains(&col.name.value)),
                    "default": default
                }));
            }
            for (cols, target, ref_cols) in &fks {
                relations.push(json!({
                    "from": name,
                    "from_columns": cols,
                    "to": target,
                    "to_columns": ref_cols,
                    "kind": "many-to-one",
                    "source": "constraint"
                }));
            }
            entities.push(json!({
                "name": name,
                "columns": columns,
                "primary_key": pks,
                "column_count": ct.columns.len()
            }));
        }
    }
    if entities.is_empty() {
        return Err(EngineError::invalid("No CREATE TABLE statements found"));
    }
    if infer_from_names {
        // `customer_id` in `orders` → customers/customer table.
        let mut inferred = Vec::new();
        for e in &entities {
            let from = e["name"].as_str().unwrap_or("");
            for c in e["columns"].as_array().into_iter().flatten() {
                let cname = c["name"].as_str().unwrap_or("");
                if c["foreign_key"] == true || c["primary_key"] == true {
                    continue;
                }
                if let Some(base) = cname
                    .strip_suffix("_id")
                    .or_else(|| cname.strip_suffix("Id"))
                    .or_else(|| cname.strip_suffix("_ID"))
                {
                    let candidates = [
                        base.to_string(),
                        format!("{base}s"),
                        format!("{base}es"),
                        base.trim_end_matches('y').to_string() + "ies",
                    ];
                    if let Some(target) = table_names.iter().find(|t| {
                        candidates.iter().any(|c| t.eq_ignore_ascii_case(c))
                            && !t.eq_ignore_ascii_case(from)
                    }) {
                        inferred.push(json!({
                            "from": from,
                            "from_columns": [cname],
                            "to": target,
                            "to_columns": ["id"],
                            "kind": "many-to-one",
                            "source": "inferred_from_name"
                        }));
                    }
                }
            }
        }
        relations.extend(inferred);
    }
    // Mermaid.
    let safe = |s: &str| s.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
    let mut mermaid = String::from("erDiagram\n");
    for e in &entities {
        mermaid.push_str(&format!("    {} {{\n", safe(e["name"].as_str().unwrap_or(""))));
        for c in e["columns"].as_array().into_iter().flatten() {
            let mut keys = Vec::new();
            if c["primary_key"] == true {
                keys.push("PK");
            }
            if c["foreign_key"] == true {
                keys.push("FK");
            }
            if c["unique"] == true && c["primary_key"] != true {
                keys.push("UK");
            }
            mermaid.push_str(&format!(
                "        {} {}{}\n",
                safe(&c["type"].as_str().unwrap_or("").replace(' ', "_")),
                safe(c["name"].as_str().unwrap_or("")),
                if keys.is_empty() {
                    String::new()
                } else {
                    format!(" {}", keys.join(","))
                }
            ));
        }
        mermaid.push_str("    }\n");
    }
    for r in &relations {
        mermaid.push_str(&format!(
            "    {} }}o--|| {} : \"{}\"\n",
            safe(r["from"].as_str().unwrap_or("")),
            safe(r["to"].as_str().unwrap_or("")),
            r["from_columns"][0].as_str().unwrap_or("")
        ));
    }
    // DBML.
    let mut dbml = String::new();
    for e in &entities {
        dbml.push_str(&format!("Table {} {{\n", e["name"].as_str().unwrap_or("")));
        for c in e["columns"].as_array().into_iter().flatten() {
            let mut attrs = Vec::new();
            if c["primary_key"] == true {
                attrs.push("pk".to_string());
            }
            if c["unique"] == true {
                attrs.push("unique".into());
            }
            if c["nullable"] == false && c["primary_key"] != true {
                attrs.push("not null".into());
            }
            if let Some(d) = c["default"].as_str() {
                attrs.push(format!("default: `{d}`"));
            }
            dbml.push_str(&format!(
                "  {} {}{}\n",
                c["name"].as_str().unwrap_or(""),
                c["type"].as_str().unwrap_or("").to_lowercase().replace(' ', "_"),
                if attrs.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", attrs.join(", "))
                }
            ));
        }
        dbml.push_str("}\n\n");
    }
    for r in &relations {
        dbml.push_str(&format!(
            "Ref: {}.{} > {}.{}\n",
            r["from"].as_str().unwrap_or(""),
            r["from_columns"][0].as_str().unwrap_or(""),
            r["to"].as_str().unwrap_or(""),
            r["to_columns"][0].as_str().unwrap_or("id")
        ));
    }
    Ok(json!({
        "entities": entities,
        "relations": relations,
        "entity_count": entities.len(),
        "relation_count": relations.len(),
        "mermaid": mermaid,
        "dbml": dbml
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_minify_validate() {
        let f = format("select a,b from t where x=1 -- c\n", 2, true, 1).unwrap();
        assert!(f["formatted"]
            .as_str()
            .unwrap()
            .starts_with("SELECT\n  a,\n  b\nFROM"));
        let m =
            minify("SELECT   a ,  b\n FROM t /* hi */ WHERE name = 'a  b' -- trailing\n;").unwrap();
        assert_eq!(m["minified"], "SELECT a,b FROM t WHERE name='a  b';");
        let v = validate(
            "SELECT id FROM users u JOIN orders o ON o.user_id = u.id; DELETE FROM logs",
            "postgres",
        )
        .unwrap();
        assert_eq!(v["valid"], true);
        assert_eq!(v["statement_count"], 2);
        assert_eq!(v["tables"], json!(["users", "orders", "logs"]));
        assert!(v["warnings"][0].as_str().unwrap().contains("DELETE"));
        let bad = validate("SELEC * FROM", "mysql").unwrap();
        assert_eq!(bad["valid"], false);
        assert!(bad["error"]["line"].is_number());
        assert!(validate("SELECT 1", "oracle").is_err());
        assert!(validate("", "").is_err());
    }

    #[test]
    fn erd_extraction() {
        let sql = "CREATE TABLE customers (id INT PRIMARY KEY, email VARCHAR(255) NOT NULL \
                   UNIQUE);\nCREATE TABLE orders (id SERIAL PRIMARY KEY, customer_id INT NOT NULL \
                   REFERENCES customers(id), product_id INT, total NUMERIC(10,2) DEFAULT \
                   0);\nCREATE TABLE products (id INT, name TEXT, PRIMARY KEY (id));";
        let e = erd(sql, "postgres", true).unwrap();
        assert_eq!(e["entity_count"], 3);
        assert_eq!(e["relation_count"], 2);
        assert_eq!(e["relations"][1]["source"], "inferred_from_name");
        assert!(e["mermaid"].as_str().unwrap().contains("orders }o--|| customers"));
        assert!(e["dbml"]
            .as_str()
            .unwrap()
            .contains("email varchar(255) [unique, not null]"));
        assert_eq!(e["entities"][2]["primary_key"], json!(["id"]));
        assert!(erd("SELECT 1", "", true).is_err());
    }
}
