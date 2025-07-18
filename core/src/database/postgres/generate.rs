use alloy::primitives::keccak256;
use std::path::Path;
use tracing::{error, info};

use crate::manifest::contract::FactoryDetailsYaml;
use crate::{
    abi::{ABIInput, ABIItem, EventInfo, GenerateAbiPropertiesType, ParamTypeError, ReadAbiError},
    helpers::camel_to_snake,
    indexer::{
        native_transfer::{NATIVE_TRANSFER_ABI, NATIVE_TRANSFER_CONTRACT_NAME},
        Indexer,
    },
    manifest::contract::Contract,
    types::code::Code,
};

fn compact_table_name_if_needed(table_name: String) -> String {
    // sql table names cant be as long as 63
    if table_name.len() > 63 {
        let hash_bytes = keccak256(table_name.as_bytes());
        let hash = hash_bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let hash_prefix = &hash[0..10];

        // Preserve the beginning of the original name, but leave room for the hash
        let preserved_length = 63 - 11; // 10 for hash plus 1 for underscore
        let prefix = &table_name[0..preserved_length];

        return format!("{prefix}_{hash_prefix}");
    }

    table_name
}

fn generate_columns(inputs: &[ABIInput], property_type: &GenerateAbiPropertiesType) -> Vec<String> {
    ABIInput::generate_abi_name_properties(inputs, property_type, None)
        .into_iter()
        .map(|m| m.value)
        .collect()
}

pub fn generate_columns_with_data_types(inputs: &[ABIInput]) -> Vec<String> {
    generate_columns(inputs, &GenerateAbiPropertiesType::PostgresWithDataTypes)
}

fn generate_columns_names_only(inputs: &[ABIInput]) -> Vec<String> {
    generate_columns(inputs, &GenerateAbiPropertiesType::PostgresColumnsNamesOnly)
}

pub fn generate_column_names_only_with_base_properties(inputs: &[ABIInput]) -> Vec<String> {
    let mut column_names: Vec<String> = vec!["contract_address".to_string()];
    column_names.extend(generate_columns_names_only(inputs));
    column_names.extend(vec![
        "tx_hash".to_string(),
        "block_number".to_string(),
        "block_hash".to_string(),
        "network".to_string(),
        "tx_index".to_string(),
        "log_index".to_string(),
    ]);
    column_names
}

