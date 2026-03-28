use colored::Colorize;
use std::io::{self, Write};

/// Render a complete message with verification status.
pub fn render_message(envelope: &serde_json::Value, verified: bool) {
    let from = envelope
        .get("from")
        .and_then(|f| f.as_str())
        .unwrap_or("unknown");
    let timestamp = envelope
        .get("timestamp")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    // Sender header
    let badge = if verified {
        " [VERIFIED]".green().to_string()
    } else {
        " [FAILED]".red().to_string()
    };
    println!(
        "\n  [{}]{}  {}",
        from.cyan(),
        badge,
        timestamp.dimmed()
    );

    // Render each content block
    if let Some(blocks) = envelope.get("content").and_then(|c| c.as_array()) {
        for block in blocks {
            render_content_block(block);
        }
    }
}

/// Render a single content block based on its type.
pub fn render_content_block(block: &serde_json::Value) {
    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match block_type {
        "text" => {
            let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
            println!("  {}", text);
        }
        "code" => {
            let language = block
                .get("language")
                .and_then(|l| l.as_str())
                .unwrap_or("plaintext");
            let code = block.get("code").and_then(|c| c.as_str()).unwrap_or("");
            println!("  {} {}", "```".dimmed(), language.dimmed());
            for line in code.lines() {
                println!("  {}", line.cyan());
            }
            println!("  {}", "```".dimmed());
        }
        "button" => {
            let label = block.get("label").and_then(|l| l.as_str()).unwrap_or("Button");
            let style = block
                .get("style")
                .and_then(|s| s.as_str())
                .unwrap_or("primary");
            let rendered = match style {
                "primary" => format!("[ {} ]", label).blue().bold().to_string(),
                "secondary" => format!("[ {} ]", label).white().to_string(),
                "danger" => format!("[ {} ]", label).red().bold().to_string(),
                _ => format!("[ {} ]", label).blue().bold().to_string(),
            };
            println!("  {}", rendered);
        }
        "form" => {
            let title = block
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("Form");
            let submit_label = block
                .get("submit_label")
                .and_then(|s| s.as_str())
                .unwrap_or("Submit");
            println!("  {}", format!("=== {} ===", title).yellow().bold());
            if let Some(fields) = block.get("fields").and_then(|f| f.as_array()) {
                for field in fields {
                    let label = field
                        .get("label")
                        .and_then(|l| l.as_str())
                        .unwrap_or("Field");
                    let field_type = field
                        .get("field_type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("text");
                    let required = field
                        .get("required")
                        .and_then(|r| r.as_bool())
                        .unwrap_or(false);
                    let req_marker = if required { "*" } else { "" };

                    if field_type == "select" {
                        if let Some(options) = field.get("options").and_then(|o| o.as_array()) {
                            let opts: Vec<&str> = options
                                .iter()
                                .filter_map(|o| {
                                    // Options can be strings or {value, label} objects
                                    o.as_str().or_else(|| o.get("label").and_then(|l| l.as_str()))
                                })
                                .collect();
                            println!(
                                "    {}{} ({}): [{}]",
                                label,
                                req_marker.red(),
                                field_type.dimmed(),
                                opts.join(" | ")
                            );
                        }
                    } else {
                        println!(
                            "    {}{} ({}): ___",
                            label,
                            req_marker.red(),
                            field_type.dimmed()
                        );
                    }
                }
            }
            println!(
                "    {}",
                format!("[ {} ]", submit_label).green().bold()
            );
        }
        "approval" => {
            let prompt = block
                .get("title")
                .and_then(|p| p.as_str())
                .unwrap_or("Approval needed");
            let status = block
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("pending");
            println!(
                "  {}",
                format!("! APPROVAL: {} !", prompt).yellow().bold()
            );
            println!("    Status: {}", status.dimmed());
            if let Some(options) = block.get("options").and_then(|o| o.as_array()) {
                for opt in options {
                    let label = opt
                        .get("label")
                        .and_then(|l| l.as_str())
                        .unwrap_or("Option");
                    let style = opt
                        .get("style")
                        .and_then(|s| s.as_str())
                        .unwrap_or("primary");
                    let rendered = match style {
                        "primary" => format!("[ {} ]", label).green().bold().to_string(),
                        "danger" => format!("[ {} ]", label).red().bold().to_string(),
                        _ => format!("[ {} ]", label).white().to_string(),
                    };
                    print!("    {} ", rendered);
                }
                println!();
            }
        }
        "table" => {
            if let Some(columns) = block.get("columns").and_then(|c| c.as_array()) {
                let headers: Vec<&str> = columns
                    .iter()
                    .filter_map(|c| c.get("label").and_then(|l| l.as_str()))
                    .collect();
                println!("  {}", headers.join(" | ").bold());
                println!("  {}", "-".repeat(headers.join(" | ").len()));
            }
            if let Some(rows) = block.get("rows").and_then(|r| r.as_array()) {
                for row in rows {
                    if let Some(cells) = row.get("cells").and_then(|c| c.as_array()) {
                        let vals: Vec<String> = cells
                            .iter()
                            .map(|c| {
                                c.as_str()
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| c.to_string())
                            })
                            .collect();
                        println!("  {}", vals.join(" | "));
                    }
                }
            }
        }
        _ => {
            // Fallback for unknown block types
            let fallback = block
                .get("fallback")
                .and_then(|f| f.as_str())
                .unwrap_or("[Unknown block]");
            println!("  {}", fallback.dimmed());
        }
    }
}

