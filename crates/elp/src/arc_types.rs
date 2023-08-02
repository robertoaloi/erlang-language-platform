/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

/// Types as defined in https://www.internalfb.com/intern/wiki/Linting/adding-linters/#flow-type
/// and https://www.internalfb.com/code/whatsapp-server/[4dcee4c563dd9d160ad885069a816907216c9e40]/erl/tools/lint/arcanist.py?lines=17 /
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    // Filepath
    path: String,
    line: Option<u32>,
    char: Option<u32>,
    // Linter name (normally this would need to match code in fbsource-lint-engine.toml)
    code: String,
    // Message severity
    severity: Severity,
    // Rule name
    name: String,
    original: Option<String>,
    replacement: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,    // May crash (eg. syntax errors); always shown; need confirmation
    Warning,  // Minor problems (eg. readability); shown on change; need confirmation
    Autofix,  // Warning that contains an automatic fix in description
    Advice,   // Improvements (eg. leftover comments); shown on change; no confrimation
    Disabled, // Suppressed error message
}

impl Diagnostic {
    pub fn new(
        path: &Path,
        line: u32,
        character: Option<u32>,
        severity: Severity,
        name: String,
        description: String,
        original: Option<String>,
    ) -> Self {
        Diagnostic {
            path: path.display().to_string(), // lossy on Windows for unicode paths
            line: Some(line),
            r#char: character,
            code: "ELP".to_owned(),
            severity,
            name,
            original,
            replacement: None,
            description: Some(description),
        }
    }
}
