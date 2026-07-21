// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use maxima_lsp::server;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| server::Backend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
