//! CLI for fetching one or more Solana transactions by signature and
//! pretty-printing the swaps recognized by `parse::parse_transaction`.
//!
//! Amounts are shown human-adjusted for each mint's decimals, and mints are
//! labeled with their symbol (falling back to name, then to a shortened
//! pubkey) using `fetch_mint_infos`.
//!
//! Usage:
//!   graceful-cli <SIGNATURE> [<SIGNATURE> ...] [--rpc-url <URL>] [--detail full|summary] [--style tree|compact|table]
//!

mod infos;

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use clap::{Parser, ValueEnum};
use colored::Colorize;
use infos::{RawMintInfo, fetch_mint_infos};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction_status::UiTransactionEncoding;

use graceful::{
    parse::{
        DexSwap, ParseTxError, Parsed,
        aggregator::{AggregatorSwap, Route},
        parse_transaction,
    },
    swap::{Program, Swap},
    transaction::instruction::Path,
    util::transfer::TokenTransfer,
};

/// Lookup of mint pubkey to its decoded info, used to adjust raw amounts by
/// decimals and to label mints by symbol/name instead of raw pubkey.
type MintMap = HashMap<Pubkey, RawMintInfo>;

#[derive(Parser, Debug)]
#[command(
    name = "graceful-cli",
    about = "Fetch Solana transaction(s) by signature and pretty-print recognized swaps"
)]
struct Args {
    /// One or more transaction signatures to fetch and parse.
    #[arg(required = true)]
    signatures: Vec<String>,

    /// RPC endpoint to fetch transactions and mint info from.
    #[arg(long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc_url: String,

    /// "full" expands aggregator swap routes; "summary" only shows the net
    /// swap for aggregators.
    #[arg(long, value_enum, default_value = "full")]
    detail: Detail,

    /// Output style. A few alternatives are provided so you can compare
    /// them and pick a favorite.
    #[arg(long, value_enum, default_value = "tree")]
    style: Style,

    /// Disable colored output.
    #[arg(long)]
    no_color: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Detail {
    Full,
    Summary,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Style {
    Tree,
    Compact,
    Table,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().unwrap();
    env_logger::init();

    let args = Args::parse();

    if args.no_color {
        colored::control::set_override(false);
    }

    let rpc = RpcClient::new(args.rpc_url.clone());

    for (i, sig_str) in args.signatures.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_signature_header(sig_str);

        let signature = match Signature::from_str(sig_str) {
            Ok(sig) => sig,
            Err(e) => {
                print_error(&format!("invalid signature: {e}"));
                continue;
            }
        };

        let tx = match rpc
            .get_transaction_with_config(
                &signature,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Base64),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                },
            )
            .await
        {
            Ok(tx) => tx,
            Err(e) => {
                print_error(&format!("failed to fetch transaction: {e}"));
                continue;
            }
        };

        let swaps = match parse_transaction(tx) {
            Ok(swaps) if swaps.is_empty() => {
                continue;
            }
            Ok(swaps) => swaps,
            Err(e) => {
                print_parse_error(&e);
                continue;
            }
        };

        let mint_map = fetch_mints_for(&rpc, &swaps).await;

        match args.style {
            Style::Tree => print_tree(&swaps, args.detail, &mint_map),
            Style::Compact => print_compact(&swaps, args.detail, &mint_map),
            Style::Table => print_table(&swaps, args.detail, &mint_map),
        }
    }
}

/// Collects every mint referenced by `swaps`, fetches their info in a single
/// batched call, and returns a lookup keyed by mint pubkey. Fetch failures
/// are reported but non-fatal: printing falls back to raw amounts and
/// shortened pubkeys for any mint that couldn't be resolved.
async fn fetch_mints_for(rpc: &RpcClient, swaps: &[Parsed]) -> MintMap {
    let mints = collect_mints(swaps);
    if mints.is_empty() {
        return MintMap::new();
    }

    match fetch_mint_infos(rpc, &mints).await {
        Ok(infos) => infos
            .into_iter()
            .flatten()
            .map(|info| (info.pubkey, info))
            .collect(),
        Err(e) => {
            print_error(&format!("failed to fetch mint info: {e}"));
            MintMap::new()
        }
    }
}

