#![allow(
    non_camel_case_types,
    clippy::enum_variant_names,
    clippy::too_many_arguments,
    clippy::upper_case_acronyms,
    clippy::type_complexity,
    dead_code
)]
use super::super::super::super::typings::database::get_or_init_postgres_client;
use super::super::super::super::typings::networks::get_provider_cache_for_network;
/// THIS IS A GENERATED FILE. DO NOT MODIFY MANUALLY.
///
/// This file was auto generated by rindexer - https://github.com/joshstevens19/rindexer.
/// Any manual changes to this file will be overwritten.
use super::playground_types_filter_abi_gen::RindexerPlaygroundTypesFilterGen::{
    self, RindexerPlaygroundTypesFilterGenEvents, RindexerPlaygroundTypesFilterGenInstance,
};
use alloy::network::AnyNetwork;
use alloy::primitives::{Address, B256, Bytes};
use alloy::sol_types::{SolEvent, SolEventInterface, SolType};
use rindexer::{
    AsyncCsvAppender, FutureExt, PostgresClient, async_trait,
    event::{
        callback_registry::{
            EventCallbackRegistry, EventCallbackRegistryInformation, EventCallbackResult,
            EventResult, HasTxInformation, TxInformation,
        },
        contract_setup::{ContractInformation, NetworkContract},
    },
    generate_random_id,
    manifest::{
        contract::{Contract, ContractDetails},
        yaml::read_manifest,
    },
    provider::{JsonRpcCachedProvider, RindexerProvider},
};
use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::{any::Any, sync::Arc};

pub type SwapData = RindexerPlaygroundTypesFilterGen::Swap;

#[derive(Debug, Clone)]
pub struct SwapResult {
    pub event_data: SwapData,
    pub tx_information: TxInformation,
}

impl HasTxInformation for SwapResult {
    fn tx_information(&self) -> &TxInformation {
        &self.tx_information
    }
}

pub type Two_WordData = RindexerPlaygroundTypesFilterGen::Two_Word;

#[derive(Debug, Clone)]
pub struct Two_WordResult {
    pub event_data: Two_WordData,
    pub tx_information: TxInformation,
}

impl HasTxInformation for Two_WordResult {
    fn tx_information(&self) -> &TxInformation {
        &self.tx_information
    }
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[async_trait]
trait EventCallback {
    async fn call(&self, events: Vec<EventResult>) -> EventCallbackResult<()>;
}

pub struct EventContext<TExtensions>
where
    TExtensions: Send + Sync,
{
    pub database: Arc<PostgresClient>,
    pub csv: Arc<AsyncCsvAppender>,
    pub extensions: Arc<TExtensions>,
}

// didn't want to use option or none made harder DX
// so a blank struct makes interface nice
pub struct NoExtensions {}
pub fn no_extensions() -> NoExtensions {
    NoExtensions {}
}

pub fn swap_handler<TExtensions, F, Fut>(custom_logic: F) -> SwapEventCallbackType<TExtensions>
where
    SwapResult: Clone + 'static,
    F: for<'a> Fn(Vec<SwapResult>, Arc<EventContext<TExtensions>>) -> Fut
        + Send
        + Sync
        + 'static
        + Clone,
    Fut: Future<Output = EventCallbackResult<()>> + Send + 'static,
    TExtensions: Send + Sync + 'static,
{
    Arc::new(move |results, context| {
        let custom_logic = custom_logic.clone();
        let results = results.clone();
        let context = Arc::clone(&context);
        async move { (custom_logic)(results, context).await }.boxed()
    })
}

type SwapEventCallbackType<TExtensions> = Arc<
    dyn for<'a> Fn(
            &'a Vec<SwapResult>,
            Arc<EventContext<TExtensions>>,
        ) -> BoxFuture<'a, EventCallbackResult<()>>
        + Send
        + Sync,
>;

pub struct SwapEvent<TExtensions>
where
    TExtensions: Send + Sync + 'static,
{
    callback: SwapEventCallbackType<TExtensions>,
    context: Arc<EventContext<TExtensions>>,
}

