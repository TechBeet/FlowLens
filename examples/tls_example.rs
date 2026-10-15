//! FlowLens Example - TLS Decryption
//!
//! This example demonstrates TLS certificate generation and management.

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    println!("FlowLens TLS Example");
    
    // In production, you would use the tls crate's functionality
    println!("TLS engine and CA generation available in flowlens-tls");
    
    Ok(())
}