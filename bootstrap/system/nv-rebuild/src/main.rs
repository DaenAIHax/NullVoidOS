use anyhow::Result;
use clap::Parser;
use nv_rebuild::cli::{Cli, Command, Config};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::from_env();

    match cli.command {
        Command::Check => nv_rebuild::cli::cmd_check(&cfg),
        Command::Build => nv_rebuild::cli::cmd_build(&cfg),
        Command::Switch => nv_rebuild::cli::cmd_switch(&cfg),
        Command::Rollback => nv_rebuild::cli::cmd_rollback(&cfg),
        Command::Generations => nv_rebuild::cli::cmd_generations(&cfg),
        Command::Run { service } => nv_rebuild::cli::cmd_run(&cfg, &service),
    }
}
