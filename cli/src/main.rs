use jj_cli::cli_util::{CliRunner, CommandError, CommandHelper};
use jj_cli::ui::Ui;

use jj_lib::{
    repo::StoreFactories,
    signing::Signer,
    workspace::{Workspace, WorkspaceInitError},
};

mod backend;
mod blocking_client;
use backend::CultivateBackend;

#[derive(Debug, Clone, clap::Subcommand)]
enum CultivateCommands {
    Init,
    Status,
}

#[derive(Debug, Clone, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
#[command(flatten_help = true)]
struct CultivateArgs {
    #[command(subcommand)]
    command: CultivateCommands,
}

#[derive(clap::Parser, Clone, Debug)]
enum CultivateSubcommand {
    /// Commands for working with the cultivation daemon
    Cultivate(CultivateArgs),
}

fn create_store_factories() -> StoreFactories {
    let mut store_factories = StoreFactories::default();
    // Register the backend so it can be loaded when the repo is loaded. The name
    // must match `Backend::name()`.
    store_factories.add_backend(
        "cultivate",
        Box::new(|settings, store_path| {
            Ok(Box::new(
                CultivateBackend::new(settings, store_path).unwrap(),
            ))
        }),
    );
    store_factories
}

fn run_cultivate_command(
    _ui: &mut Ui,
    command_helper: &CommandHelper,
    command: CultivateSubcommand,
) -> Result<(), CommandError> {
    let CultivateSubcommand::Cultivate(CultivateArgs { command }) = command;
    match command {
        CultivateCommands::Status => todo!(),
        CultivateCommands::Init => {
            let wc_path = command_helper.cwd();
            // Initialize a workspace with the custom backend
            Workspace::init_with_backend(
                command_helper.settings(),
                wc_path,
                &|settings, store_path| Ok(Box::new(CultivateBackend::new(settings, store_path)?)),
                Signer::from_settings(command_helper.settings())
                    .map_err(WorkspaceInitError::SignInit)?,
            )?;
            Ok(())
        }
    }
}

fn main() -> std::process::ExitCode {
    CliRunner::init()
        .set_store_factories(create_store_factories())
        .add_subcommand(run_cultivate_command)
        .run()
}
