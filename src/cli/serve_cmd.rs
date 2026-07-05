use std::path::PathBuf;

pub fn run_serve(project: PathBuf, port: u16) -> anyhow::Result<()> {
    // We need to block on the async runtime
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(crate::server::run_server(project, port))?;
    Ok(())
}