fn generate_event_table_sql_with_comments(
    abi_inputs: &[EventInfo],
    contract_name: &str,
    schema_name: &str,
    apply_full_name_comment_for_events: Vec<String>,
) -> String {
    abi_inputs
        .iter()
        .map(|event_info| {
            let table_name = format!("{}.{}", schema_name, camel_to_snake(&event_info.name));
            info!("Creating table if not exists: {}", table_name);
            let event_columns = if event_info.inputs.is_empty() {
                "".to_string()
            } else {
                generate_columns_with_data_types(&event_info.inputs).join(", ") + ","
            };

            let create_table_sql = format!(
                "CREATE TABLE IF NOT EXISTS {table_name} (\
                rindexer_id SERIAL PRIMARY KEY NOT NULL, \
                contract_address CHAR(66) NOT NULL, \
                {event_columns} \
                tx_hash CHAR(66) NOT NULL, \
                block_number NUMERIC NOT NULL, \
                block_hash CHAR(66) NOT NULL, \
                network VARCHAR(50) NOT NULL, \
                tx_index NUMERIC NOT NULL, \
                log_index VARCHAR(78) NOT NULL\
            );"
            );

            if !apply_full_name_comment_for_events.contains(&event_info.name) {
                return create_table_sql;
            }

            // smart comments needed to avoid clashing of order by graphql names
            let table_comment = format!(
                "COMMENT ON TABLE {} IS E'@name {}{}';",
                table_name, contract_name, event_info.name
            );

            format!("{create_table_sql}\n{table_comment}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn generate_internal_factory_event_table_sql(
    indexer_name: &str,
    factories: &[FactoryDetailsYaml],
) -> String {
    factories.iter().map(|factory| {
        let params = GenerateInternalFactoryEventTableNameParams {
            indexer_name: indexer_name.to_string(),
            contract_name: factory.name.to_string(),
            event_name: factory.event_name.to_string(),
            input_name: factory.input_name.to_string(),
        };
        let table_name = generate_internal_factory_event_table_name(&params);

        let create_table_query = format!(
            r#"CREATE TABLE IF NOT EXISTS rindexer_internal.{table_name} ("factory_address" CHAR(42), "factory_deployed_address" CHAR(42), "network" TEXT, PRIMARY KEY ("factory_address", "factory_deployed_address", "network"));"#
        );

        create_table_query
    }).collect::<Vec<_>>().join("\n")
}

fn generate_internal_event_table_sql(
    abi_inputs: &[EventInfo],
    schema_name: &str,
    networks: Vec<&str>,
) -> String {
    abi_inputs.iter().map(|event_info| {
        let table_name = generate_internal_event_table_name(schema_name, &event_info.name);

        let create_table_query = format!(
            r#"CREATE TABLE IF NOT EXISTS rindexer_internal.{table_name} ("network" TEXT PRIMARY KEY, "last_synced_block" NUMERIC);"#
        );

        let insert_queries = networks.iter().map(|network| {
            format!(
                r#"INSERT INTO rindexer_internal.{table_name} ("network", "last_synced_block") VALUES ('{network}', 0) ON CONFLICT ("network") DO NOTHING;"#,
            )
        }).collect::<Vec<_>>().join("\n");

        let create_latest_block_query = r#"CREATE TABLE IF NOT EXISTS rindexer_internal.latest_block ("network" TEXT PRIMARY KEY, "block" NUMERIC);"#.to_string();

        let latest_block_insert_queries = networks.iter().map(|network| {
            format!(
                r#"INSERT INTO rindexer_internal.latest_block ("network", "block") VALUES ('{network}', 0) ON CONFLICT ("network") DO NOTHING;"#
            )
        }).collect::<Vec<_>>().join("\n");

        format!("{create_table_query}\n{insert_queries}\n{create_latest_block_query}\n{latest_block_insert_queries}")
    }).collect::<Vec<_>>().join("\n")
}

#[derive(thiserror::Error, Debug)]
pub enum GenerateTablesForIndexerSqlError {
    #[error("{0}")]
    ReadAbiError(#[from] ReadAbiError),

    #[error("{0}")]
    ParamTypeError(#[from] ParamTypeError),
}

/// If any event names match the whole table name should be exposed differently on graphql
/// to avoid clashing of graphql namings
fn find_clashing_event_names(
    project_path: &Path,
    current_contract_name: &str,
    other_contracts: &[Contract],
    current_event_names: &[EventInfo],
) -> Result<Vec<String>, GenerateTablesForIndexerSqlError> {
    let mut clashing_events = Vec::new();

    for other_contract in other_contracts {
        if other_contract.name == current_contract_name {
            continue;
        }

        let other_abi_items = ABIItem::read_abi_items(project_path, other_contract)?;
        let other_event_names =
            ABIItem::extract_event_names_and_signatures_from_abi(other_abi_items)?;

        for event_name in current_event_names {
            if other_event_names.iter().any(|e| e.name == event_name.name)
                && !clashing_events.contains(&event_name.name)
            {
                clashing_events.push(event_name.name.clone());
            }
        }
    }

    Ok(clashing_events)
}

pub fn generate_tables_for_indexer_sql(
    project_path: &Path,
    indexer: &Indexer,
    disable_event_tables: bool,
) -> Result<Code, GenerateTablesForIndexerSqlError> {
    let mut sql = "CREATE SCHEMA IF NOT EXISTS rindexer_internal;".to_string();

    for contract in &indexer.contracts {
        let contract_name = contract.before_modify_name_if_filter_readonly();
        let abi_items = ABIItem::read_abi_items(project_path, contract)?;
        let events = ABIItem::extract_event_names_and_signatures_from_abi(abi_items)?;
        let schema_name = generate_indexer_contract_schema_name(&indexer.name, &contract_name);
        let networks: Vec<&str> = contract.details.iter().map(|d| d.network.as_str()).collect();
        let factories = contract.details.iter().flat_map(|d| d.factory.clone()).collect::<Vec<_>>();

        if !disable_event_tables {
            sql.push_str(format!("CREATE SCHEMA IF NOT EXISTS {schema_name};").as_str());
            info!("Creating schema if not exists: {}", schema_name);

            let event_matching_name_on_other = find_clashing_event_names(
                project_path,
                &contract_name,
                &indexer.contracts,
                &events,
            )?;

            sql.push_str(&generate_event_table_sql_with_comments(
                &events,
                &contract.name,
                &schema_name,
                event_matching_name_on_other,
            ));
        }

        // we still need to create the internal tables for the contract
        sql.push_str(&generate_internal_event_table_sql(&events, &schema_name, networks));

        // generate internal tables for contract factories indexing
        sql.push_str(&generate_internal_factory_event_table_sql(&indexer.name, &factories));
    }

    if indexer.native_transfers.enabled {
        let contract_name = NATIVE_TRANSFER_CONTRACT_NAME.to_string();
        let abi_str = NATIVE_TRANSFER_ABI;
        let abi_items: Vec<ABIItem> =
            serde_json::from_str(abi_str).expect("JSON was not well-formatted");
        let event_names = ABIItem::extract_event_names_and_signatures_from_abi(abi_items)?;
        let schema_name = generate_indexer_contract_schema_name(&indexer.name, &contract_name);
        let networks = indexer.clone().native_transfers.networks.unwrap();
        let networks: Vec<&str> = networks.iter().map(|d| d.network.as_str()).collect();

        if !disable_event_tables {
            sql.push_str(format!("CREATE SCHEMA IF NOT EXISTS {schema_name};").as_str());
            info!("Creating schema if not exists: {}", schema_name);

            let event_matching_name_on_other = find_clashing_event_names(
                project_path,
                &contract_name,
                &indexer.contracts,
                &event_names,
            )?;

            sql.push_str(&generate_event_table_sql_with_comments(
                &event_names,
                &contract_name,
                &schema_name,
                event_matching_name_on_other,
            ));
        }
        sql.push_str(&generate_internal_event_table_sql(&event_names, &schema_name, networks));
    }

    sql.push_str(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS rindexer_internal.{indexer_name}_last_known_relationship_dropping_sql (
            key INT PRIMARY KEY,
            value TEXT NOT NULL
        );
    "#,
        indexer_name = camel_to_snake(&indexer.name)
    ));

    sql.push_str(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS rindexer_internal.{indexer_name}_last_known_indexes_dropping_sql (
            key INT PRIMARY KEY,
            value TEXT NOT NULL
        );
    "#,
        indexer_name = camel_to_snake(&indexer.name)
    ));

    Ok(Code::new(sql))
}

pub fn generate_event_table_full_name(
    indexer_name: &str,
    contract_name: &str,
    event_name: &str,
) -> String {
    let schema_name = generate_indexer_contract_schema_name(indexer_name, contract_name);
    format!("{}.{}", schema_name, camel_to_snake(event_name))
}

pub fn generate_event_table_columns_names_sql(column_names: &[String]) -> String {
    column_names.iter().map(|name| format!("\"{name}\"")).collect::<Vec<String>>().join(", ")
}

pub fn generate_indexer_contract_schema_name(indexer_name: &str, contract_name: &str) -> String {
    format!("{}_{}", camel_to_snake(indexer_name), camel_to_snake(contract_name))
}

pub fn generate_internal_event_table_name(schema_name: &str, event_name: &str) -> String {
    let table_name = format!("{}_{}", schema_name, camel_to_snake(event_name));

    compact_table_name_if_needed(table_name)
}

pub struct GenerateInternalFactoryEventTableNameParams {
    pub indexer_name: String,
    pub contract_name: String,
    pub event_name: String,
    pub input_name: String,
}

pub fn generate_internal_factory_event_table_name(
    params: &GenerateInternalFactoryEventTableNameParams,
) -> String {
    let schema_name =
        generate_indexer_contract_schema_name(&params.indexer_name, &params.contract_name);
    let table_name = format!(
        "{}_{}_{}",
        schema_name,
        camel_to_snake(&params.event_name),
        camel_to_snake(&params.input_name)
    );

    compact_table_name_if_needed(table_name)
}

pub fn drop_tables_for_indexer_sql(project_path: &Path, indexer: &Indexer) -> Code {
    let mut sql = format!(
        "DROP TABLE IF EXISTS rindexer_internal.{}_last_known_indexes_dropping_sql CASCADE;",
        camel_to_snake(&indexer.name)
    );
    sql.push_str(format!("DROP TABLE IF EXISTS rindexer_internal.{}_last_known_relationship_dropping_sql CASCADE;", camel_to_snake(&indexer.name)).as_str());

    for contract in &indexer.contracts {
        let contract_name = contract.before_modify_name_if_filter_readonly();
        let schema_name = generate_indexer_contract_schema_name(&indexer.name, &contract_name);
        sql.push_str(format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE;").as_str());

        // drop last synced blocks for contracts
        let abi_items = ABIItem::read_abi_items(project_path, contract);
        if let Ok(abi_items) = abi_items {
            for abi_item in abi_items.iter() {
                let table_name = generate_internal_event_table_name(&schema_name, &abi_item.name);
                sql.push_str(
                    format!("DROP TABLE IF EXISTS rindexer_internal.{table_name} CASCADE;")
                        .as_str(),
                );
            }
        } else {
            error!(
                "Could not read ABI items for contract moving on clearing the other data up: {}",
                contract.name
            );
        }

        // drop factory indexing tables
        for factory in contract.details.iter().flat_map(|d| d.factory.as_ref()) {
            let params = GenerateInternalFactoryEventTableNameParams {
                indexer_name: indexer.name.clone(),
                contract_name: factory.name.clone(),
                event_name: factory.event_name.clone(),
                input_name: factory.input_name.clone(),
            };
            let table_name = generate_internal_factory_event_table_name(&params);
            sql.push_str(
                format!("DROP TABLE IF EXISTS rindexer_internal.{table_name} CASCADE;").as_str(),
            )
        }
    }

    Code::new(sql)
}

#[allow(clippy::manual_strip)]
pub fn solidity_type_to_db_type(abi_type: &str) -> String {
    let is_array = abi_type.ends_with("[]");
    let base_type = abi_type.trim_end_matches("[]");

    let sql_type = match base_type {
        "address" => "CHAR(42)",
        "bool" => "BOOLEAN",
        "string" => "TEXT",
        t if t.starts_with("bytes") => "BYTEA",
        t if t.starts_with("int") || t.starts_with("uint") => {
            // Handling fixed-size integers (intN and uintN where N can be 8 to 256 in steps of 8)
            let (prefix, size): (&str, usize) = if t.starts_with("int") {
                ("int", t[3..].parse().expect("Invalid intN type"))
            } else {
                ("uint", t[4..].parse().expect("Invalid uintN type"))
            };

            match size {
                8 | 16 => "SMALLINT",
                24 | 32 => "INTEGER",
                40 | 48 | 56 | 64 | 72 | 80 | 88 | 96 | 104 | 112 | 120 | 128 => "NUMERIC",
                136 | 144 | 152 | 160 | 168 | 176 | 184 | 192 | 200 | 208 | 216 | 224 | 232
                | 240 | 248 | 256 => "VARCHAR(78)",
                _ => panic!("Unsupported {prefix}N size: {size}"),
            }
        }
        _ => panic!("Unsupported type: {base_type}"),
    };

    // Return the SQL type, appending array brackets if necessary
    if is_array {
        // CHAR(42)[] does not work nicely with parsers so using
        // TEXT[] works out the box and CHAR(42) doesnt protect much anyway
        // as its already in type Address
        if base_type == "address" {
            return "TEXT[]".to_string();
        }
        format!("{sql_type}[]")
    } else {
        sql_type.to_string()
    }
}
