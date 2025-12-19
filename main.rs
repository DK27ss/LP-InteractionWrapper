use clap::{Parser, Subcommand};
use ethers::{
    prelude::*,
    utils::parse_ether,
};
use eyre::Result;
use std::sync::Arc;

const RPC_URL: &str = "RPC_ADDRESS";
const PRIVATE_KEY: &str = "PRIVATE_KEY";

#[derive(Parser)]
#[command(name = "universal-cli")]
#[command(about = "Universal CLI to trigger ADD/REMOVE on any PancakeV3Pool", long_about = None)]
struct Cli {
    #[arg(short = 'c', long, env = "CONTRACT_ADDRESS")]
    contract: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    SetPool {
        #[arg(short, long)]
        pool: String,
    },
    Info,
    Add {
        #[arg(short, long)]
        base: f64,
        #[arg(short, long)]
        quote: f64,
        #[arg(short = 'n', long, default_value = "1")]
        count: u32,
    },
    Remove {
        #[arg(short, long)]
        id: Option<u64>,
        #[arg(short = 'n', long)]
        count: Option<usize>,
        #[arg(short, long)]
        all: bool,
    },
    Rebalance {
        #[arg(short, long)]
        id: Option<u64>,
        #[arg(short = 'n', long)]
        count: Option<u32>,
    },
    Buy {
        #[arg(short, long)]
        amount: f64,
        #[arg(short = 'n', long, default_value = "1")]
        count: u32,
    },
    Sell {
        #[arg(short, long)]
        amount: f64,
        #[arg(short = 'n', long, default_value = "1")]
        count: u32,
    },
    Balance,
    Positions,
    SetTicks {
        #[arg(short, long)]
        lower: i32,
        #[arg(short, long)]
        upper: i32,
    },
    Withdraw,
    WithdrawToken {
        #[arg(short, long)]
        token: String,
    },
    Fund {
        #[arg(short, long)]
        token: String,
        #[arg(short, long)]
        amount: f64,
    },
}

abigen!(
    UniversalTrigger,
    r#"[
        function setPool(address _pool) external
        function setPoolFull(address _pool, address _baseToken, address _quoteToken) external
        function setDefaultTicks(int24 _lower, int24 _upper) external
        function addPositionDefault(uint256 baseAmount, uint256 quoteAmount) external returns (uint256 tokenId)
        function addPositionBatch(uint256 basePerPos, uint256 quotePerPos, uint256 count) external returns (uint256[] memory tokenIds)
        function closePosition(uint256 tokenId) external
        function closeAllPositions() external
        function closeBatchPositions(uint256 count) external
        function rebalanceDefault(uint256 tokenId) external returns (uint256 newTokenId)
        function rebalanceBatchDefault(uint256 count) external returns (uint256[] memory newTokenIds)
        function swapBaseForQuote(uint256 amountIn) external
        function swapQuoteForBase(uint256 amountIn) external
        function swapBaseForQuoteBatch(uint256 amountPerSwap, uint256 count) external
        function swapQuoteForBaseBatch(uint256 amountPerSwap, uint256 count) external
        function getBalances() external view returns (uint256 baseBalance, uint256 quoteBalance)
        function getPoolInfo() external view returns (address pool, address base, address quote, uint24 fee, int24 currentTick)
        function getActivePositions() external view returns (uint256[] memory activeIds, uint128[] memory liquidities)
        function withdrawAll() external
        function withdrawToken(address token) external
        function defaultTickLower() external view returns (int24)
        function defaultTickUpper() external view returns (int24)
        event PoolConfigured(address indexed pool, address baseToken, address quoteToken, uint24 fee)
        event PositionCreated(uint256 indexed tokenId, int24 tickLower, int24 tickUpper, uint128 liquidity)
        event PositionClosed(uint256 indexed tokenId)
        event Rebalanced(uint256 indexed oldTokenId, uint256 indexed newTokenId, int24 newTickLower, int24 newTickUpper)
    ]"#
);

