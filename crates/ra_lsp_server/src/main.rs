//! `ra_lsp_server` binary
mod args;

use lsp_server::Connection;
use ra_lsp_server::{cli, from_json, show_message, Result, ServerConfig};
use ra_prof;

use crate::args::HelpPrinted;

fn main() -> Result<()> {
    setup_logging()?;
    let args = match args::Args::parse()? {
        Ok(it) => it,
        Err(HelpPrinted) => return Ok(()),
    };
    match args.command {
        args::Command::Parse { no_dump } => cli::parse(no_dump)?,
        args::Command::Symbols => cli::symbols()?,
        args::Command::Highlight { rainbow } => cli::highlight(rainbow)?,
        args::Command::Stats { randomize, memory_usage, only, with_deps, path } => {
            cli::analysis_stats(
                args.verbosity,
                memory_usage,
                path.as_ref(),
                only.as_ref().map(String::as_ref),
                with_deps,
                randomize,
            )?
        }

        args::Command::Bench { path, what } => {
            cli::analysis_bench(args.verbosity, path.as_ref(), what)?
        }

        args::Command::RunServer => run_server()?,
        args::Command::Version => println!("rust-analyzer {}", env!("REV")),
    }
    Ok(())
}

fn setup_logging() -> Result<()> {
    std::env::set_var("RUST_BACKTRACE", "short");
    env_logger::try_init()?;
    ra_prof::init();
    Ok(())
}

fn run_server() -> Result<()> {
    log::info!("lifecycle: server started");

    let (connection, io_threads) = Connection::stdio();
    let server_capabilities = serde_json::to_value(ra_lsp_server::server_capabilities()).unwrap();

    let initialize_params = connection.initialize(server_capabilities)?;
    let initialize_params =
        from_json::<lsp_types::InitializeParams>("InitializeParams", initialize_params)?;

    if let Some(client_info) = initialize_params.client_info {
        log::info!("Client '{}' {}", client_info.name, client_info.version.unwrap_or_default());
    }

    let cwd = std::env::current_dir()?;
    let root = initialize_params.root_uri.and_then(|it| it.to_file_path().ok()).unwrap_or(cwd);

    let workspace_roots = initialize_params
        .workspace_folders
        .map(|workspaces| {
            workspaces.into_iter().filter_map(|it| it.uri.to_file_path().ok()).collect::<Vec<_>>()
        })
        .filter(|workspaces| !workspaces.is_empty())
        .unwrap_or_else(|| vec![root]);

    let server_config = initialize_params
        .initialization_options
        .and_then(|v| {
            from_json::<ServerConfig>("config", v)
                .map_err(|e| {
                    log::error!("{}", e);
                    show_message(lsp_types::MessageType::Error, e.to_string(), &connection.sender);
                })
                .ok()
        })
        .unwrap_or_default();

    ra_lsp_server::main_loop(
        workspace_roots,
        initialize_params.capabilities,
        server_config,
        connection,
    )?;

    log::info!("shutting down IO...");
    io_threads.join()?;
    log::info!("... IO is down");
    Ok(())
}