impl<TExtensions> SwapEvent<TExtensions>
where
    TExtensions: Send + Sync + 'static,
{
    pub async fn handler<F, Fut>(closure: F, extensions: TExtensions) -> Self
    where
        SwapResult: Clone + 'static,
        F: for<'a> Fn(Vec<SwapResult>, Arc<EventContext<TExtensions>>) -> Fut
            + Send
            + Sync
            + 'static
            + Clone,
        Fut: Future<Output = EventCallbackResult<()>> + Send + 'static,
    {
        let csv = AsyncCsvAppender::new(
            r"/Users/jackedgson/Development/avara/rindexer/rindexer_rust_playground/generated_csv/PlaygroundTypesFilter/playgroundtypesfilter-swap.csv",
        );
        if !Path::new(r"/Users/jackedgson/Development/avara/rindexer/rindexer_rust_playground/generated_csv/PlaygroundTypesFilter/playgroundtypesfilter-swap.csv").exists() {
            csv.append_header(vec!["contract_address".into(), "sender".into(), "recipient".into(), "amount_0".into(), "amount_1".into(), "sqrt_price_x96".into(), "liquidity".into(), "tick".into(), "tick_2".into(), "tick_3".into(), "tick_4".into(), "tick_5".into(), "tick_6".into(), "tick_7".into(), "tx_hash".into(), "block_number".into(), "block_hash".into(), "network".into(), "tx_index".into(), "log_index".into()].into())
                .await
                .expect("Failed to write CSV header");
        }

        Self {
            callback: swap_handler(closure),
            context: Arc::new(EventContext {
                database: get_or_init_postgres_client().await,
                csv: Arc::new(csv),
                extensions: Arc::new(extensions),
            }),
        }
    }
}

#[async_trait]
impl<TExtensions> EventCallback for SwapEvent<TExtensions>
where
    TExtensions: Send + Sync,
{
    async fn call(&self, events: Vec<EventResult>) -> EventCallbackResult<()> {
        // note some can not downcast because it cant decode
        // this happens on events which failed decoding due to
        // not having the right abi for example
        // transfer events with 2 indexed topics cant decode
        // transfer events with 3 indexed topics
        let result: Vec<SwapResult> = events
            .into_iter()
            .filter_map(|item| {
                item.decoded_data.downcast::<SwapData>().ok().map(|arc| SwapResult {
                    event_data: (*arc).clone(),
                    tx_information: item.tx_information,
                })
            })
            .collect();

        (self.callback)(&result, Arc::clone(&self.context)).await
    }
}

pub enum PlaygroundTypesFilterEventType<TExtensions>
where
    TExtensions: 'static + Send + Sync,
{
    Swap(SwapEvent<TExtensions>),
}

pub async fn playground_types_filter_contract(
    network: &str,
    address: Address,
) -> RindexerPlaygroundTypesFilterGenInstance<Arc<RindexerProvider>, AnyNetwork> {
    RindexerPlaygroundTypesFilterGen::new(
        address,
        get_provider_cache_for_network(network).await.get_inner_provider(),
    )
}

pub async fn decoder_contract(
    network: &str,
) -> RindexerPlaygroundTypesFilterGenInstance<Arc<RindexerProvider>, AnyNetwork> {
    if network == "base" {
        RindexerPlaygroundTypesFilterGen::new(
            // do not care about address here its decoding makes it easier to handle ValueOrArray
            Address::ZERO,
            get_provider_cache_for_network(network).await.get_inner_provider(),
        )
    } else {
        panic!("Network not supported");
    }
}