abigen!(
    IERC20,
    r#"[
        function balanceOf(address account) external view returns (uint256)
        function transfer(address to, uint256 amount) external returns (bool)
        function approve(address spender, uint256 amount) external returns (bool)
        function symbol() external view returns (string)
        function decimals() external view returns (uint8)
    ]"#
);

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let provider = Provider::<Http>::try_from(RPC_URL)?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet: LocalWallet = PRIVATE_KEY.parse::<LocalWallet>()?.with_chain_id(chain_id);
    let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet));
    let contract_address: Address = cli.contract.parse()?;
    let contract = UniversalTrigger::new(contract_address, client.clone());

    match cli.command {
        Commands::SetPool { pool } => {
            println!("Pool config: {}", pool);
            let pool_addr: Address = pool.parse()?;
            let tx = contract
                .set_pool(pool_addr)
                .gas(200_000u64)
                .legacy();

            let pending = tx.send().await?;
            println!("TX: {:?}", pending.tx_hash());
            let receipt = pending.await?;
            if let Some(r) = receipt {
                println!("Pool configuré - Bloc: {}", r.block_number.unwrap_or_default());
            }

            let (pool, base, quote, fee, tick) = contract.get_pool_info().call().await?;
            println!("\n=== Pool Info ===");
            println!("Pool:   {:?}", pool);
            println!("Base:   {:?}", base);
            println!("Quote:  {:?}", quote);
            println!("Fee:    {} ({}%)", fee, fee as f64 / 10000.0);
            println!("Tick:   {}", tick);
        }

        Commands::Info => {
            let (pool, base, quote, fee, tick) = contract.get_pool_info().call().await?;
            let lower = contract.default_tick_lower().call().await?;
            let upper = contract.default_tick_upper().call().await?;
            println!("=== Pool Configuration ===");
            println!("Pool:       {:?}", pool);
            println!("Base:       {:?}", base);
            println!("Quote:      {:?}", quote);
            println!("Fee:        {} ({}%)", fee, fee as f64 / 10000.0);
            println!("Tick:       {}", tick);
            println!("Ticks def:  [{}, {}]", lower, upper);
            let (base_bal, quote_bal) = contract.get_balances().call().await?;
            println!("\n=== Balances ===");
            println!("Base:  {}", format_ether(base_bal));
            println!("Quote: {}", format_ether(quote_bal));
            let (ids, _) = contract.get_active_positions().call().await?;
            println!("\n=== Positions: {} ===", ids.len());
        }

        Commands::Add { base, quote, count } => {
            let base_amount = parse_ether(base)?;
            let quote_amount = parse_ether(quote)?;
            if count == 1 {
                println!("ADD: {} base + {} quote", base, quote);
                let tx = contract
                    .add_position_default(base_amount, quote_amount)
                    .gas(800_000u64)
                    .legacy();
                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    for log in r.logs {
                        if log.topics.len() >= 2 {
                            let topic = format!("{:?}", log.topics[0]);
                            if topic.contains("9314bc80") {
                                let token_id = U256::from_big_endian(log.topics[1].as_bytes());
                                println!("Position {} created", token_id);
                            }
                        }
                    }
                    println!("Gas: {}", r.gas_used.unwrap_or_default());
                }
            } else {
                println!("ADD BATCH: {} positions × ({} base + {} quote)", count, base, quote);
                let gas = 600_000u64 * count as u64;
                let tx = contract
                    .add_position_batch(base_amount, quote_amount, U256::from(count))
                    .gas(gas.min(8_000_000))
                    .legacy();

                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    let mut created = 0u32;
                    for log in r.logs {
                        if log.topics.len() >= 2 {
                            let topic = format!("{:?}", log.topics[0]);
                            if topic.contains("9314bc80") {
                                created += 1;
                            }
                        }
                    }
                    println!("{}/{} positions created (1 TX)", created, count);
                    println!("Gas: {}", r.gas_used.unwrap_or_default());
                }
            }

            let (base_bal, quote_bal) = contract.get_balances().call().await?;
            println!("\nBalance: {} base | {} quote", format_ether(base_bal), format_ether(quote_bal));
        }

        Commands::Remove { id, count, all } => {
            if let Some(position_id) = id {
                println!("REMOVE: position {}", position_id);
                let tx = contract
                    .close_position(U256::from(position_id))
                    .gas(500_000u64)
                    .legacy();

                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("Position {} closed - Gas: {}", position_id, r.gas_used.unwrap_or_default());
                }
            } else if all {
                let (ids, _) = contract.get_active_positions().call().await?;
                if ids.is_empty() {
                    println!("No active position.");
                    return Ok(());
                }

                println!("REMOVE ALL: {} positions in 1 TX", ids.len());
                let gas = 300_000u64 * ids.len() as u64;
                let tx = contract
                    .close_all_positions()
                    .gas(gas.min(8_000_000))
                    .legacy();

                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("{} positions closed! - Gas: {}", ids.len(), r.gas_used.unwrap_or_default());
                }
            } else if let Some(n) = count {
                let (ids, _) = contract.get_active_positions().call().await?;
                if ids.is_empty() {
                    println!("No active position.");
                    return Ok(());
                }

                let to_close = n.min(ids.len());
                println!("REMOVE: {} positions in 1 TX", to_close);

                let gas = 300_000u64 * to_close as u64;
                let tx = contract
                    .close_batch_positions(U256::from(to_close))
                    .gas(gas.min(8_000_000))
                    .legacy();

                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("{} positions closed - Gas: {}", to_close, r.gas_used.unwrap_or_default());
                }
            } else {
                println!("Usage: remove --id <ID> | --count <N> | --all");
            }

            let (base_bal, quote_bal) = contract.get_balances().call().await?;
            println!("\nBalance: {} base | {} quote", format_ether(base_bal), format_ether(quote_bal));
        }

        Commands::Rebalance { id, count } => {
            if let Some(position_id) = id {
                println!("REBALANCE: position {} (REMOVE → ADD en 1 TX)", position_id);
                let tx = contract
                    .rebalance_default(U256::from(position_id))
                    .gas(800_000u64)
                    .legacy();

                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("Rebalanced - Gas: {}", r.gas_used.unwrap_or_default());
                }
            } else if let Some(n) = count {
                let (ids, _) = contract.get_active_positions().call().await?;
                if ids.is_empty() {
                    println!("No active position.");
                    return Ok(());
                }

                let to_rebalance = (n as usize).min(ids.len());
                println!("REBALANCE BATCH: {} positions (REMOVE×{} → ADD×{} en 1 TX)",
                    to_rebalance, to_rebalance, to_rebalance);
                let gas = 600_000u64 * to_rebalance as u64;
                let tx = contract
                    .rebalance_batch_default(U256::from(to_rebalance))
                    .gas(gas.min(8_000_000))
                    .legacy();
                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("{} positions rebalanced! - Gas: {}", to_rebalance, r.gas_used.unwrap_or_default());
                }
            } else {
                println!("Usage: rebalance --id <ID> | --count <N>");
            }

            let (base_bal, quote_bal) = contract.get_balances().call().await?;
            println!("\nBalance: {} base | {} quote", format_ether(base_bal), format_ether(quote_bal));
        }

        Commands::Buy { amount, count } => {
            let amount_in = parse_ether(amount)?;
            if count == 1 {
                println!("BUY: {} base -> quote", amount);
                let tx = contract
                    .swap_base_for_quote(amount_in)
                    .gas(300_000u64)
                    .legacy();
                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("Swap sended! - Gas: {}", r.gas_used.unwrap_or_default());
                }
            } else {
                println!("BUY BATCH: {} swaps × {} base", count, amount);
                let gas = 250_000u64 * count as u64;
                let tx = contract
                    .swap_base_for_quote_batch(amount_in, U256::from(count))
                    .gas(gas.min(8_000_000))
                    .legacy();
                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("{} swaps sended! - Gas: {}", count, r.gas_used.unwrap_or_default());
                }
            }

            let (base_bal, quote_bal) = contract.get_balances().call().await?;
            println!("\nBalance: {} base | {} quote", format_ether(base_bal), format_ether(quote_bal));
        }

        Commands::Sell { amount, count } => {
            let amount_in = parse_ether(amount)?;
            if count == 1 {
                println!("SELL: {} quote -> base", amount);
                let tx = contract
                    .swap_quote_for_base(amount_in)
                    .gas(300_000u64)
                    .legacy();
                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("Swap sended! - Gas: {}", r.gas_used.unwrap_or_default());
                }
            } else {
                println!("SELL BATCH: {} swaps × {} quote", count, amount);
                let gas = 250_000u64 * count as u64;
                let tx = contract
                    .swap_quote_for_base_batch(amount_in, U256::from(count))
                    .gas(gas.min(8_000_000))
                    .legacy();
                let pending = tx.send().await?;
                println!("TX: {:?}", pending.tx_hash());
                let receipt = pending.await?;
                if let Some(r) = receipt {
                    println!("{} swaps sended! - Gas: {}", count, r.gas_used.unwrap_or_default());
                }
            }

            let (base_bal, quote_bal) = contract.get_balances().call().await?;
            println!("\nBalance: {} base | {} quote", format_ether(base_bal), format_ether(quote_bal));
        }

        Commands::Balance => {
            let (base_bal, quote_bal) = contract.get_balances().call().await?;
            println!("=== Contract balance ===");
            println!("Base:  {}", format_ether(base_bal));
            println!("Quote: {}", format_ether(quote_bal));
            let (ids, _) = contract.get_active_positions().call().await?;
            println!("\n=== Positions: {} ===", ids.len());
        }

        Commands::Positions => {
            let (ids, liquidities) = contract.get_active_positions().call().await?;
            if ids.is_empty() {
                println!("No active position");
            } else {
                println!("=== {} Active positions ===", ids.len());
                for (id, liq) in ids.iter().zip(liquidities.iter()) {
                    println!("  #{}: liquidity = {}", id, liq);
                }
            }
        }

        Commands::SetTicks { lower, upper } => {
            println!("Ticks configurations: [{}, {}]", lower, upper);
            let tx = contract
                .set_default_ticks(lower as i32, upper as i32)
                .gas(100_000u64)
                .legacy();
            let pending = tx.send().await?;
            println!("TX: {:?}", pending.tx_hash());
            let receipt = pending.await?;
            if let Some(r) = receipt {
                println!("Ticks configured - Block: {}", r.block_number.unwrap_or_default());
            }
        }

        Commands::Withdraw => {
            println!("all balance withdraw process...");
            let tx = contract
                .withdraw_all()
                .gas(150_000u64)
                .legacy();
            let pending = tx.send().await?;
            println!("TX: {:?}", pending.tx_hash());
            let receipt = pending.await?;
            if let Some(r) = receipt {
                println!("Balance withdrawed - Block: {}", r.block_number.unwrap_or_default());
            }
        }

        Commands::WithdrawToken { token } => {
            println!("Withdraw token: {}", token);
            let token_addr: Address = token.parse()?;
            let tx = contract
                .withdraw_token(token_addr)
                .gas(100_000u64)
                .legacy();
            let pending = tx.send().await?;
            println!("TX: {:?}", pending.tx_hash());
            let receipt = pending.await?;
            if let Some(r) = receipt {
                println!("Token withdrawed - Block: {}", r.block_number.unwrap_or_default());
            }
        }

        Commands::Fund { token, amount } => {
            println!("sending {} to contract...", amount);
            let token_addr: Address = token.parse()?;
            let token_contract = IERC20::new(token_addr, client.clone());
            let amount_wei = parse_ether(amount)?;
            let tx = token_contract
                .transfer(contract_address, amount_wei)
                .gas(60_000u64)
                .legacy();
            let pending = tx.send().await?;
            println!("TX: {:?}", pending.tx_hash());
            let receipt = pending.await?;
            if let Some(r) = receipt {
                println!("Tokens send - Block: {}", r.block_number.unwrap_or_default());
            }
            let (base_bal, quote_bal) = contract.get_balances().call().await?;
            println!("\nBalance: {} base | {} quote", format_ether(base_bal), format_ether(quote_bal));
        }
    }

    Ok(())
}

fn format_ether(wei: U256) -> String {
    let wei_str = wei.to_string();
    let len = wei_str.len();
    if len <= 18 {
        let zeros = "0".repeat(18 - len);
        let decimal = format!("{}{}", zeros, wei_str);
        let trimmed = decimal.trim_end_matches('0');
        if trimmed.is_empty() {
            "0".to_string()
        } else {
            format!("0.{}", trimmed)
        }
    } else {
        let (integer, decimal) = wei_str.split_at(len - 18);
        let decimal_trimmed = decimal.trim_end_matches('0');
        if decimal_trimmed.is_empty() {
            integer.to_string()
        } else {
            format!("{}.{}", integer, &decimal[..4.min(decimal.len())])
        }
    }
}