/// Render a streaming delta inline (no newline) to show token-by-token arrival.
pub fn render_stream_delta(delta: &str) {
    print!("{}", delta);
    io::stdout().flush().ok();
}

/// Render stream start indicator.
pub fn render_stream_start(_stream_id: &str) {
    print!("  ");
    io::stdout().flush().ok();
}

/// Render stream end with verification badge.
pub fn render_stream_end(verified: bool) {
    let badge = if verified {
        " [VERIFIED]".green().to_string()
    } else {
        " [FAILED]".red().to_string()
    };
    println!("{}", badge);
}

/// Render a tool visibility event.
pub fn render_tool_event(event_type: &str, params: &serde_json::Value) {
    match event_type {
        "started" => {
            let tool_name = params
                .get("tool")
                .and_then(|t| t.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");
            let description = params
                .get("tool")
                .and_then(|t| t.get("description"))
                .and_then(|d| d.as_str())
                .unwrap_or("");
            println!(
                "  {}",
                format!(">> Tool: {} - {}", tool_name, description).yellow()
            );
        }
        "progress" => {
            let progress_text = params
                .get("tool")
                .and_then(|t| t.get("progress_text"))
                .or_else(|| params.get("progress_text"))
                .and_then(|p| p.as_str())
                .unwrap_or("Working...");
            let percent = params
                .get("tool")
                .and_then(|t| t.get("percent_complete"))
                .or_else(|| params.get("percent_complete"))
                .and_then(|p| p.as_u64())
                .unwrap_or(0);
            println!(
                "  {}",
                format!("   {} [{}%]", progress_text, percent).dimmed()
            );
        }
        "completed" => {
            let status = params
                .get("tool")
                .and_then(|t| t.get("status"))
                .and_then(|s| s.as_str())
                .unwrap_or("unknown");
            let result_summary = params
                .get("tool")
                .and_then(|t| t.get("result_summary"))
                .and_then(|r| r.as_str())
                .unwrap_or("");
            let duration_ms = params
                .get("tool")
                .and_then(|t| t.get("duration_ms"))
                .and_then(|d| d.as_u64())
                .unwrap_or(0);
            let color_fn = if status == "success" {
                format!(
                    ">> Tool complete: {} ({}ms)",
                    result_summary, duration_ms
                )
                .green()
                .to_string()
            } else {
                format!(
                    ">> Tool failed: {} ({}ms)",
                    result_summary, duration_ms
                )
                .red()
                .to_string()
            };
            println!("  {}", color_fn);
        }
        _ => {}
    }
}

/// Render a memory operation event.
pub fn render_memory_event(event_type: &str, params: &serde_json::Value) {
    match event_type {
        "stored" => {
            let key = params
                .get("key")
                .and_then(|k| k.as_str())
                .unwrap_or("unknown");
            println!(
                "  {}",
                format!("Stored archival fact: {}", key).green()
            );
        }
        "query_result" => {
            if let Some(results) = params.get("results").and_then(|r| r.as_array()) {
                if results.is_empty() {
                    println!("  No archival facts found matching query.");
                } else {
                    println!("  Archival memory query results:");
                    for fact in results {
                        let key = fact
                            .get("key")
                            .and_then(|k| k.as_str())
                            .unwrap_or("?");
                        let value = fact
                            .get("value")
                            .map(|v| {
                                if let Some(s) = v.as_str() {
                                    format!("\"{}\"", s)
                                } else {
                                    v.to_string()
                                }
                            })
                            .unwrap_or_default();
                        println!(
                            "    - {}: {}",
                            key.yellow(),
                            value
                        );
                    }
                }
            }
        }
        "context_frame" => {
            println!(
                "  {}",
                "=== Context Frame Received ===".yellow().bold()
            );
            if let Some(frame) = params.get("frame") {
                let sections = [
                    "active_context",
                    "conversation_summary",
                    "archival_facts",
                    "participants",
                ];
                for section in &sections {
                    if let Some(val) = frame.get(*section) {
                        let byte_count = serde_json::to_string(val)
                            .map(|s| s.len())
                            .unwrap_or(0);
                        println!(
                            "    [{}] {}",
                            section.cyan(),
                            format!("({} bytes)", byte_count).dimmed()
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

/// Render a bridge compatibility event.
pub fn render_bridge_event(bridge_type: &str, params: &serde_json::Value) {
    match bridge_type {
        "mcp_header" => {
            println!(
                "  {}",
                "--- MCP Bridge ---".cyan().bold()
            );
        }
        "a2a_header" => {
            println!(
                "  {}",
                "--- A2A Bridge ---".cyan().bold()
            );
        }
        "a2a_submitted" => {
            println!(
                "  {}",
                "A2A task submitted -> sub-thread created".yellow()
            );
        }
        "a2a_working" => {
            let detail = params
                .get("detail")
                .and_then(|d| d.as_str())
                .unwrap_or("A2A task working -> agent processing");
            println!(
                "  {}",
                detail.dimmed()
            );
        }
        "a2a_completed" => {
            println!(
                "  {}",
                "A2A task completed -> result delivered".green()
            );
        }
        _ => {}
    }
}

/// Render an Agent Card in formatted display.
pub fn render_agent_card(card: &serde_json::Value) {
    println!();
    println!("  {}", "=== Agent Card ===".cyan().bold());
    if let Some(name) = card.get("name").and_then(|n| n.as_str()) {
        println!("  Name:        {}", name.bold());
    }
    if let Some(desc) = card.get("description").and_then(|d| d.as_str()) {
        println!("  Description: {}", desc);
    }
    if let Some(version) = card.get("version").and_then(|v| v.as_str()) {
        println!("  Version:     {}", version);
    }
    if let Some(ainm) = card.get("ainm") {
        if let Some(addr) = ainm.get("address").and_then(|a| a.as_str()) {
            println!("  Address:     {}", addr.cyan());
        }
        if let Some(types) = ainm.get("contentTypes").and_then(|c| c.as_array()) {
            let type_strs: Vec<&str> = types.iter().filter_map(|t| t.as_str()).collect();
            println!("  Content:     {}", type_strs.join(", "));
        }
        if let Some(pv) = ainm.get("protocolVersion").and_then(|v| v.as_str()) {
            println!("  Protocol:    {}", pv);
        }
        if let Some(streaming) = ainm.get("streaming") {
            let parallel = streaming
                .get("parallelBlocks")
                .and_then(|p| p.as_bool())
                .unwrap_or(false);
            println!("  Parallel:    {}", parallel);
            if let Some(chunk) = streaming.get("preferredChunkSize").and_then(|c| c.as_str()) {
                println!("  Chunk size:  {}", chunk);
            }
        }
        if let Some(threading) = ainm.get("threading") {
            let sub = threading
                .get("subThreads")
                .and_then(|s| s.as_bool())
                .unwrap_or(false);
            println!("  Threads:     sub-threads {}", if sub { "enabled" } else { "disabled" });
            if let Some(max) = threading.get("maxConcurrentThreads").and_then(|m| m.as_u64()) {
                println!("  Concurrency: {} threads", max);
            }
        }
        if let Some(memory) = ainm.get("memory") {
            let archival = memory
                .get("archivalSupported")
                .and_then(|a| a.as_bool())
                .unwrap_or(false);
            let frame = memory
                .get("contextFramePreferred")
                .and_then(|f| f.as_bool())
                .unwrap_or(false);
            println!("  Memory:      archival {}", if archival { "supported" } else { "not supported" });
            println!("  Context:     frame delivery {}", if frame { "preferred" } else { "not preferred" });
        }
        if let Some(tools) = ainm.get("tools").and_then(|t| t.as_array()) {
            println!("  Tools:");
            for tool in tools {
                let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                let desc = tool
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                println!("    - {} ({})", name.yellow(), desc.dimmed());
            }
        }
    }
    if let Some(skills) = card.get("skills").and_then(|s| s.as_array()) {
        println!("  Skills:");
        for skill in skills {
            let name = skill.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            let desc = skill
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            println!("    - {} ({})", name.yellow(), desc.dimmed());
        }
    }
    println!("  {}", "==================".cyan().bold());
    println!();
}