fn collect_mints(swaps: &[Parsed]) -> Vec<Pubkey> {
    let mut set = HashSet::new();

    let add_swap = |swap: &Swap, set: &mut HashSet<Pubkey>| {
        set.insert(swap.input_mint);
        set.insert(swap.output_mint);
    };

    for parsed in swaps {
        match parsed {
            Parsed::Dex(dex) => add_swap(&dex.swap, &mut set),
            Parsed::Aggregator(agg) => {
                if let Some(swap) = &agg.swap {
                    add_swap(swap, &mut set);
                }
                for route in agg.routes.iter() {
                    match route {
                        Route::Decoded(dex) => add_swap(&dex.swap, &mut set),
                        Route::Undecoded { transfers, .. } => {
                            for t in transfers {
                                if let Some(mint) = t.mint {
                                    set.insert(mint);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    set.into_iter().collect()
}

fn short(pk: &Pubkey) -> String {
    let s = pk.to_string();
    if s.len() <= 10 {
        s
    } else {
        format!("{}..{}", &s[0..4], &s[s.len() - 4..])
    }
}

fn commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i != 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

/// Adjusts a raw amount by `decimals`, trimming trailing zeros in the
/// fractional part, and comma-separates the whole part.
fn format_amount(raw: u64, decimals: u8) -> String {
    if decimals == 0 {
        return commas(raw);
    }
    let Some(divisor) = 10u64.checked_pow(decimals as u32) else {
        return commas(raw);
    };

    let whole = raw / divisor;
    let frac = raw % divisor;
    let mut frac_str = format!("{:0width$}", frac, width = decimals as usize);
    while frac_str.ends_with('0') {
        frac_str.pop();
    }

    if frac_str.is_empty() {
        commas(whole)
    } else {
        format!("{}.{}", commas(whole), frac_str)
    }
}

/// A raw amount shown adjusted for the mint's decimals if known, otherwise
/// shown as-is with a note that decimals weren't resolved.
fn amount_for_mint(raw: u64, mint: &Pubkey, mints: &MintMap) -> String {
    match mints.get(mint) {
        Some(info) => format_amount(raw, info.decimals),
        None => format!("{} (raw)", commas(raw)),
    }
}

/// Symbol if known, else name, else a shortened pubkey.
fn mint_label(mint: &Pubkey, mints: &MintMap) -> String {
    match mints.get(mint) {
        Some(info) => info
            .symbol
            .clone()
            .or_else(|| info.name.clone())
            .unwrap_or_else(|| short(mint)),
        None => short(mint),
    }
}

fn format_swap(swap: &Swap, mints: &MintMap) -> String {
    format!(
        "{} {} {} {} {}",
        amount_for_mint(swap.input_amount, &swap.input_mint, mints).green(),
        mint_label(&swap.input_mint, mints).dimmed(),
        "->".bold(),
        amount_for_mint(swap.output_amount, &swap.output_mint, mints).green(),
        mint_label(&swap.output_mint, mints).dimmed(),
    )
}

fn print_signature_header(sig: &str) {
    let line = "-".repeat(sig.len() + 4);
    println!("{}", line.dimmed());
    println!("  {}", sig.bold());
    println!("{}", line.dimmed());
}

fn print_error(msg: &str) {
    println!("  {} {}", "x".red().bold(), msg.red());
}

fn print_parse_error(e: &ParseTxError) {
    println!("  {} {}", "x".red().bold(), e.to_string().red());
}

fn print_tree(swaps: &[Parsed], detail: Detail, mints: &MintMap) {
    let last = swaps.len().saturating_sub(1);
    for (i, parsed) in swaps.iter().enumerate() {
        let branch = if i == last { "\\-" } else { "|-" };
        match parsed {
            Parsed::Dex(dex) => print_dex_tree(dex, branch, "  ", mints),
            Parsed::Aggregator(agg) => print_aggregator_tree(agg, branch, "  ", detail, mints),
        }
    }
}

fn child_indent(indent: &str, branch: &str) -> String {
    format!(
        "{indent}{}",
        if branch.starts_with('\\') {
            "   "
        } else {
            "|  "
        }
    )
}

fn print_dex_tree(dex: &DexSwap, branch: &str, indent: &str, mints: &MintMap) {
    println!(
        "{indent}{branch} {} {}",
        "*".cyan(),
        format!("DEX SWAP via {}", dex.program).bold()
    );
    let ci = child_indent(indent, branch);
    println!("{ci}|- user   {}", short(&dex.user).yellow());
    println!("{ci}|- market {}", short(&dex.market).yellow());
    println!("{ci}\\- swap   {}", format_swap(&dex.swap, mints));
}

fn print_aggregator_tree(
    agg: &AggregatorSwap,
    branch: &str,
    indent: &str,
    detail: Detail,
    mints: &MintMap,
) {
    println!(
        "{indent}{branch} {} {}",
        "#".magenta(),
        format!("AGGREGATOR SWAP via {}", agg.program).bold()
    );
    let ci = child_indent(indent, branch);
    let show_routes = detail == Detail::Full && !agg.routes.is_empty();

    println!("{ci}|- user {}", short(&agg.user).yellow());

    let net_branch = if show_routes { "|-" } else { "\\-" };
    match &agg.swap {
        Some(swap) => println!("{ci}{net_branch} net   {}", format_swap(swap, mints)),
        None => println!("{ci}{net_branch} net   {}", "unknown".dimmed()),
    }

    if show_routes {
        println!("{ci}\\- routes ({})", agg.routes.len());
        let ri = format!("{ci}   ");
        let last = agg.routes.len().saturating_sub(1);
        for (i, route) in agg.routes.iter().enumerate() {
            let rb = if i == last { "\\-" } else { "|-" };
            match route {
                Route::Decoded(dex) => print_dex_tree(dex, &format!("[{i}] {rb}"), &ri, mints),
                Route::Undecoded {
                    program,
                    path,
                    transfers,
                } => print_undecoded_tree(program, path, transfers, i, rb, &ri, mints),
            }
        }
    }
}

fn print_undecoded_tree(
    program: &Program,
    path: &Path,
    transfers: &[TokenTransfer],
    i: usize,
    branch: &str,
    indent: &str,
    mints: &MintMap,
) {
    println!(
        "{indent}[{i}] {branch} {} {}",
        "o".red(),
        format!("UNDECODED via {program}").bold()
    );
    let ci = child_indent(indent, branch);
    println!("{ci}|- path {path}");

    if transfers.is_empty() {
        println!("{ci}\\- transfers: none");
        return;
    }

    println!("{ci}\\- transfers ({})", transfers.len());
    let ti = format!("{ci}   ");
    let last = transfers.len().saturating_sub(1);
    for (j, t) in transfers.iter().enumerate() {
        let tb = if j == last { "\\-" } else { "|-" };
        let (amount, mint_label) = match t.mint {
            Some(mint) => (
                amount_for_mint(t.amount, &mint, mints),
                mint_label_str(&mint, mints),
            ),
            None => (
                format!("{} (raw)", commas(t.amount)),
                "unknown mint".to_string(),
            ),
        };
        let token22 = if t.token_22 { ", token-2022" } else { "" };
        println!(
            "{ti}{tb} {} {} {} -> {} (signer {}{token22})",
            amount.green(),
            mint_label.dimmed(),
            short(&t.source_account),
            short(&t.destination_account),
            short(&t.signer),
        );
    }
}

// `mint_label` above is a free function used by `format_swap`; this thin
// wrapper avoids a name clash while keeping call sites readable.
fn mint_label_str(mint: &Pubkey, mints: &MintMap) -> String {
    mint_label(mint, mints)
}

fn print_compact(swaps: &[Parsed], detail: Detail, mints: &MintMap) {
    for parsed in swaps {
        match parsed {
            Parsed::Dex(dex) => println!(
                "{} {:<12} user {} market {}  {}",
                "[DEX]".cyan().bold(),
                dex.program.to_string(),
                short(&dex.user),
                short(&dex.market),
                format_swap(&dex.swap, mints)
            ),
            Parsed::Aggregator(agg) => {
                let net = agg
                    .swap
                    .as_ref()
                    .map(|s| format_swap(s, mints))
                    .unwrap_or_else(|| "unknown".dimmed().to_string());
                println!(
                    "{} {:<12} user {}  net {}  ({} routes)",
                    "[AGG]".magenta().bold(),
                    agg.program.to_string(),
                    short(&agg.user),
                    net,
                    agg.routes.len()
                );
                if detail == Detail::Full {
                    for (i, route) in agg.routes.iter().enumerate() {
                        match route {
                            Route::Decoded(dex) => println!(
                                "    {} [{i}] {:<12} user {} market {}  {}",
                                "L>".dimmed(),
                                dex.program.to_string(),
                                short(&dex.user),
                                short(&dex.market),
                                format_swap(&dex.swap, mints)
                            ),
                            Route::Undecoded {
                                program,
                                path,
                                transfers,
                            } => println!(
                                "    {} [{i}] undecoded via {program} (path {path}, {} transfers)",
                                "L>".dimmed(),
                                transfers.len()
                            ),
                        }
                    }
                }
            }
        }
    }
}

fn print_table(swaps: &[Parsed], detail: Detail, mints: &MintMap) {
    use comfy_table::{Cell, Color, Table, presets::UTF8_FULL};

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Type", "Program", "User", "Input", "Output", "Extra"]);

    for parsed in swaps {
        match parsed {
            Parsed::Dex(dex) => {
                table.add_row(vec![
                    Cell::new("DEX").fg(Color::Cyan),
                    Cell::new(dex.program.to_string()),
                    Cell::new(short(&dex.user)),
                    Cell::new(format!(
                        "{} {}",
                        amount_for_mint(dex.swap.input_amount, &dex.swap.input_mint, mints),
                        mint_label(&dex.swap.input_mint, mints)
                    )),
                    Cell::new(format!(
                        "{} {}",
                        amount_for_mint(dex.swap.output_amount, &dex.swap.output_mint, mints),
                        mint_label(&dex.swap.output_mint, mints)
                    )),
                    Cell::new(format!("market {}", short(&dex.market))),
                ]);
            }
            Parsed::Aggregator(agg) => {
                let (input, output) = match &agg.swap {
                    Some(s) => (
                        format!(
                            "{} {}",
                            amount_for_mint(s.input_amount, &s.input_mint, mints),
                            mint_label(&s.input_mint, mints)
                        ),
                        format!(
                            "{} {}",
                            amount_for_mint(s.output_amount, &s.output_mint, mints),
                            mint_label(&s.output_mint, mints)
                        ),
                    ),
                    None => ("unknown".into(), "unknown".into()),
                };
                table.add_row(vec![
                    Cell::new("AGG").fg(Color::Magenta),
                    Cell::new(agg.program.to_string()),
                    Cell::new(short(&agg.user)),
                    Cell::new(input),
                    Cell::new(output),
                    Cell::new(format!("{} routes", agg.routes.len())),
                ]);

                if detail == Detail::Full {
                    for (i, route) in agg.routes.iter().enumerate() {
                        match route {
                            Route::Decoded(dex) => {
                                table.add_row(vec![
                                    Cell::new(format!("  L[{i}]")),
                                    Cell::new(dex.program.to_string()),
                                    Cell::new(short(&dex.user)),
                                    Cell::new(format!(
                                        "{} {}",
                                        amount_for_mint(
                                            dex.swap.input_amount,
                                            &dex.swap.input_mint,
                                            mints
                                        ),
                                        mint_label(&dex.swap.input_mint, mints)
                                    )),
                                    Cell::new(format!(
                                        "{} {}",
                                        amount_for_mint(
                                            dex.swap.output_amount,
                                            &dex.swap.output_mint,
                                            mints
                                        ),
                                        mint_label(&dex.swap.output_mint, mints)
                                    )),
                                    Cell::new(format!("market {}", short(&dex.market))),
                                ]);
                            }
                            Route::Undecoded {
                                program,
                                path,
                                transfers,
                            } => {
                                table.add_row(vec![
                                    Cell::new(format!("  L[{i}]")),
                                    Cell::new(program.to_string()),
                                    Cell::new(""),
                                    Cell::new(""),
                                    Cell::new(""),
                                    Cell::new(format!(
                                        "undecoded, path {path}, {} transfers",
                                        transfers.len()
                                    )),
                                ]);
                            }
                        }
                    }
                }
            }
        }
    }

    println!("{table}");
}