impl<TExtensions> PlaygroundTypesFilterEventType<TExtensions>
where
    TExtensions: 'static + Send + Sync,
{
    pub fn topic_id(&self) -> &'static str {
        match self {
            PlaygroundTypesFilterEventType::Swap(_) => {
                "0xd05a7c246337c1ef3460a6bf033445dc2004f4e65a9c755d82104f902aa300a4"
            }
        }
    }

    pub fn event_name(&self) -> &'static str {
        match self {
            PlaygroundTypesFilterEventType::Swap(_) => "Swap",
        }
    }

    pub fn contract_name(&self) -> String {
        "PlaygroundTypes".to_string()
    }

    async fn get_provider(&self, network: &str) -> Arc<JsonRpcCachedProvider> {
        get_provider_cache_for_network(network).await
    }

    fn decoder(
        &self,
        network: &str,
    ) -> Arc<dyn Fn(Vec<B256>, Bytes) -> Arc<dyn Any + Send + Sync> + Send + Sync> {
        let decoder_contract = decoder_contract(network);

        match self {
            PlaygroundTypesFilterEventType::Swap(_) => {
                Arc::new(move |topics: Vec<B256>, data: Bytes| {
                    match SwapData::decode_raw_log(topics, &data[0..]) {
                        Ok(event) => {
                            let result: SwapData = event;
                            Arc::new(result) as Arc<dyn Any + Send + Sync>
                        }
                        Err(error) => Arc::new(error) as Arc<dyn Any + Send + Sync>,
                    }
                })
            }
        }
    }

    pub async fn register(self, manifest_path: &PathBuf, registry: &mut EventCallbackRegistry) {
        let rindexer_yaml = read_manifest(manifest_path).expect("Failed to read rindexer.yaml");
        let topic_id = self.topic_id();
        let contract_name = self.contract_name();
        let event_name = self.event_name();

        let contract_details = rindexer_yaml
            .contracts
            .iter()
            .find(|c| c.name == contract_name)
            .unwrap_or_else(|| {
                panic!(
                    "Contract {} not found please make sure its defined in the rindexer.yaml",
                    contract_name
                )
            })
            .clone();

        let index_event_in_order = contract_details
            .index_event_in_order
            .as_ref()
            .map_or(false, |vec| vec.contains(&event_name.to_string()));

        // Expect providers to have been initialized, but it's an async init so this should
        // be fast but for correctness we must await each future.
        let mut providers = HashMap::new();
        for n in contract_details.details.iter() {
            let provider = self.get_provider(&n.network).await;
            providers.insert(n.network.clone(), provider);
        }

        let contract = ContractInformation {
            name: contract_details.before_modify_name_if_filter_readonly().into_owned(),
            details: contract_details
                .details
                .iter()
                .map(|c| NetworkContract {
                    id: generate_random_id(10),
                    network: c.network.clone(),
                    cached_provider: providers
                        .get(&c.network)
                        .expect("must have a provider")
                        .clone(),
                    decoder: self.decoder(&c.network),
                    indexing_contract_setup: c.indexing_contract_setup(manifest_path),
                    start_block: c.start_block,
                    end_block: c.end_block,
                    disable_logs_bloom_checks: rindexer_yaml
                        .networks
                        .iter()
                        .find(|n| n.name == c.network)
                        .map_or(false, |n| n.disable_logs_bloom_checks.unwrap_or_default()),
                })
                .collect(),
            abi: contract_details.abi,
            reorg_safe_distance: contract_details.reorg_safe_distance.unwrap_or_default(),
        };

        let callback: Arc<
            dyn Fn(Vec<EventResult>) -> BoxFuture<'static, EventCallbackResult<()>> + Send + Sync,
        > = match self {
            PlaygroundTypesFilterEventType::Swap(event) => {
                let event = Arc::new(event);
                Arc::new(move |result| {
                    let event = Arc::clone(&event);
                    async move { event.call(result).await }.boxed()
                })
            }
        };

        registry.register_event(EventCallbackRegistryInformation {
            id: generate_random_id(10),
            indexer_name: "RindexerPlayground".to_string(),
            event_name: event_name.to_string(),
            index_event_in_order,
            topic_id: topic_id.parse::<B256>().unwrap(),
            contract,
            callback,
        });
    }
}
