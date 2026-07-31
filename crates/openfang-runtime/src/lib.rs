//! Agent runtime and execution environment.
//!
//! Manages the agent execution loop, LLM driver abstraction,
//! tool execution, and WASM sandboxing for untrusted skill/plugin code.

/// Default User-Agent header sent with all outgoing HTTP requests.
/// Some LLM providers (e.g. Moonshot, Qwen) reject requests without one.
pub const USER_AGENT: &str = "openfang/0.3.48";

/// Connect timeout for LLM provider requests, in seconds.
///
/// Only covers TCP + TLS establishment, so it can be short: a provider that
/// cannot be reached should fail over immediately rather than stalling a turn.
pub const LLM_CONNECT_TIMEOUT_SECS: u64 = 15;

/// Read timeout for LLM provider requests, in seconds.
///
/// This is a *stall* detector, not a budget for the whole request: it bounds
/// the gap between successive bytes, so a long generation or a long stream is
/// fine as long as the connection keeps producing something.
///
/// `reqwest` applies NO timeout by default. Without this, a provider that
/// accepts the connection and then never answers hangs the agent **forever** —
/// observed repeatedly against NVIDIA NIM, where an agent sat 15 minutes on a
/// single request and made zero tool calls while the same model answered a
/// direct probe in seconds.
///
/// The critical consequence is not just the delay. A hang never returns `Err`,
/// so `FallbackDriver` never fires: the fallback chain is unreachable precisely
/// when it is needed. Converting a hang into an error is what makes fallback
/// work at all.
///
/// Kept well under `DEFAULT_AGENT_TURN_TIMEOUT_SECS` (600) so the driver, not
/// the turn supervisor, is what gives up first — that way the error is
/// attributable to the provider and a fallback still has time to run.
pub const LLM_READ_TIMEOUT_SECS: u64 = 300;

pub mod a2a;
pub mod agent_context;
pub mod agent_loop;
pub mod apply_patch;
pub mod audit;
pub mod auth_cooldown;
pub mod browser;
pub mod command_lane;
pub mod compactor;
pub mod context_budget;
pub mod context_overflow;
pub mod copilot_oauth;
pub mod docker_sandbox;
pub mod drivers;
pub mod embedding;
pub mod graceful_shutdown;
pub mod hooks;
pub mod host_functions;
pub mod image_gen;
pub mod kernel_handle;
pub mod link_understanding;
pub mod llm_driver;
pub mod llm_errors;
pub mod loop_guard;
pub mod mcp;
pub mod mcp_server;
pub mod media_understanding;
pub mod model_catalog;
pub mod process_manager;
pub mod prompt_builder;
pub mod provider_health;
pub mod python_runtime;
pub mod reply_directives;
pub mod retry;
pub mod routing;
pub mod sandbox;
pub mod session_repair;
pub mod shell_bleed;
pub mod str_utils;
pub mod subprocess_sandbox;
pub mod think_filter;
pub mod tool_policy;
pub mod tool_runner;
pub mod tts;
pub mod web_cache;
pub mod web_content;
pub mod web_fetch;
pub mod web_search;
pub mod workspace_context;
pub mod workspace_sandbox;
