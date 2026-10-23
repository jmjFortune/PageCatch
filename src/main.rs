use clap::Parser;
use pagecatch::{api, config::Config, downloader, queue::Queue, store::Store};
use std::sync::Arc;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config::parse();
    let cpu = Arc::new(pagecatch::cpu::CpuLimit::new()?);
    config.prepare()?;
    downloader::dependency_check(&config)?;
    // The dependency check starts the engine once; drop the binary pages it
    // leaves behind so an idle container does not keep them as active cache.
    for tool in [&config.downloader, &config.ffmpeg, &config.ffprobe] {
        pagecatch::cache::release(std::path::Path::new(tool));
    }
    if let Some(browser) = pagecatch::resolver::browser::executable(&config) {
        pagecatch::cache::release(&browser);
    }
    if config.doctor {
        println!(
            "Download engine, FFmpeg and ffprobe are available; browser: {:?}",
            pagecatch::resolver::browser::executable(&config)
        );
        return Ok(());
    }
    std::fs::create_dir_all(config.data.join("logs"))?;
    std::fs::create_dir_all(config.data.join("tmp"))?;
    let store = Arc::new(Store::open(&config.data.join("tasks.sqlite"))?);
    if let Some(settings) = store.load_settings()? {
        config = config.with_download_settings(&settings)?;
    }
    cpu.set(config.cpu_cores)?;
    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;
    println!(
        "PageCatch started: http://{}:{} · Save location: {}",
        config.host,
        config.port,
        config.output.display()
    );
    if config.token.is_none() && config.host != "127.0.0.1" && config.host != "localhost" {
        println!("LAN access is enabled. Set PC_TOKEN to control access.");
    }
    let queue = Queue::new(Arc::new(config), store, cpu);
    queue.start();
    let shutdown_queue = queue.clone();
    axum::serve(listener, api::router(queue))
        .with_graceful_shutdown(async move {
            #[cfg(unix)]
            {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .unwrap();
                tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            shutdown_queue.shutdown().await;
        })
        .await?;
    Ok(())
}
