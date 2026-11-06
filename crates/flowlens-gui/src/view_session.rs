//! Session details panel for FlowLens GUI
//! 
//! Tabbed interface with Request, Response, TLS, and Process information panels.

use egui::{Ui, CollapsingHeader, Frame};
use flowlens_core::models::{TrafficEvent, TlsSession, HttpRequest, HttpResponse, ProcessInfo};
use crate::styles::ThemeMode;

/// Render the session details panel with tabs
pub fn render_session_details(ui: &mut Ui, event: &TrafficEvent, selected: Option<usize>, theme: &ThemeManager) {
    if selected.is_none() {
        ui.label("Select an event row to view details");
        return;
    }
    
    let event = &event; // Get the selected event
    
    // Tab bar for different detail views
    egui::TabBar::new("session_tabs").show(ui, |ui| {
        // Request tab
        if ui.tab("Request").clicked() {
            Self::render_request_tab(ui, event);
        }
        // Response tab
        if ui.tab("Response").clicked() {
            Self::render_response_tab(ui, event);
        }
        // TLS tab
        if ui.tab("TLS").clicked() {
            Self::render_tls_tab(ui, event);
        }
        // Process tab
        if ui.tab("Process").clicked() {
            Self::render_process_tab(ui, event);
        }
    });
}

/// Render the Request details tab
fn render_request_tab(ui: &mut Ui, event: &TrafficEvent) {
    ui.heading("Request Details");
    
    // Method and URL
    ui.horizontal(|ui| {
        ui.label("Method:");
        ui.monospace("GET"); // placeholder - would be event.request.method
        ui.label("URL:");
        ui.monospace("/api/users"); // placeholder
    });
    
    ui.horizontal(|ui| {
        ui.label("Protocol:");
        ui.monospace("HTTP/1.1"); // placeholder
    });
    
    // Headers section
    ui.collapsing("Request Headers", |ui| {
        ui.label("Host: example.com");
        ui.label("User-Agent: Mozilla/5.0");
        ui.label("Accept: */*");
        ui.label("Content-Type: application/json");
    });
    
    // Body preview
    ui.collapsing("Body Preview", |ui| {
        ui.label("--- (binary data)");
        ui.label("Size: 245 bytes");
    });
}

/// Render the Response details tab
fn render_response_tab(ui: &mut Ui, event: &TrafficEvent) {
    ui.heading("Response Details");
    
    // Status code and protocol
    ui.horizontal(|ui| {
        ui.label("Status:");
        ui.monospace("200"); // placeholder
        ui.label("Protocol:");
        ui.monospace("HTTP/1.1"); // placeholder
    });
    
    // Response headers
    ui.collapsing("Response Headers", |ui| {
        ui.label("Content-Type: application/json");
        ui.label("Content-Length: 1234");
        ui.label("Cache-Control: max-age=3600");
    });
    
    // Body preview
    ui.collapsing("Body Preview", |ui| {
        ui.label("--- (binary data)");
        ui.label("Size: 567 bytes");
    });
}

/// Render the TLS details tab
fn render_tls_tab(ui: &mut Ui, event: &TrafficEvent) {
    ui.heading("TLS Information");
    
    // Check if TLS info is available
    ui.label("SNI: example.com");
    ui.label("Cipher Suite: TLS_AES_256_GCM_SHA384");
    ui.label("Version: TLS 1.3");
    ui.label("Verification: OK");
    
    // Certificate info section
    ui.collapsing("Certificate Details", |ui| {
        ui.label("Subject: CN=example.com");
        ui.label("Issuer: Let's Encrypt Authority X3");
        ui.label("Not Before: 2024-01-01");
        ui.label("Not After: 2025-01-01");
    });
    
    // Fallback message if no TLS info
    ui.label("TLS not decrypted (without proper CA installation)");
}

/// Render the Process details tab
fn render_process_tab(ui: &mut Ui, event: &TrafficEvent) {
    ui.heading("Process Information");
    
    // PID and process name
    ui.horizontal(|ui| {
        ui.label("PID:");
        ui.monospace("1234");
        ui.label("Name:");
        ui.monospace("chrome.exe");
    });
    
    // EXE path
    ui.horizontal(|ui| {
        ui.label("EXE Path:");
        ui.monospace("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe");
    });
    
    // Command line
    ui.horizontal(|ui| {
        ui.label("Command Line:");
        ui.monospace("--type=renderer --field-trial-handle=...");
    });
    
    // Attribution info
    ui.collapsing("Attribution", |ui| {
        ui.label("Quality: Heuristic");
        ui.label("Interface: Ethernet");
    });
}
///// Styled details panel with syntax highlighting colors

pub struct SessionView;

impl SessionView {
    pub fn render(&self, ui: &mut egui::Ui) {
        ui.label("Session Details");
    }
}