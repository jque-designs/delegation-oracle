use anyhow::Result;
use clap::{Parser, Subcommand};

mod types;
mod scanners;
mod api;

use types::*;

#[derive(Debug, Parser)]
#[command(name = "delegation-oracle")]
#[command(about = "Multi-program delegation scanner for Solana validators")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Scan a validator across all delegation programs
    Scan {
        /// Validator vote account pubkey
        validator: String,
        
        /// Specific program to scan (optional)
        #[arg(long)]
        program: Option<String>,
        
        /// Output format
        #[arg(long, default_value = "table")]
        output: OutputFormat,
    },
    
    /// Start the REST API server
    Serve {
        /// Port to listen on
        #[arg(long, default_value_t = 3003)]
        port: u16,
        
        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },
    
    /// List supported programs
    Programs,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Table,
    Json,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Scan { validator, program, output } => {
            let result = scanners::scan_validator(&validator, program.as_deref()).await?;
            
            match output {
                OutputFormat::Table => print_table(&result),
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
            }
        }
        
        Commands::Serve { port, host } => {
            api::serve(&host, port).await?;
        }
        
        Commands::Programs => {
            println!("Supported Programs:");
            println!("  - marinade  : Marinade Finance (mSOL)");
            println!("  - jito      : Jito StakeNet (jitoSOL + MEV)");
            println!("  - blaze     : SolBlaze (bSOL + BLZE)");
            println!("  - sanctum   : Sanctum Gauge (vSOL)");
            println!("  - sfdp      : Solana Foundation Delegation Program");
        }
    }
    
    Ok(())
}

fn print_table(result: &ScanResult) {
    println!("\nValidator: {}", result.validator);
    println!("Scanned: {}\n", result.scanned_at);
    
    println!("┌────────────────┬─────────────┬────────────┬────────────┬──────────┐");
    println!("│ PROGRAM        │ STATUS      │ CURRENT    │ POTENTIAL  │ GAP      │");
    println!("├────────────────┼─────────────┼────────────┼────────────┼──────────┤");
    
    for p in &result.programs {
        let status_str = match p.status {
            RegistrationStatus::Active => "✅ Active",
            RegistrationStatus::Eligible => "⚠️ Eligible",
            RegistrationStatus::NotRegistered => "❌ Not Reg",
            RegistrationStatus::Ineligible => "🚫 Ineligible",
            RegistrationStatus::Unknown => "❓ Unknown",
        };
        
        println!(
            "│ {:<14} │ {:<11} │ {:>10.0} │ {:>10.0} │ {:>+8.0} │",
            p.display_name,
            status_str,
            p.current_stake_sol,
            p.potential_stake_sol,
            p.gap_sol
        );
    }
    
    println!("└────────────────┴─────────────┴────────────┴────────────┴──────────┘\n");
    
    println!("SUMMARY:");
    println!("  Current Stake:    {:>12.0} SOL", result.summary.total_current_sol);
    println!("  Potential Stake:  {:>12.0} SOL", result.summary.total_potential_sol);
    println!("  Missed Revenue:   {:>12.0} SOL/year", result.summary.missed_revenue_sol);
    println!("  Missed Revenue:   ${:>11.0} USD/year\n", result.summary.missed_revenue_usd);
    
    if !result.summary.action_items.is_empty() {
        println!("ACTION ITEMS:");
        for (i, action) in result.summary.action_items.iter().enumerate() {
            println!("  {}. {} (+{:.0} SOL)", i + 1, action.action, action.potential_gain_sol);
            if let Some(url) = &action.url {
                println!("     → {}", url);
            }
        }
    }
}
