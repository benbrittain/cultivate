use jj_cli::{
    cli_util::{CliRunner, CommandHelper},
    command_error::CommandError,
    ui::Ui,
};
use jj_lib::{
    op_store::WorkspaceId,
    repo::{ReadonlyRepo, StoreFactories},
    signing::Signer,
    workspace::{default_working_copy_factories, Workspace, WorkspaceInitError},
};

mod backend;
mod blocking_client;
mod working_copy;

use backend::CultivateBackend;
use working_copy::{CultivateWorkingCopy, CultivateWorkingCopyFactory};

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
            Workspace::init_with_factories(
                command_helper.settings(),
                wc_path,
                &|settings, store_path| Ok(Box::new(CultivateBackend::new(settings, store_path)?)),
                Signer::from_settings(command_helper.settings())
                    .map_err(WorkspaceInitError::SignInit)?,
                ReadonlyRepo::default_op_store_initializer(),
                ReadonlyRepo::default_op_heads_store_initializer(),
                ReadonlyRepo::default_index_store_initializer(),
                ReadonlyRepo::default_submodule_store_initializer(),
                &CultivateWorkingCopyFactory {},
                WorkspaceId::default(),
            )?;
            Ok(())
        }
    }
}

fn main() -> std::process::ExitCode {
    let mut working_copy_factories = default_working_copy_factories();
    working_copy_factories.insert(
        CultivateWorkingCopy::name().to_owned(),
        Box::new(CultivateWorkingCopyFactory {}),
    );
    // NOTE: logging before this point will not work since it is
    // initialized by CliRunner.
    CliRunner::init()
        .set_store_factories(create_store_factories())
        .set_working_copy_factories(working_copy_factories)
        .add_subcommand(run_cultivate_command)
        .run()
}
