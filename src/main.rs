use clap::Parser;
use unityassetdb::cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        unityassetdb::cli::Commands::Index { action } => {
            unityassetdb::cli::run_index_action(action)
        }
        unityassetdb::cli::Commands::Refs {
            path,
            project,
            direction,
            filter,
        } => unityassetdb::cli::run_refs(&path, project, &direction, &filter),
        unityassetdb::cli::Commands::Ls {
            path,
            project,
            depth,
        } => unityassetdb::cli::run_ls(&path, project, depth),
        unityassetdb::cli::Commands::Glob {
            pattern,
            project,
            entry_type,
        } => unityassetdb::cli::run_glob(&pattern, project, &entry_type),
        unityassetdb::cli::Commands::Grep {
            pattern,
            project,
            path,
        } => unityassetdb::cli::run_grep(&pattern, project, path.as_deref()),
        unityassetdb::cli::Commands::Read { path, project } => {
            unityassetdb::cli::run_read(&path, project)
        }
        unityassetdb::cli::Commands::Serve { project, port } => {
            unityassetdb::cli::run_serve(project, port)
        }
        unityassetdb::cli::Commands::Mcp => unityassetdb::cli::run_mcp_server(),
    }
}
