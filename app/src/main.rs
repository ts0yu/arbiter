use eyre::Result;
mod cli;
mod tokens;
mod uniswap;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    // search tokens from CLI inputs
    let (tokens, bp) = utils::get_tokens_from_cli();

    // RPC endpoint [deault: alchemy]
    let provider = utils::get_provider().await;
    let uniswap_factory = uniswap::get_uniswapv3_factory(provider.clone());

    // Return addresses of UniswapV3 pools given a token pair
    let result_addresses = uniswap::get_pool_from_uniswap(&tokens, uniswap_factory, bp).await;
    println!("Uniswap Pool Result: {:#?}", result_addresses);

    // Monitor pool swap events
    if result_addresses.len() == 1 {
        let pool_object = uniswap::get_pool_objects(result_addresses[0], provider).await;
        uniswap::monitor_pool(&pool_object, &tokens).await;
    }

    Ok(())
}
