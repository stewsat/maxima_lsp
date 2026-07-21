// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

//! Library surface of the Maxima LSP server.
//!
//! The modules are exposed publicly so that they can be exercised by the
//! integration tests under `tests/` in addition to the `maxima-lsp` binary.

pub mod db;
pub mod definitions;
pub mod docs;
pub mod docstring;
pub mod handlers;
pub mod imports;
pub mod lisp_extractor;
pub mod paths;
pub mod server;

#[cfg(test)]
mod parser_audit;
