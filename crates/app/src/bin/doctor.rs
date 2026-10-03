use dalan_acp::SUPPORTED_PROTOCOL_VERSION;
use dalan_drivers::PLANNED_DRIVERS;

fn main() {
    println!("dalan: planning scaffold, not a database client yet.");
    println!("Desktop target: macOS first; Linux next; Windows later.");
    println!(
        "AI boundary: ACP v{SUPPORTED_PROTOCOL_VERSION} only; no app BYOK. Transport not implemented."
    );
    for driver in PLANNED_DRIVERS {
        println!(
            "{}: {:?} ({})",
            driver.engine.display_name(),
            driver.status,
            driver.proposed_backend
        );
    }
    println!("Read docs/product-plan.md and docs/development.md before implementing a feature.");
}
