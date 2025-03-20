use std::{fmt::Display, path::Path, time::Instant};

use lapce_rpc::dap_types::{self, RunDebugConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunDebugMode {
    Run,
    Debug,
}

impl Display for RunDebugMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RunDebugMode::Run => "Run",
            RunDebugMode::Debug => "Debug",
        };
        f.write_str(s)
    }
}

#[derive(Clone, Debug)]
pub struct RunDebugProcess {
    pub mode:          RunDebugMode,
    pub origin_config: RunDebugConfig,
    pub config:        RunDebugConfig,
    pub stopped:       bool,
    pub created:       Instant,
    pub is_prelaunch:  bool,
}

#[derive(Deserialize, Serialize)]
pub struct RunDebugConfigs {
    pub configs: Vec<RunDebugConfig>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LapceBreakpoint {
    pub id:       Option<usize>,
    pub verified: bool,
    pub message:  Option<String>,
    pub line:     usize,
    pub offset:   usize,
    pub dap_line: Option<usize>,
    pub active:   bool,
}

#[derive(Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ScopeOrVar {
    Scope(dap_types::Scope),
    Var(dap_types::Variable),
}

impl Default for ScopeOrVar {
    fn default() -> Self {
        ScopeOrVar::Scope(dap_types::Scope::default())
    }
}

impl ScopeOrVar {
    pub fn name(&self) -> &str {
        match self {
            ScopeOrVar::Scope(scope) => &scope.name,
            ScopeOrVar::Var(var) => &var.name,
        }
    }

    pub fn value(&self) -> Option<&str> {
        match self {
            ScopeOrVar::Scope(_) => None,
            ScopeOrVar::Var(var) => Some(&var.value),
        }
    }

    pub fn ty(&self) -> Option<&str> {
        match self {
            ScopeOrVar::Scope(_) => None,
            ScopeOrVar::Var(var) => var.ty.as_deref(),
        }
    }

    pub fn reference(&self) -> usize {
        match self {
            ScopeOrVar::Scope(scope) => scope.variables_reference,
            ScopeOrVar::Var(var) => var.variables_reference,
        }
    }
}

pub struct DapVariableViewdata {
    pub item:     ScopeOrVar,
    pub parent:   Vec<usize>,
    pub expanded: bool,
    pub level:    usize,
}

pub enum BreakpointAction<'a> {
    Remove {
        path: &'a Path,
        line: usize,
    },
    Add {
        path:   &'a Path,
        line:   usize,
        offset: usize,
    },
    Toggle {
        path: &'a Path,
        line: usize,
    },
}
