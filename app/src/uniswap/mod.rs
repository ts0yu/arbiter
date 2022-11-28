use bindings::i_uniswap_v3_pool::IUniswapV3Pool;
use bindings::uniswap_v3_factory::UniswapV3Factory;
use ethers::abi::Address;
use ethers::prelude::*;
use ethers::providers::Provider;
use num_bigfloat::BigFloat;
use std::fs::read;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle, Thread};
use tokio::select;

use crate::tokens::{self, Token};
use crate::utils;

// get uniswap factory from bindings
pub fn get_uniswapv3_factory(provider: Arc<Provider<Http>>) -> UniswapV3Factory<Provider<Http>> {
    let uniswap_v3_factory_address = "0x1F98431c8aD98523631AE4a59f267346ea31F984"
        .parse::<Address>()
        .unwrap();
    UniswapV3Factory::new(uniswap_v3_factory_address, provider)
}

// get pool address for specified tokens and fee
pub async fn get_pool_from_uniswap(
    tokens: &(Token, Token),
    factory: UniswapV3Factory<Provider<Http>>,
    bp: Option<String>,
) -> Vec<Address> {
    // BP options = 100, 500, 3000, 10000 [1bb, 5bp, 30bp, 100bp]
    let bp = Some(String::from(bp.unwrap()));
    let pool = match bp {
        // The division was valid
        Some(x) => match x.as_ref() {
            "1" => vec![factory
                .get_pool(tokens.0.address, tokens.1.address, 100)
                .call()
                .await
                .unwrap()
                .into()],
            "5" => vec![factory
                .get_pool(tokens.0.address, tokens.1.address, 500)
                .call()
                .await
                .unwrap()
                .into()],
            "30" => vec![factory
                .get_pool(tokens.0.address, tokens.1.address, 3000)
                .call()
                .await
                .unwrap()
                .into()],
            "100" => vec![factory
                .get_pool(tokens.0.address, tokens.1.address, 10000)
                .call()
                .await
                .unwrap()
                .into()],
            _ => panic!("Enter Valid bp [Example: 1, 5, 30, 100]"),
        },
        // in the future multi thread this
        None => vec![
            factory
                .get_pool(tokens.0.address, tokens.1.address, 100)
                .call()
                .await
                .unwrap(),
            factory
                .get_pool(tokens.0.address, tokens.1.address, 500)
                .call()
                .await
                .unwrap(),
            factory
                .get_pool(tokens.0.address, tokens.1.address, 3000)
                .call()
                .await
                .unwrap(),
            factory
                .get_pool(tokens.0.address, tokens.1.address, 10000)
                .call()
                .await
                .unwrap(),
        ],
    };
    pool
}
// Get pool obect bindings from address
pub async fn get_pool_objects(
    addresses: Vec<Address>,
    provider: Arc<Provider<Http>>,
) -> Vec<IUniswapV3Pool<Provider<Http>>> {
    let mut vec = vec![];
    for address in addresses {
        vec.push(IUniswapV3Pool::new(address, provider.clone()));
    }
    vec
}
pub async fn monitor_pools(
    pools: Vec<IUniswapV3Pool<Provider<Http>>>,
    tokens: (Token, Token),
) -> Vec<JoinHandle<()>> {
    let mut handles: Vec<JoinHandle<()>> = vec![];
    let token_resources = Arc::new(Mutex::new(tokens));
    for pool in pools {
        let pool_resource = Arc::new(pool);

        let pool = Arc::clone(&pool_resource);
        let tokens = Arc::clone(&token_resources);
        let handle = thread::spawn(move || monitor_thread_pool(pool, tokens));
        handles.push(handle);
    }
    handles
}
// Maybe need to use message passing with channels?
// Use the select macro!!!
pub async fn test_select_macro() {
    select! {}
}
pub fn monitor_thread_pool(
    pool: Arc<IUniswapV3Pool<ethers::providers::Provider<ethers::providers::Http>>>,
    tokens: Arc<Mutex<(tokens::Token, tokens::Token)>>,
) {
    let swap_event_filter = &pool.swap_filter();
    // let pool_token_0 = pool.token_0().call().await.unwrap()
}
//monitor event stream from pool
pub async fn monitor_pool(pool: &IUniswapV3Pool<Provider<Http>>, tokens: &(Token, Token)) {
    let swap_events = pool.swap_filter();
    let pool_token_0 = pool.token_0().call().await.unwrap();
    let mut swap_stream = swap_events.stream().await.unwrap();
    while let Some(Ok(event)) = swap_stream.next().await {
        println!("------------New Swap------------");
        println!("From pool {:#?}", pool.address());
        println!(
            "Sender: {:#?}, Recipient: {:#?}",
            event.sender, event.recipient
        ); // H160s
        println!("amount_0 {:#?}", event.amount_0); // I256
        println!("amount_1 {:#?}", event.amount_1); // I256
        println!("liquidity {:#?}", event.liquidity); // u128
        println!("tick {:#?}", event.tick); // i32
        println!(
            "price {:#?}",
            compute_price(tokens.clone(), event.sqrt_price_x96, pool_token_0,)
        )
    }
}
pub fn compute_price(tokens: (Token, Token), sqrt_price_x96: U256, pool_token_0: H160) -> BigFloat {
    // Takes in UniswapV3's sqrt_price_x96 (a q64_96 fixed point number) and outputs the price in human readable form.
    // See Uniswap's documentation: https://docs.uniswap.org/sdk/guides/fetching-prices
    let diff_decimals: BigFloat = ((tokens.0.decimals as i16) - (tokens.1.decimals as i16)).into();
    if pool_token_0 == tokens.0.address {
        utils::convert_q64_96(sqrt_price_x96)
            .pow(&BigFloat::from_i16(2))
            .div(&BigFloat::from_i16(10).pow(&-diff_decimals))
    } else {
        BigFloat::from_i16(1).div(
            &utils::convert_q64_96(sqrt_price_x96)
                .pow(&BigFloat::from_i16(2))
                .div(&BigFloat::from_i16(10).pow(&diff_decimals)),
        )
    }
}
#[cfg(test)]
mod tests {
    use crate::{tokens, uniswap, utils};
    use ethers::{abi::Address, providers::*};
    use std::sync::Arc;

    use super::get_pool_from_uniswap;
    #[tokio::test]
    async fn test_get_pool_from_uniswap_1() {
        let provider: Arc<Provider<Http>> = utils::get_provider();
        let tokens = tokens::get_tokens();
        let factory = uniswap::get_uniswapv3_factory(provider.clone());

        let (test_tokens, bp) = (
            (
                tokens.get("ETH").unwrap().to_owned(),
                tokens.get("USDC").unwrap().to_owned(),
            ),
            Some(String::from("1")),
        );
        let pool = get_pool_from_uniswap(&test_tokens, factory.clone(), bp).await;
        assert_eq!(
            pool[0],
            "0xe0554a476a092703abdb3ef35c80e0d76d32939f"
                .parse::<Address>()
                .unwrap()
        );
    }
    #[tokio::test]
    async fn test_get_pool_from_uniswap_5() {
        let provider: Arc<Provider<Http>> = utils::get_provider();
        let tokens = tokens::get_tokens();
        let factory = uniswap::get_uniswapv3_factory(provider.clone());

        let (test_tokens, bp) = (
            (
                tokens.get("ETH").unwrap().to_owned(),
                tokens.get("USDC").unwrap().to_owned(),
            ),
            Some(String::from("5")),
        );
        let pool = get_pool_from_uniswap(&test_tokens, factory.clone(), bp).await;
        assert_eq!(
            pool[1],
            "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640"
                .parse::<Address>()
                .unwrap()
        );
    }
    #[tokio::test]
    async fn test_get_pool_from_uniswap_30() {
        let provider: Arc<Provider<Http>> = utils::get_provider();
        let tokens = tokens::get_tokens();
        let factory = uniswap::get_uniswapv3_factory(provider.clone());

        let (test_tokens, bp) = (
            (
                tokens.get("ETH").unwrap().to_owned(),
                tokens.get("USDC").unwrap().to_owned(),
            ),
            Some(String::from("30")),
        );
        let pool = get_pool_from_uniswap(&test_tokens, factory.clone(), bp).await;
        assert_eq!(
            pool[2],
            "0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8"
                .parse::<Address>()
                .unwrap()
        );
    }
    #[tokio::test]
    async fn test_get_pool_from_uniswap_100() {
        let provider: Arc<Provider<Http>> = utils::get_provider();
        let tokens = tokens::get_tokens();
        let factory = uniswap::get_uniswapv3_factory(provider.clone());
        let (test_tokens, bp) = (
            (
                tokens.get("ETH").unwrap().to_owned(),
                tokens.get("USDC").unwrap().to_owned(),
            ),
            Some(String::from("100")),
        );
        let pool = get_pool_from_uniswap(&test_tokens, factory.clone(), bp).await;
        assert_eq!(
            pool[3],
            "0x7bea39867e4169dbe237d55c8242a8f2fcdcc387"
                .parse::<Address>()
                .unwrap()
        );
    }
    #[tokio::test]
    #[should_panic]
    async fn test_get_pool_from_uniswap_700() {
        let provider: Arc<Provider<Http>> = utils::get_provider();
        let tokens = tokens::get_tokens();
        let factory = uniswap::get_uniswapv3_factory(provider.clone());
        let (test_tokens, bp) = (
            (
                tokens.get("ETH").unwrap().to_owned(),
                tokens.get("USDC").unwrap().to_owned(),
            ),
            Some(String::from("700")),
        );
        get_pool_from_uniswap(&test_tokens, factory.clone(), bp).await;
    }
}
